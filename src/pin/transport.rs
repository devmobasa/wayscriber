use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};

use super::PinRefusal;
use super::limits::MAX_PIN_PNG_BYTES;
use super::protocol::{
    MAX_PIN_PACKET_BYTES, PIN_PROTOCOL_VERSION, PinClientPacket, PinCreateResponse, PinCreateWire,
    PinHostPacket,
};

mod runtime;

pub(crate) use runtime::{HostLock, PinListener, PinRuntimePaths, StarterLock};
#[cfg(test)]
use runtime::{PIN_DIR, PIN_LOCK, PIN_SOCKET, PIN_START_LOCK};

pub(crate) struct PinConnection {
    descriptor: OwnedFd,
    negotiated: bool,
}

impl PinConnection {
    fn accepted(descriptor: OwnedFd) -> Self {
        Self {
            descriptor,
            negotiated: false,
        }
    }
    pub(crate) fn connect(path: &Path, timeout: Duration) -> Result<Self> {
        let descriptor = seqpacket_socket()?;
        crate::unix_transport::set_socket_timeout(descriptor.as_raw_fd(), timeout)?;
        connect_path(descriptor.as_raw_fd(), path)?;
        if !authorize_peer_uid(
            crate::unix_transport::peer_uid(descriptor.as_raw_fd())?,
            effective_uid(),
        ) {
            bail!(PinRefusal::UnauthorizedPeer);
        }
        Ok(Self {
            descriptor,
            negotiated: false,
        })
    }

    pub(crate) fn set_nonblocking(&self, enabled: bool) -> io::Result<()> {
        set_nonblocking(self.as_raw_fd(), enabled)
    }

    pub(crate) fn receive(&mut self) -> Result<ReceivedPacket> {
        let (packet, mut descriptors) =
            crate::unix_transport::recv_packet(self.as_raw_fd(), MAX_PIN_PACKET_BYTES, 1)?;
        let packet: PinClientPacket = serde_json::from_slice(&packet)?;
        match packet {
            PinClientPacket::Hello { version } => {
                if !descriptors.is_empty() {
                    bail!("pin hello included a descriptor");
                }
                Ok(ReceivedPacket::Hello { version })
            }
            PinClientPacket::Create(wire) => {
                if !self.negotiated {
                    bail!("pin create arrived before version negotiation");
                }
                if descriptors.len() != 1 {
                    bail!("pin create requires exactly one descriptor");
                }
                if wire.request_id.get() == 0
                    || !wire.output.is_valid()
                    || !wire.placement.is_valid()
                {
                    bail!("pin create metadata is invalid");
                }
                super::limits::validate_source_dimensions(wire.width, wire.height)
                    .map_err(anyhow::Error::msg)?;
                let length = usize::try_from(wire.png_length)
                    .map_err(|_| anyhow!(PinRefusal::LimitExceeded))?;
                if length == 0 || length > MAX_PIN_PNG_BYTES {
                    bail!(PinRefusal::LimitExceeded);
                }
                let descriptor = descriptors.pop().expect("length checked");
                crate::unix_transport::validate_sealed_memfd(&descriptor, length)?;
                Ok(ReceivedPacket::Create(PendingCreate { wire, descriptor }))
            }
        }
    }

    pub(crate) fn send_hello(&mut self, requested: u16) -> Result<bool> {
        let accepted = requested == PIN_PROTOCOL_VERSION;
        let packet = if accepted {
            PinHostPacket::Hello {
                version: PIN_PROTOCOL_VERSION,
            }
        } else {
            PinHostPacket::UnsupportedVersion {
                requested,
                supported: PIN_PROTOCOL_VERSION,
            }
        };
        send_host_packet(self.as_raw_fd(), &packet)?;
        self.negotiated = accepted;
        Ok(accepted)
    }

    pub(crate) fn send_response(&self, response: PinCreateResponse) -> Result<()> {
        if !self.negotiated {
            bail!("pin response sent before version negotiation");
        }
        send_host_packet(self.as_raw_fd(), &PinHostPacket::Create(response))
    }

    pub(crate) fn negotiate_client(&mut self) -> Result<()> {
        send_client_packet(
            self.as_raw_fd(),
            &PinClientPacket::Hello {
                version: PIN_PROTOCOL_VERSION,
            },
            &[],
        )?;
        let response = recv_host_packet(self.as_raw_fd())?;
        match response {
            PinHostPacket::Hello { version } if version == PIN_PROTOCOL_VERSION => {
                self.negotiated = true;
                Ok(())
            }
            PinHostPacket::UnsupportedVersion { .. } => bail!(PinRefusal::UnsupportedVersion),
            _ => bail!("pin host returned an invalid hello response"),
        }
    }

