#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DaemonControlProtocolMode {
    LegacyV1,
    #[cfg(test)]
    DarkV2Harness,
    PublishedV2,
}

impl DaemonControlProtocolMode {
    pub(crate) const fn production() -> Self {
        Self::PublishedV2
    }

    #[cfg(test)]
    pub(crate) const fn dark_harness() -> Self {
        Self::DarkV2Harness
    }

    #[cfg(test)]
    pub(crate) const fn rollback_compatibility() -> Self {
        Self::LegacyV1
    }
}
