use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

use super::wire::{
    BlobWire, INLINE_BLOB_BYTES, MAX_OUTPUT_BYTES, MAX_PACKET_BYTES, MAX_PACKET_DESCRIPTORS,
    REQUIRED_MEMFD_SEALS,
};

pub(super) const GRACEFUL_SHUTDOWN_BYTE: u8 = 1;

pub(super) fn set_socket_timeout(fd: RawFd, timeout: Duration) -> io::Result<()> {
    let timeout = timeout.max(Duration::from_millis(1));
    let value = libc::timeval {
        tv_sec: libc::time_t::try_from(timeout.as_secs())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "socket timeout overflow"))?,
        tv_usec: libc::suseconds_t::from(timeout.subsec_micros()),
    };
    for option in [libc::SO_SNDTIMEO, libc::SO_RCVTIMEO] {
        // SAFETY: value is initialized and setsockopt copies it.
        if unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                (&value as *const libc::timeval).cast(),
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
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
            validate_sealed_memfd(&descriptor, length)?;
            let mut bytes = vec![0_u8; length];
            let mut offset = 0;
            while offset < bytes.len() {
                // SAFETY: the destination range is writable and the validated
                // memfd has exactly the declared immutable length.
                let read = unsafe {
                    libc::pread(
                        descriptor.as_raw_fd(),
                        bytes[offset..].as_mut_ptr().cast(),
                        bytes.len() - offset,
                        offset as libc::off_t,
                    )
                };
                if read > 0 {
                    offset += read as usize;
                } else if read < 0
                    && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
                {
                    continue;
                } else {
                    return Err(if read == 0 {
                        anyhow!("broker memfd ended before its declared length")
                    } else {
                        io::Error::last_os_error().into()
                    });
                }
            }
            Ok(bytes)
        }
    }
}

fn sealed_memfd(bytes: &[u8]) -> Result<OwnedFd> {
    let name = c"wayscriber-broker-payload";
    // SAFETY: name is a valid C string and flags request a private, sealable fd.
    let raw =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    if raw < 0 {
        return Err(io::Error::last_os_error()).context("broker memfd creation failed");
    }
    // SAFETY: memfd_create returned a new owned descriptor.
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the source range is readable for the requested length.
        let written = unsafe {
            libc::write(
                descriptor.as_raw_fd(),
                bytes[offset..].as_ptr().cast(),
                bytes.len() - offset,
            )
        };
        if written > 0 {
            offset += written as usize;
        } else if written < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        } else {
            return Err(if written == 0 {
                anyhow!("short write while creating broker memfd")
            } else {
                io::Error::last_os_error().into()
            });
        }
    }
    // SAFETY: fcntl changes only seal metadata on the owned memfd.
    if unsafe {
        libc::fcntl(
            descriptor.as_raw_fd(),
            libc::F_ADD_SEALS,
            REQUIRED_MEMFD_SEALS,
        )
    } < 0
    {
        return Err(io::Error::last_os_error()).context("failed to seal broker memfd");
    }
    Ok(descriptor)
}

fn validate_sealed_memfd(descriptor: &OwnedFd, length: usize) -> Result<()> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: stat points to writable storage for fstat.
    if unsafe { libc::fstat(descriptor.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error()).context("failed to inspect broker memfd");
    }
    // SAFETY: fstat initialized stat after success.
    let stat = unsafe { stat.assume_init() };
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG || stat.st_size != length as libc::off_t {
        bail!("broker memfd shape does not match its declaration");
    }
    // SAFETY: F_GET_SEALS reads descriptor metadata.
    let seals = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GET_SEALS) };
    if seals < 0 || seals & REQUIRED_MEMFD_SEALS != REQUIRED_MEMFD_SEALS {
        bail!("broker memfd is not immutably sealed");
    }
    Ok(())
}

