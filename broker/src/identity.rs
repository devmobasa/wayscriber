use std::fmt;
use std::io;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolId([u8; 16]);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolToken([u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BootDeadline(u64);

pub struct BootClock;

fn fill_random(bytes: &mut [u8]) -> io::Result<()> {
    let mut filled = 0;
    while filled < bytes.len() {
        // SAFETY: the remaining slice is writable for its full length. getrandom
        // writes at most that length and does not retain the pointer.
        let result = unsafe {
            libc::getrandom(bytes[filled..].as_mut_ptr().cast(), bytes.len() - filled, 0)
        };
        if result > 0 {
            filled += result as usize;
            continue;
        }
        if result == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "getrandom returned zero bytes",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
    Ok(())
}

fn lowercase_hex(bytes: &[u8], output: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(output, "{byte:02x}")?;
    }
    Ok(())
}

impl ProtocolId {
    pub fn generate() -> io::Result<Self> {
        let mut bytes = [0; 16];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ProtocolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for ProtocolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        lowercase_hex(&self.0, formatter)
    }
}

impl ProtocolToken {
    pub fn generate() -> io::Result<Self> {
        let mut bytes = [0; 32];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ProtocolToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtocolToken([redacted])")
    }
}

impl fmt::Display for ProtocolToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        lowercase_hex(&self.0, formatter)
    }
}

impl BootClock {
    pub fn now() -> io::Result<BootDeadline> {
        let mut value = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: value is a valid writable timespec and CLOCK_BOOTTIME has no
        // additional preconditions.
        if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut value) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CLOCK_BOOTTIME returned an invalid timespec",
            ));
        }
        let seconds = u64::try_from(value.tv_sec)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative boot time"))?;
        let nanos = u64::try_from(value.tv_nsec)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative boot nanoseconds"))?;
        let total = seconds
            .checked_mul(1_000_000_000)
            .and_then(|base| base.checked_add(nanos))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "boot time overflow"))?;
        Ok(BootDeadline(total))
    }
}

impl BootDeadline {
    pub const fn from_nanos(value: u64) -> Self {
        Self(value)
    }

    pub fn checked_add(self, duration: Duration) -> io::Result<Self> {
        let nanos = u64::try_from(duration.as_nanos()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "deadline duration overflow")
        })?;
        self.0
            .checked_add(nanos)
            .map(Self)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "deadline overflow"))
    }

    pub const fn as_nanos(self) -> u64 {
        self.0
    }
}
