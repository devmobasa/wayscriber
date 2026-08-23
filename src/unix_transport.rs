use std::ffi::CStr;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

pub(crate) const REQUIRED_MEMFD_SEALS: i32 =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

pub(crate) fn set_socket_timeout(fd: RawFd, timeout: Duration) -> io::Result<()> {
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

pub(crate) fn sealed_memfd(name: &CStr, bytes: &[u8]) -> Result<OwnedFd> {
    // SAFETY: name is a valid C string and flags request a private, sealable fd.
    let raw =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    if raw < 0 {
        return Err(io::Error::last_os_error()).context("memfd creation failed");
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
                anyhow!("short write while creating memfd")
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
        return Err(io::Error::last_os_error()).context("failed to seal memfd");
    }
    Ok(descriptor)
}

pub(crate) fn validate_sealed_memfd(descriptor: &OwnedFd, length: usize) -> Result<()> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: stat points to writable storage for fstat.
    if unsafe { libc::fstat(descriptor.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error()).context("failed to inspect memfd");
    }
    // SAFETY: fstat initialized stat after success.
    let stat = unsafe { stat.assume_init() };
    let declared = libc::off_t::try_from(length).context("memfd length exceeds off_t")?;
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG || stat.st_size != declared {
        bail!("memfd shape does not match its declaration");
    }
    // SAFETY: F_GET_SEALS reads descriptor metadata.
    let seals = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GET_SEALS) };
    if seals < 0 || seals & REQUIRED_MEMFD_SEALS != REQUIRED_MEMFD_SEALS {
        bail!("memfd is not immutably sealed");
    }
    Ok(())
}

pub(crate) fn read_sealed_memfd(descriptor: OwnedFd, length: usize) -> Result<Vec<u8>> {
    validate_sealed_memfd(&descriptor, length)?;
    let mut bytes = vec![0_u8; length];
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the destination is writable and the validated memfd has exact length.
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
        } else if read < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        } else {
            return Err(if read == 0 {
                anyhow!("memfd ended before its declared length")
            } else {
                io::Error::last_os_error().into()
            });
        }
    }
    Ok(bytes)
}

pub(crate) fn send_packet(fd: RawFd, packet: &[u8], descriptors: &[RawFd]) -> io::Result<()> {
    loop {
        let mut iovec = libc::iovec {
            iov_base: packet.as_ptr().cast_mut().cast(),
            iov_len: packet.len(),
        };
        let control_len = if descriptors.is_empty() {
            0
        } else {
            // SAFETY: size describes the descriptor array passed below.
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

pub(crate) fn recv_packet(
    fd: RawFd,
    max_packet_bytes: usize,
    max_descriptors: usize,
) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let mut buffer = vec![0_u8; max_packet_bytes.saturating_add(1)];
    loop {
        let mut iovec = libc::iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: buffer.len(),
        };
        // Reserve one extra descriptor so an over-cap packet is detectable.
        let descriptor_capacity = max_descriptors.saturating_add(1);
        // SAFETY: capacity is the byte size of the ancillary descriptor buffer.
        let control_len = unsafe {
            libc::CMSG_SPACE((descriptor_capacity * std::mem::size_of::<RawFd>()) as libc::c_uint)
                as usize
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
            let mut ancillary_error = false;
            // SAFETY: recvmsg initialized ancillary headers in the buffer.
            unsafe {
                let mut header = libc::CMSG_FIRSTHDR(&message);
                while !header.is_null() {
                    if (*header).cmsg_level != libc::SOL_SOCKET
                        || (*header).cmsg_type != libc::SCM_RIGHTS
                    {
                        ancillary_error = true;
                    } else {
                        let base_len = libc::CMSG_LEN(0) as usize;
                        if (*header).cmsg_len < base_len {
                            ancillary_error = true;
                        } else {
                            let data_len = (*header).cmsg_len - base_len;
                            if !data_len.is_multiple_of(std::mem::size_of::<RawFd>()) {
                                ancillary_error = true;
                            } else {
                                let count = data_len / std::mem::size_of::<RawFd>();
                                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                                for index in 0..count {
                                    descriptors.push(OwnedFd::from_raw_fd(*data.add(index)));
                                }
                            }
                        }
                    }
                    header = libc::CMSG_NXTHDR(&message, header);
                }
            }
            if ancillary_error
                || read > max_packet_bytes
                || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
                || descriptors.len() > max_descriptors
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "packet exceeds its data or descriptor cap",
                ));
            }
            buffer.truncate(read);
            return Ok((buffer, descriptors));
        }
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "socket closed",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
}

pub(crate) fn peer_uid(fd: RawFd) -> io::Result<libc::uid_t> {
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: credentials and length point to writable storage of the right size.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut length,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "peer credentials had an unexpected size",
        ));
    }
    // SAFETY: getsockopt initialized credentials after success.
    Ok(unsafe { credentials.assume_init() }.uid)
}
