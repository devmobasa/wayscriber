#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::wayland) struct ScreenAcquisitionId(u64);

impl ScreenAcquisitionId {
    #[cfg(test)]
    fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::wayland) enum ScreenAcquisitionOwner {
    UserFreeze,
    Eyedropper,
    Ocr,
    RegionCapture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ScreenAcquisitionOutcome {
    Ready { installed_generation: u64 },
    Cancelled,
    Unavailable,
    StaleLayout,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ScreenAcquisitionCompletion {
    pub id: ScreenAcquisitionId,
    pub owner: ScreenAcquisitionOwner,
    pub outcome: ScreenAcquisitionOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum AcquisitionStage {
    Queued,
    Started,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct AcquisitionRecord {
    pub id: ScreenAcquisitionId,
    pub owner: ScreenAcquisitionOwner,
    pub stage: AcquisitionStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ScreenAcquisitionBusy;

#[derive(Debug)]
pub(in crate::backend::wayland) struct ScreenAcquisitionRegistry {
    next_id: u64,
    slot: Option<AcquisitionRecord>,
}

impl Default for ScreenAcquisitionRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            slot: None,
        }
    }
}

impl ScreenAcquisitionRegistry {
    pub fn request(
        &mut self,
        owner: ScreenAcquisitionOwner,
    ) -> Result<ScreenAcquisitionId, ScreenAcquisitionBusy> {
        if self.slot.is_some() {
            return Err(ScreenAcquisitionBusy);
        }
        let id = ScreenAcquisitionId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("screen acquisition id space exhausted");
        self.slot = Some(AcquisitionRecord {
            id,
            owner,
            stage: AcquisitionStage::Queued,
        });
        Ok(id)
    }

    pub fn slot(&self) -> Option<&AcquisitionRecord> {
        self.slot.as_ref()
    }

    pub fn mark_started(&mut self, id: ScreenAcquisitionId, owner: ScreenAcquisitionOwner) -> bool {
        let Some(record) = self.slot.as_mut() else {
            return false;
        };
        if record.id != id || record.owner != owner || record.stage != AcquisitionStage::Queued {
            return false;
        }
        record.stage = AcquisitionStage::Started;
        true
    }

    pub fn take(&mut self) -> Option<AcquisitionRecord> {
        self.slot.take()
    }

    pub fn take_matching(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<AcquisitionRecord> {
        if !self
            .slot
            .as_ref()
            .is_some_and(|record| record.id == id && record.owner == owner)
        {
            return None;
        }
        self.slot.take()
    }
}

pub(in crate::backend::wayland) fn rejected_ready_generation(
    owner: ScreenAcquisitionOwner,
    outcome: &ScreenAcquisitionOutcome,
    current_generation: u64,
    frozen_active: bool,
) -> Option<u64> {
    if owner == ScreenAcquisitionOwner::UserFreeze || !frozen_active {
        return None;
    }
    let ScreenAcquisitionOutcome::Ready {
        installed_generation,
    } = outcome
    else {
        return None;
    };
    (*installed_generation == current_generation).then_some(*installed_generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_allocates_monotonic_ids_and_takes_only_matching_records() {
        let mut registry = ScreenAcquisitionRegistry::default();

        let first = registry
            .request(ScreenAcquisitionOwner::Ocr)
            .expect("empty registry accepts a request");
        assert_eq!(first.get(), 1);
        assert_eq!(
            registry.request(ScreenAcquisitionOwner::Eyedropper),
            Err(ScreenAcquisitionBusy)
        );
        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::Eyedropper),
            None
        );
        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::Ocr),
            Some(AcquisitionRecord {
                id: first,
                owner: ScreenAcquisitionOwner::Ocr,
                stage: AcquisitionStage::Queued,
            })
        );

        let second = registry
            .request(ScreenAcquisitionOwner::UserFreeze)
            .expect("released registry accepts another request");
        assert_eq!(second.get(), 2);
        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::UserFreeze),
            None,
            "a stale id must not consume the replacement acquisition"
        );
        assert_eq!(
            registry.slot(),
            Some(&AcquisitionRecord {
                id: second,
                owner: ScreenAcquisitionOwner::UserFreeze,
                stage: AcquisitionStage::Queued,
            })
        );
    }

    #[test]
    fn rejected_ready_release_is_modal_generation_checked() {
        let ready = ScreenAcquisitionOutcome::Ready {
            installed_generation: 7,
        };

        assert_eq!(
            rejected_ready_generation(ScreenAcquisitionOwner::Ocr, &ready, 7, true,),
            Some(7)
        );
        assert_eq!(
            rejected_ready_generation(ScreenAcquisitionOwner::UserFreeze, &ready, 7, true,),
            None
        );
        assert_eq!(
            rejected_ready_generation(ScreenAcquisitionOwner::Eyedropper, &ready, 8, true,),
            None
        );
        assert_eq!(
            rejected_ready_generation(
                ScreenAcquisitionOwner::RegionCapture,
                &ScreenAcquisitionOutcome::StaleLayout,
                7,
                true,
            ),
            None
        );
        assert_eq!(
            rejected_ready_generation(ScreenAcquisitionOwner::Ocr, &ready, 7, false,),
            None
        );
    }

    #[test]
    fn started_owner_cancellation_takes_the_registry_record_once_and_preserves_replacement() {
        let mut registry = ScreenAcquisitionRegistry::default();
        let first = registry
            .request(ScreenAcquisitionOwner::Ocr)
            .expect("first request");
        assert!(registry.mark_started(first, ScreenAcquisitionOwner::Ocr));

        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::Ocr),
            Some(AcquisitionRecord {
                id: first,
                owner: ScreenAcquisitionOwner::Ocr,
                stage: AcquisitionStage::Started,
            })
        );
        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::Ocr),
            None
        );

        let replacement = registry
            .request(ScreenAcquisitionOwner::Eyedropper)
            .expect("replacement request");
        assert_eq!(
            registry.take_matching(first, ScreenAcquisitionOwner::Ocr),
            None
        );
        assert_eq!(
            registry.slot(),
            Some(&AcquisitionRecord {
                id: replacement,
                owner: ScreenAcquisitionOwner::Eyedropper,
                stage: AcquisitionStage::Queued,
            })
        );
    }
}
