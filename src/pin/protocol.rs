use serde::{Deserialize, Serialize};

use super::{PinId, PinOutputHint, PinPlacementHint, PinRefusal, PinRequestId};

pub(crate) const PIN_PROTOCOL_VERSION: u16 = 1;
pub(crate) const MAX_PIN_PACKET_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "message", deny_unknown_fields)]
pub(crate) enum PinClientPacket {
    Hello { version: u16 },
    Create(PinCreateWire),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "message", deny_unknown_fields)]
pub(crate) enum PinHostPacket {
    Hello { version: u16 },
    UnsupportedVersion { requested: u16, supported: u16 },
    Create(PinCreateResponse),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinCreateWire {
    pub request_id: PinRequestId,
    pub png_length: u64,
    pub width: u32,
    pub height: u32,
    pub output: PinOutputHint,
    pub placement: PinPlacementHint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome", deny_unknown_fields)]
pub(crate) enum PinCreateResponse {
    Ready {
        request_id: PinRequestId,
        pin_id: PinId,
    },
    Refused {
        request_id: PinRequestId,
        reason: PinRefusal,
    },
    Failed {
        request_id: PinRequestId,
        message: String,
    },
}

impl PinCreateResponse {
    pub(crate) const fn request_id(&self) -> PinRequestId {
        match *self {
            Self::Ready { request_id, .. }
            | Self::Refused { request_id, .. }
            | Self::Failed { request_id, .. } => request_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::{PinOutputTransform, PinPlacementHint};

    fn create() -> PinClientPacket {
        PinClientPacket::Create(PinCreateWire {
            request_id: PinRequestId::new(7).unwrap(),
            png_length: 128,
            width: 40,
            height: 20,
            output: PinOutputHint::new("DP-1".into(), 1920, 1080, 2, PinOutputTransform::Flipped90)
                .unwrap(),
            placement: PinPlacementHint::new(10.0, 20.0, 40.0, 20.0).unwrap(),
        })
    }

    #[test]
    fn v1_hello_create_and_ready_round_trip() {
        for packet in [PinClientPacket::Hello { version: 1 }, create()] {
            let json = serde_json::to_vec(&packet).unwrap();
            assert_eq!(
                serde_json::from_slice::<PinClientPacket>(&json).unwrap(),
                packet
            );
        }
        let ready = PinHostPacket::Create(PinCreateResponse::Ready {
            request_id: PinRequestId::new(7).unwrap(),
            pin_id: PinId::new(9).unwrap(),
        });
        let json = serde_json::to_vec(&ready).unwrap();
        assert_eq!(
            serde_json::from_slice::<PinHostPacket>(&json).unwrap(),
            ready
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(
            serde_json::from_str::<PinClientPacket>(
                r#"{"message":"hello","version":1,"future":true}"#
            )
            .is_err()
        );
    }
}
