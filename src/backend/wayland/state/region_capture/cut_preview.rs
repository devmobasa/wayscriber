//! Region cut-preview jobs, correlated results, and live scheduling.

mod apply;
mod job;
mod scheduler;
mod snapshot;

pub(in crate::backend::wayland) use job::CutPreviewOutcome;

#[cfg(test)]
mod tests;