    pub(crate) fn create_client(
        &self,
        wire: PinCreateWire,
        png: &[u8],
    ) -> Result<PinCreateResponse> {
        if !self.negotiated {
            bail!("pin create sent before version negotiation");
        }
        if usize::try_from(wire.png_length).ok() != Some(png.len()) {
            bail!("pin create PNG length does not match its declaration");
        }
        let descriptor = crate::unix_transport::sealed_memfd(c"wayscriber-pin-png", png)?;
        send_client_packet(
            self.as_raw_fd(),
            &PinClientPacket::Create(wire),
            &[descriptor.as_raw_fd()],
        )?;
        match recv_host_packet(self.as_raw_fd())? {
            PinHostPacket::Create(response) => {
                if matches!(
                    &response,
                    PinCreateResponse::Ready { pin_id, .. } if (*pin_id).get() == 0
                ) {
                    bail!("pin host returned an invalid pin identifier");
                }
                Ok(response)
            }
            _ => bail!("pin host returned the wrong create response"),
        }
    }
}

impl AsRawFd for PinConnection {
    fn as_raw_fd(&self) -> RawFd {
        self.descriptor.as_raw_fd()
    }
}

pub(crate) enum ReceivedPacket {
    Hello { version: u16 },
    Create(PendingCreate),
}

pub(crate) struct PendingCreate {
    pub wire: PinCreateWire,
    descriptor: OwnedFd,
}

impl PendingCreate {
    pub(crate) fn read_png(self) -> Result<(PinCreateWire, Vec<u8>)> {
        let length = usize::try_from(self.wire.png_length)?;
        let bytes = crate::unix_transport::read_sealed_memfd(self.descriptor, length)?;
        Ok((self.wire, bytes))
    }
}

fn send_client_packet(fd: RawFd, packet: &PinClientPacket, descriptors: &[RawFd]) -> Result<()> {
    let packet = serde_json::to_vec(packet)?;
    if packet.len() > MAX_PIN_PACKET_BYTES {
        bail!("pin client packet exceeds cap");
    }
    crate::unix_transport::send_packet(fd, &packet, descriptors)?;
    Ok(())
}

fn send_host_packet(fd: RawFd, packet: &PinHostPacket) -> Result<()> {
    let packet = serde_json::to_vec(packet)?;
    if packet.len() > MAX_PIN_PACKET_BYTES {
        bail!("pin host packet exceeds cap");
    }
    crate::unix_transport::send_packet(fd, &packet, &[])?;
    Ok(())
}

fn recv_host_packet(fd: RawFd) -> Result<PinHostPacket> {
    let (packet, descriptors) = crate::unix_transport::recv_packet(fd, MAX_PIN_PACKET_BYTES, 0)?;
    if !descriptors.is_empty() {
        bail!("pin host response included a descriptor");
    }
    Ok(serde_json::from_slice(&packet)?)
}

fn seqpacket_socket() -> io::Result<OwnedFd> {
    // SAFETY: socket returns a new descriptor or -1.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: socket returned a new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}

fn connect_path(fd: RawFd, path: &Path) -> io::Result<()> {
    let (address, length) = socket_address(path)?;
    // SAFETY: address/length describe a fully initialized sockaddr_un.
    if unsafe { libc::connect(fd, (&address as *const libc::sockaddr_un).cast(), length) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn socket_address(path: &Path) -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    let bytes = OsStr::new(path).as_bytes();
    // SAFETY: zero is valid initialization for sockaddr_un.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.len() >= address.sun_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pin socket path is empty or too long",
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (destination, source) in address.sun_path.iter_mut().zip(bytes) {
        *destination = *source as libc::c_char;
    }
    let base = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    let length = base
        .checked_add(bytes.len())
        .and_then(|value| value.checked_add(1))
        .and_then(|value| libc::socklen_t::try_from(value).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "socket length overflow"))?;
    Ok((address, length))
}

fn set_nonblocking(fd: RawFd, enabled: bool) -> io::Result<()> {
    // SAFETY: F_GETFL reads flags from the live descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let flags = if enabled {
        flags | libc::O_NONBLOCK
    } else {
        flags & !libc::O_NONBLOCK
    };
    // SAFETY: F_SETFL updates status flags on the live descriptor.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn effective_uid() -> libc::uid_t {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

const fn authorize_peer_uid(peer: libc::uid_t, effective: libc::uid_t) -> bool {
    peer == effective
}

#[cfg(test)]
mod tests;
