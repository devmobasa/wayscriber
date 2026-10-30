use std::collections::VecDeque;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use rustix::event::{self, PollFd, PollFlags, Timespec};
use rustix::fs::{self as rustix_fs, FileType, MemfdFlags};
use rustix::io::{self as rustix_io, Errno};
use rustix::net::sockopt::{self, Timeout};
use rustix::net::{
    self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags,
};

use super::wire::{
    BlobWire, INLINE_BLOB_BYTES, MAX_OUTPUT_BYTES, MAX_PACKET_BYTES, MAX_PACKET_DESCRIPTORS,
    REQUIRED_MEMFD_SEALS,
};

pub(super) const GRACEFUL_SHUTDOWN_BYTE: u8 = 1;

pub(super) fn set_socket_timeout(fd: BorrowedFd<'_>, timeout: Duration) -> io::Result<()> {
    let timeout = timeout.max(Duration::from_millis(1));
    for option in [Timeout::Send, Timeout::Recv] {
        sockopt::set_socket_timeout(fd, option, Some(timeout))?;
    }
    Ok(())
}

pub(super) fn shutdown_requested(descriptor: BorrowedFd<'_>) -> io::Result<bool> {
    let mut pollfds = [PollFd::from_borrowed_fd(descriptor, PollFlags::IN)];
    event::poll(&mut pollfds, Some(&Timespec::default()))?;
    let revents = pollfds[0].revents();
    if revents.contains(PollFlags::NVAL) {
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "broker shutdown channel became invalid",
        ));
    }
    Ok(revents.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR))
}

pub(super) fn take_graceful_shutdown_signal(descriptor: BorrowedFd<'_>) -> io::Result<bool> {
    let mut pollfds = [PollFd::from_borrowed_fd(descriptor, PollFlags::IN)];
    loop {
        match event::poll(&mut pollfds, Some(&Timespec::default())) {
            Ok(0) => return Ok(false),
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(errno) => return Err(errno.into()),
        }
        let revents = pollfds[0].revents();
        if revents.contains(PollFlags::NVAL) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker shutdown channel became invalid",
            ));
        }
        if revents.contains(PollFlags::IN) {
            return receive_graceful_shutdown_signal(descriptor);
        }
        if revents.intersects(PollFlags::HUP | PollFlags::ERR) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker shutdown channel closed without a graceful signal",
            ));
        }
        return Ok(false);
    }
}

fn receive_graceful_shutdown_signal(descriptor: BorrowedFd<'_>) -> io::Result<bool> {
    let mut byte = [0_u8];
    loop {
        // TRUNC makes an oversized SOCK_SEQPACKET message report its full length.
        let flags = RecvFlags::DONTWAIT | RecvFlags::TRUNC;
        let received = match net::recv(descriptor, &mut byte, flags) {
            Ok((_, received)) => received,
            Err(Errno::INTR) => continue,
            Err(errno) => return Err(errno.into()),
        };
        if received == 1 && byte[0] == GRACEFUL_SHUTDOWN_BYTE {
            return Ok(true);
        }
        return Err(io::Error::new(
            if received == 0 {
                io::ErrorKind::BrokenPipe
            } else {
                io::ErrorKind::InvalidData
            },
            "broker shutdown channel did not contain the graceful signal",
        ));
    }
}

pub(super) fn encode_blob(bytes: Vec<u8>, cap: usize) -> Result<(BlobWire, Option<OwnedFd>)> {
    if bytes.len() > cap.min(MAX_OUTPUT_BYTES) {
        bail!("broker blob exceeds cap");
    }
    if bytes.len() <= INLINE_BLOB_BYTES {
        return Ok((BlobWire::Inline { bytes }, None));
    }
    let length = bytes.len();
    let descriptor = sealed_memfd(&bytes)?;
    Ok((BlobWire::SealedMemfd { length }, Some(descriptor)))
}