pub(super) fn send_packet(fd: RawFd, packet: &[u8], descriptors: &[RawFd]) -> io::Result<()> {
    if descriptors.len() > MAX_PACKET_DESCRIPTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many broker packet descriptors",
        ));
    }
    loop {
        let mut iovec = libc::iovec {
            iov_base: packet.as_ptr().cast_mut().cast(),
            iov_len: packet.len(),
        };
        let control_len = if descriptors.is_empty() {
            0
        } else {
            unsafe { libc::CMSG_SPACE(std::mem::size_of_val(descriptors) as libc::c_uint) as usize }
        };
        let mut control = vec![0_usize; control_len.div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: zero is a valid initial state for msghdr.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut iovec;
        message.msg_iovlen = 1;
        if !descriptors.is_empty() {
            message.msg_control = control.as_mut_ptr().cast();
            message.msg_controllen = control_len;
            // SAFETY: the control buffer has CMSG_SPACE bytes.
            unsafe {
                let header = libc::CMSG_FIRSTHDR(&message);
                (*header).cmsg_level = libc::SOL_SOCKET;
                (*header).cmsg_type = libc::SCM_RIGHTS;
                (*header).cmsg_len =
                    libc::CMSG_LEN(std::mem::size_of_val(descriptors) as libc::c_uint) as usize;
                std::ptr::copy_nonoverlapping(
                    descriptors.as_ptr(),
                    libc::CMSG_DATA(header).cast::<RawFd>(),
                    descriptors.len(),
                );
            }
        }
        // SAFETY: message references live packet/control buffers.
        let sent = unsafe { libc::sendmsg(fd, &message, libc::MSG_NOSIGNAL) };
        if sent == packet.len() as isize {
            return Ok(());
        }
        if sent < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(if sent < 0 {
            io::Error::last_os_error()
        } else {
            io::Error::new(io::ErrorKind::WriteZero, "short seqpacket send")
        });
    }
}

pub(super) fn recv_packet(fd: RawFd) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let mut buffer = vec![0_u8; MAX_PACKET_BYTES + 1];
    loop {
        let mut iovec = libc::iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: buffer.len(),
        };
        let control_len = unsafe {
            libc::CMSG_SPACE(
                (MAX_PACKET_DESCRIPTORS * std::mem::size_of::<RawFd>()) as libc::c_uint,
            ) as usize
        };
        let mut control = vec![0_usize; control_len.div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: zero is a valid initial state for msghdr.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut iovec;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = control_len;
        // SAFETY: message references writable packet/control buffers.
        let read = unsafe { libc::recvmsg(fd, &mut message, libc::MSG_CMSG_CLOEXEC) };
        if read > 0 {
            let read = read as usize;
            let mut descriptors = Vec::new();
            // SAFETY: recvmsg initialized ancillary headers in the buffer.
            unsafe {
                let mut header = libc::CMSG_FIRSTHDR(&message);
                while !header.is_null() {
                    if (*header).cmsg_level != libc::SOL_SOCKET
                        || (*header).cmsg_type != libc::SCM_RIGHTS
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unexpected broker packet ancillary data",
                        ));
                    }
                    let base_len = libc::CMSG_LEN(0) as usize;
                    if (*header).cmsg_len < base_len {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "malformed broker descriptor payload",
                        ));
                    }
                    let data_len = (*header).cmsg_len - base_len;
                    if !data_len.is_multiple_of(std::mem::size_of::<RawFd>()) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "misaligned broker descriptor payload",
                        ));
                    }
                    let count = data_len / std::mem::size_of::<RawFd>();
                    let data = libc::CMSG_DATA(header).cast::<RawFd>();
                    for index in 0..count {
                        descriptors.push(OwnedFd::from_raw_fd(*data.add(index)));
                    }
                    header = libc::CMSG_NXTHDR(&message, header);
                }
            }
            if read > MAX_PACKET_BYTES
                || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
                || descriptors.len() > MAX_PACKET_DESCRIPTORS
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "broker packet exceeds its data or descriptor cap",
                ));
            }
            buffer.truncate(read);
            return Ok((buffer, descriptors));
        }
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "broker socket closed",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
}
