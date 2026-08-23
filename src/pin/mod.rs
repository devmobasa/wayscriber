mod client;
pub(crate) mod geometry;
pub(crate) mod host;
pub(crate) mod image;
pub(crate) mod limits;
pub(crate) mod protocol;
pub(crate) mod surface;
pub(crate) mod transport;

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::capture::RenderedImage;

pub(crate) use client::{create_pin, pin_available, validate_source_dimensions};
pub(crate) use geometry::PinFrame;
pub(crate) use host::run_host;
pub(crate) use image::PinImage;
pub(crate) use limits::{PinMemoryCharge, PinMemoryLedger};
pub(crate) use protocol::PinCreateResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PinId(u64);

impl PinId {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for PinId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PinRequestId(u64);

impl PinRequestId {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for PinRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PinIdSequence {
    next: Option<u64>,
}

impl Default for PinIdSequence {
    fn default() -> Self {
        Self { next: Some(1) }
    }
}

impl PinIdSequence {
    #[cfg(test)]
    pub(crate) const fn starting_at(next: u64) -> Self {
        Self {
            next: if next == 0 { None } else { Some(next) },
        }
    }

    pub(crate) fn allocate(&mut self) -> Result<PinId, PinRefusal> {
        let value = self.next.ok_or(PinRefusal::IdExhausted)?;
        self.next = value.checked_add(1);
        PinId::new(value).ok_or(PinRefusal::IdExhausted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PinOutputTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinOutputHint {
    pub connector_name: String,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: u32,
    pub transform: PinOutputTransform,
}

impl PinOutputHint {
    pub(crate) fn new(
        connector_name: String,
        logical_width: u32,
        logical_height: u32,
        scale: u32,
        transform: PinOutputTransform,
    ) -> Result<Self, PinCreateError> {
        let value = Self {
            connector_name,
            logical_width,
            logical_height,
            scale,
            transform,
        };
        if !value.is_valid() {
            return Err(PinRefusal::InvalidOutput.into());
        }
        Ok(value)
    }

    pub(crate) const fn logical_size(&self) -> (u32, u32) {
        (self.logical_width, self.logical_height)
    }

    pub(crate) fn is_valid(&self) -> bool {
        !self.connector_name.is_empty()
            && self.connector_name.len() <= 256
            && !self.connector_name.contains('\0')
            && self.logical_width > 0
            && self.logical_height > 0
            && self.scale > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinPlacementHint {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PinPlacementHint {
    pub(crate) fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let value = Self {
            x,
            y,
            width,
            height,
        };
        value.is_valid().then_some(value)
    }

    #[cfg(test)]
    pub(crate) fn from_corners(x1: f64, y1: f64, x2: f64, y2: f64) -> Option<Self> {
        Self::new(x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs())
    }

    pub(crate) fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PinCreateRequest {
    pub request_id: PinRequestId,
    pub image: RenderedImage,
    pub output: PinOutputHint,
    pub placement: PinPlacementHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PinCreateAck {
    pub request_id: PinRequestId,
    pub pin_id: PinId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PinRefusal {
    InvalidImage,
    MetadataMismatch,
    InvalidOutput,
    InvalidPlacement,
    LimitExceeded,
    MemoryLimit,
    TooManyPins,
    IdExhausted,
    UnsupportedVersion,
    UnauthorizedPeer,
}

impl fmt::Display for PinRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidImage => "the pin image is invalid",
            Self::MetadataMismatch => "the pin image metadata does not match its PNG",
            Self::InvalidOutput => "the pin output hint is invalid",
            Self::InvalidPlacement => "the pin placement is invalid",
            Self::LimitExceeded => "the pin exceeds a safety limit",
            Self::MemoryLimit => "the pin host memory limit would be exceeded",
            Self::TooManyPins => "the maximum number of pins is already open",
            Self::IdExhausted => "the pin identifier space is exhausted",
            Self::UnsupportedVersion => "the pin host protocol version is unsupported",
            Self::UnauthorizedPeer => "the pin host peer is not authorized",
        })
    }
}

impl std::error::Error for PinRefusal {}

#[derive(Debug)]
pub(crate) enum PinCreateError {
    SecureRuntimeUnavailable,
    Refused(PinRefusal),
    Timeout,
    Transport(String),
    Host(String),
}

impl fmt::Display for PinCreateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecureRuntimeUnavailable => {
                formatter.write_str("Pin requires a secure runtime directory.")
            }
            Self::Refused(reason) => write!(formatter, "Region was not pinned: {reason}"),
            Self::Timeout => formatter.write_str("Region was not pinned: the pin host timed out"),
            Self::Transport(message) => {
                write!(
                    formatter,
                    "Region was not pinned: pin host transport failed: {message}"
                )
            }
            Self::Host(message) => write!(formatter, "Region was not pinned: {message}"),
        }
    }
}

impl std::error::Error for PinCreateError {}

impl From<PinRefusal> for PinCreateError {
    fn from(value: PinRefusal) -> Self {
        Self::Refused(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_ids_never_wrap_or_reuse() {
        let mut ids = PinIdSequence::starting_at(u64::MAX - 1);
        assert_eq!(ids.allocate().unwrap().get(), u64::MAX - 1);
        assert_eq!(ids.allocate().unwrap().get(), u64::MAX);
        assert_eq!(ids.allocate(), Err(PinRefusal::IdExhausted));
    }

    #[test]
    fn placement_rejects_non_finite_coordinates() {
        assert!(PinPlacementHint::new(f64::NAN, 0.0, 1.0, 1.0).is_none());
    }
}