pub(super) fn decode_blob(
    blob: BlobWire,
    descriptors: &mut VecDeque<OwnedFd>,
    cap: usize,
) -> Result<Vec<u8>> {
    let cap = cap.min(MAX_OUTPUT_BYTES);
    match blob {
        BlobWire::Inline { bytes } => {
            if bytes.len() > INLINE_BLOB_BYTES {
                bail!("inline broker blob exceeds cap");
            }
            Ok(bytes)
        }
        BlobWire::SealedMemfd { length } => {
            if length > cap {
                bail!("broker memfd blob exceeds cap");
            }
            let descriptor = descriptors
                .pop_front()
                .ok_or_else(|| anyhow!("broker memfd descriptor is missing"))?;
            validate_sealed_memfd(&descriptor, length)?;
            let mut bytes = vec![0_u8; length];
            let mut offset = 0;
            while offset < bytes.len() {
                match rustix_io::pread(&descriptor, &mut bytes[offset..], offset as u64) {
                    Ok(0) => bail!("broker memfd ended before its declared length"),
                    Ok(read) => offset += read,
                    Err(Errno::INTR) => {}
                    Err(errno) => return Err(io::Error::from(errno).into()),
                }
            }
            Ok(bytes)
        }
    }
}

fn sealed_memfd(bytes: &[u8]) -> Result<OwnedFd> {
    let descriptor = rustix_fs::memfd_create(
        c"wayscriber-broker-payload",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(io::Error::from)
    .context("broker memfd creation failed")?;
    let mut offset = 0;
    while offset < bytes.len() {
        match rustix_io::write(&descriptor, &bytes[offset..]) {
            Ok(0) => bail!("short write while creating broker memfd"),
            Ok(written) => offset += written,
            Err(Errno::INTR) => {}
            Err(errno) => return Err(io::Error::from(errno).into()),
        }
    }
    rustix_fs::fcntl_add_seals(&descriptor, REQUIRED_MEMFD_SEALS)
        .map_err(io::Error::from)
        .context("failed to seal broker memfd")?;
    Ok(descriptor)
}

fn validate_sealed_memfd(descriptor: &OwnedFd, length: usize) -> Result<()> {
    let stat = rustix_fs::fstat(descriptor)
        .map_err(io::Error::from)
        .context("failed to inspect broker memfd")?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || u64::try_from(stat.st_size).ok() != Some(length as u64)
    {
        bail!("broker memfd shape does not match its declaration");
    }
    let sealed = rustix_fs::fcntl_get_seals(descriptor)
        .is_ok_and(|seals| seals.contains(REQUIRED_MEMFD_SEALS));
    if !sealed {
        bail!("broker memfd is not immutably sealed");
    }
    Ok(())
}

pub(super) fn send_packet(
    fd: BorrowedFd<'_>,
    packet: &[u8],
    descriptors: &[BorrowedFd<'_>],
) -> io::Result<()> {
    if descriptors.len() > MAX_PACKET_DESCRIPTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many broker packet descriptors",
        ));
    }
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_PACKET_DESCRIPTORS))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !descriptors.is_empty() && !control.push(SendAncillaryMessage::ScmRights(descriptors)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "broker packet descriptors do not fit their control buffer",
        ));
    }
    let payload = [IoSlice::new(packet)];
    loop {
        match net::sendmsg(fd, &payload, &mut control, SendFlags::NOSIGNAL) {
            Ok(sent) if sent == packet.len() => return Ok(()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "short seqpacket send",
                ));
            }
            Err(Errno::INTR) => {}
            Err(errno) => return Err(errno.into()),
        }
    }
}

pub(super) fn recv_packet(fd: BorrowedFd<'_>) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let mut buffer = vec![0_u8; MAX_PACKET_BYTES + 1];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_PACKET_DESCRIPTORS))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let received = loop {
        let mut payload = [IoSliceMut::new(&mut buffer)];
        match net::recvmsg(fd, &mut payload, &mut control, RecvFlags::CMSG_CLOEXEC) {
            Ok(received) => break received,
            Err(Errno::INTR) => {}
            Err(errno) => return Err(errno.into()),
        }
    };
    if received.bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "broker socket closed",
        ));
    }
    // Descriptors left in `control` close when it drops, so every rejection
    // below releases what the packet carried.
    let mut descriptors = Vec::new();
    for message in control.drain() {
        let RecvAncillaryMessage::ScmRights(passed) = message else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected broker packet ancillary data",
            ));
        };
        descriptors.extend(passed);
    }
    if received.bytes > MAX_PACKET_BYTES
        || received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        || descriptors.len() > MAX_PACKET_DESCRIPTORS
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "broker packet exceeds its data or descriptor cap",
        ));
    }
    buffer.truncate(received.bytes);
    Ok((buffer, descriptors))
}
