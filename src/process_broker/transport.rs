use std::collections::VecDeque;
use std::io;
use std::os::fd::{OwnedFd, RawFd};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};

use super::wire::{
    BlobWire, INLINE_BLOB_BYTES, MAX_OUTPUT_BYTES, MAX_PACKET_BYTES, MAX_PACKET_DESCRIPTORS,
};

pub(super) const GRACEFUL_SHUTDOWN_BYTE: u8 = 1;

pub(super) fn set_socket_timeout(fd: RawFd, timeout: Duration) -> io::Result<()> {
    crate::unix_transport::set_socket_timeout(fd, timeout)
}

pub(super) fn shutdown_requested(descriptor: RawFd) -> io::Result<bool> {
    let mut pollfd = libc::pollfd {
        fd: descriptor,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: pollfd points to one initialized entry.
    let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if pollfd.revents & libc::POLLNVAL != 0 {
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "broker shutdown channel became invalid",
        ));
    }
    Ok(pollfd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0)
}

pub(super) fn take_graceful_shutdown_signal(descriptor: RawFd) -> io::Result<bool> {
    let mut pollfd = libc::pollfd {
        fd: descriptor,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        // SAFETY: pollfd points to one initialized entry.
        let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if result == 0 {
            return Ok(false);
        }
        if pollfd.revents & libc::POLLNVAL != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker shutdown channel became invalid",
            ));
        }
        if pollfd.revents & libc::POLLIN != 0 {
            return receive_graceful_shutdown_signal(descriptor);
        }
        if pollfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broker shutdown channel closed without a graceful signal",
            ));
        }
        return Ok(false);
    }
}

fn receive_graceful_shutdown_signal(descriptor: RawFd) -> io::Result<bool> {
    let mut byte = 0_u8;
    loop {
        // MSG_TRUNC makes an oversized SOCK_SEQPACKET message report its full length.
        // SAFETY: byte is a valid one-byte destination for this live broker socket.
        let received = unsafe {
            libc::recv(
                descriptor,
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        };
        if received < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if received == 1 && byte == GRACEFUL_SHUTDOWN_BYTE {
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
            crate::unix_transport::read_sealed_memfd(descriptor, length)
        }
    }
}

fn sealed_memfd(bytes: &[u8]) -> Result<OwnedFd> {
    crate::unix_transport::sealed_memfd(c"wayscriber-broker-payload", bytes)
}

pub(super) fn send_packet(fd: RawFd, packet: &[u8], descriptors: &[RawFd]) -> io::Result<()> {
    if descriptors.len() > MAX_PACKET_DESCRIPTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many broker packet descriptors",
        ));
    }
    crate::unix_transport::send_packet(fd, packet, descriptors)
}

pub(super) fn recv_packet(fd: RawFd) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    crate::unix_transport::recv_packet(fd, MAX_PACKET_BYTES, MAX_PACKET_DESCRIPTORS)
}
