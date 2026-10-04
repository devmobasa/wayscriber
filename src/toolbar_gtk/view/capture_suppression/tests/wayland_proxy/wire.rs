//! Wayland stream framing. Descriptors stay with the message during whose
//! bytes they arrived and are passed on, then closed, when it is sent.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;

use anyhow::{Result, ensure};

const HEADER_LEN: usize = 8;
/// Ancillary space reserved for each read, in descriptors.
const RECEIVE_DESCRIPTORS: usize = 256;
// SAFETY: CMSG_SPACE only performs arithmetic on its argument.
const RECEIVE_CONTROL_LEN: usize =
    unsafe { libc::CMSG_SPACE((RECEIVE_DESCRIPTORS * size_of::<RawFd>()) as u32) } as usize;

pub(super) struct Packet {
    data: Vec<u8>,
    descriptors: Vec<OwnedFd>,
}

impl Packet {
    pub(super) fn object_id(&self) -> u32 {
        word_at(&self.data, 0)
    }

    pub(super) fn opcode(&self) -> usize {
        (word_at(&self.data, 4) & 0xffff) as usize
    }

    pub(super) fn payload(&self) -> &[u8] {
        &self.data[HEADER_LEN..]
    }
}

fn word_at(data: &[u8], offset: usize) -> u32 {
    let mut word = [0; 4];
    word.copy_from_slice(&data[offset..offset + 4]);

    u32::from_ne_bytes(word)
}

/// Reads exactly one message, or `None` at end of stream.
pub(super) fn receive(socket: &UnixStream) -> Result<Option<Packet>> {
    let mut data = Vec::with_capacity(HEADER_LEN);
    let mut descriptors = Vec::new();
    let mut size = HEADER_LEN;

    while data.len() < size {
        let filled = data.len();
        data.resize(size, 0);
        let read = receive_chunk(socket, &mut data[filled..], &mut descriptors)?;
        if read == 0 {
            return Ok(None);
        }
        data.truncate(filled + read);

        if data.len() == HEADER_LEN {
            size = (word_at(&data, 4) >> 16) as usize;
            ensure!(
                size >= HEADER_LEN && size.is_multiple_of(4),
                "invalid Wayland message length"
            );
        }
    }

    Ok(Some(Packet { data, descriptors }))
}

fn receive_chunk(
    socket: &UnixStream,
    buffer: &mut [u8],
    descriptors: &mut Vec<OwnedFd>,
) -> Result<usize> {
    let mut control = [0_u64; RECEIVE_CONTROL_LEN.div_ceil(size_of::<u64>())];
    let mut iovec = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast(),
        iov_len: buffer.len(),
    };
    // SAFETY: zero is a valid initial state for msghdr.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iovec;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = RECEIVE_CONTROL_LEN;

    let read = loop {
        // SAFETY: message references the live, writable data and control buffers.
        let read =
            unsafe { libc::recvmsg(socket.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
        if read >= 0 {
            break read as usize;
        }

        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    };

    // Own the descriptors first so they close if the read is rejected.
    take_descriptors(&message, descriptors)?;
    ensure!(
        message.msg_flags & libc::MSG_CTRUNC == 0,
        "truncated Wayland descriptors"
    );

    Ok(read)
}

fn take_descriptors(message: &libc::msghdr, descriptors: &mut Vec<OwnedFd>) -> Result<()> {
    // SAFETY: recvmsg filled msg_controllen bytes of the control buffer.
    let mut header = unsafe { libc::CMSG_FIRSTHDR(message) };
    while !header.is_null() {
        // SAFETY: CMSG_FIRSTHDR/CMSG_NXTHDR return only complete headers.
        let (level, kind, length) = unsafe {
            (
                (*header).cmsg_level,
                (*header).cmsg_type,
                (*header).cmsg_len,
            )
        };

        if level == libc::SOL_SOCKET && kind == libc::SCM_RIGHTS {
            // SAFETY: CMSG_LEN only performs arithmetic on its argument.
            let bytes = length - unsafe { libc::CMSG_LEN(0) } as usize;
            // SAFETY: the kernel wrote `bytes` bytes of descriptors after the header.
            let data = unsafe { libc::CMSG_DATA(header) }.cast::<RawFd>();
            for index in 0..bytes / size_of::<RawFd>() {
                // SAFETY: each received descriptor is open and owned by nobody else.
                descriptors.push(unsafe { OwnedFd::from_raw_fd(data.add(index).read_unaligned()) });
            }
            ensure!(
                bytes.is_multiple_of(size_of::<RawFd>()),
                "misaligned Wayland descriptors"
            );
        }

        // SAFETY: header is a header of this message's control buffer.
        header = unsafe { libc::CMSG_NXTHDR(message, header) };
    }

    Ok(())
}

/// Writes the whole message with its descriptors attached to the first chunk.
pub(super) fn send(socket: &UnixStream, packet: Packet) -> Result<()> {
    let mut sent = send_chunk(socket, &packet.data, &packet.descriptors)?;
    while sent < packet.data.len() {
        sent += send_chunk(socket, &packet.data[sent..], &[])?;
    }

    Ok(())
}

fn send_chunk(socket: &UnixStream, bytes: &[u8], descriptors: &[OwnedFd]) -> Result<usize> {
    let raw: Vec<RawFd> = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
    // SAFETY: CMSG_SPACE only performs arithmetic on its argument.
    let control_len = unsafe { libc::CMSG_SPACE(size_of_val(raw.as_slice()) as u32) } as usize;
    let mut control = vec![0_u64; control_len.div_ceil(size_of::<u64>())];
    let mut iovec = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: zero is a valid initial state for msghdr.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iovec;
    message.msg_iovlen = 1;

    if !raw.is_empty() {
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = control_len;
        // SAFETY: the control buffer holds CMSG_SPACE bytes for these descriptors.
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(size_of_val(raw.as_slice()) as u32) as usize;
            std::ptr::copy_nonoverlapping(
                raw.as_ptr(),
                libc::CMSG_DATA(header).cast::<RawFd>(),
                raw.len(),
            );
        }
    }

    loop {
        // SAFETY: message references the live data and control buffers.
        let sent = unsafe { libc::sendmsg(socket.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
        if sent > 0 {
            return Ok(sent as usize);
        }

        let error = match sent {
            0 => io::Error::from(io::ErrorKind::WriteZero),
            _ => io::Error::last_os_error(),
        };
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}
