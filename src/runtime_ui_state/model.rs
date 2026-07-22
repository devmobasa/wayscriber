use std::collections::{BTreeMap, BTreeSet};

use super::{
    ControllerId, InteractionSeedRegistry, InteractionSeedTarget, InteractionSeedValue, SeedGuard,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeOverride {
    pub(crate) seed: InteractionSeedValue,
    pub(crate) value: InteractionSeedValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeUiModel {
    overrides: BTreeMap<InteractionSeedTarget, RuntimeOverride>,
}

impl RuntimeUiModel {
    pub(crate) fn get(&self, target: &InteractionSeedTarget) -> Option<&RuntimeOverride> {
        self.overrides.get(target)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&InteractionSeedTarget, &RuntimeOverride)> {
        self.overrides.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub(in crate::runtime_ui_state) fn insert_decoded(
        &mut self,
        target: InteractionSeedTarget,
        runtime_override: RuntimeOverride,
    ) -> Result<(), InteractionSeedTarget> {
        if !target.is_runtime_owned()
            || !runtime_override.seed.matches_target(&target)
            || !runtime_override.value.matches_target(&target)
            || self.overrides.contains_key(&target)
        {
            return Err(target);
        }
        self.overrides.insert(target, runtime_override);
        Ok(())
    }

    pub(crate) fn apply(
        &mut self,
        guards: &[SeedGuard],
        desired: &RuntimeUiMutationValues,
    ) -> bool {
        let before = self.clone();
        for guard in guards {
            let value = desired
                .values
                .get(&guard.target)
                .expect("mutation values validated against permit scope");
            if value == &guard.normalized_seed {
                self.overrides.remove(&guard.target);
            } else {
                self.overrides.insert(
                    guard.target.clone(),
                    RuntimeOverride {
                        seed: guard.normalized_seed.clone(),
                        value: value.clone(),
                    },
                );
            }
        }
        *self != before
    }

    pub(crate) fn reconcile(&mut self, seeds: &InteractionSeedRegistry) -> bool {
        let before = self.overrides.len();
        self.overrides.retain(|target, runtime_override| {
            target.is_runtime_owned()
                && seeds.current_value(target).is_some_and(|seed| {
                    seed == &runtime_override.seed && seed != &runtime_override.value
                })
        });
        self.overrides.len() != before
    }

    pub(crate) fn remove_targets(&mut self, targets: &BTreeSet<InteractionSeedTarget>) -> bool {
        let before = self.overrides.len();
        self.overrides.retain(|target, _| !targets.contains(target));
        self.overrides.len() != before
    }

    pub(crate) fn clear(&mut self) {
        self.overrides.clear();
    }
}

/// Canonical controller snapshot handed to the future wire encoder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeUiWireState {
    pub(crate) model: RuntimeUiModel,
    pub(crate) passthrough: WirePassthrough,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct WirePassthrough {
    pub(in crate::runtime_ui_state) top_level: BTreeMap<String, String>,
    pub(in crate::runtime_ui_state) toolbar: BTreeMap<String, String>,
    pub(in crate::runtime_ui_state) boards: BTreeMap<String, String>,
    pub(in crate::runtime_ui_state) entries:
        BTreeMap<InteractionSeedTarget, BTreeMap<String, String>>,
}

impl WirePassthrough {
    pub(crate) fn is_empty(&self) -> bool {
        self.top_level.is_empty()
            && self.toolbar.is_empty()
            && self.boards.is_empty()
            && self.entries.values().all(BTreeMap::is_empty)
    }

    pub(crate) fn reconcile_entries(&mut self, model: &RuntimeUiModel) -> bool {
        let before = self.entries.len();
        self.entries.retain(|target, _| model.get(target).is_some());
        self.entries.len() != before
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeUiLiveOnlyOverlay {
    values: BTreeMap<InteractionSeedTarget, InteractionSeedValue>,
}

impl RuntimeUiLiveOnlyOverlay {
    pub(crate) fn get(&self, target: &InteractionSeedTarget) -> Option<&InteractionSeedValue> {
        self.values.get(target)
    }

    pub(crate) fn clear(&mut self) {
        self.values.clear();
    }

    pub(crate) fn apply(
        &mut self,
        guards: &[SeedGuard],
        desired: &RuntimeUiMutationValues,
    ) -> bool {
        let before = self.clone();
        for guard in guards {
            let value = desired
                .values
                .get(&guard.target)
                .expect("live-only values validated against guard scope");
            if value == &guard.normalized_seed {
                self.values.remove(&guard.target);
            } else {
                self.values.insert(guard.target.clone(), value.clone());
            }
        }
        *self != before
    }

    pub(crate) fn reconcile(&mut self, changed_targets: &BTreeSet<InteractionSeedTarget>) {
        self.values
            .retain(|target, _| !changed_targets.contains(target));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeUiLiveState {
    values: BTreeMap<InteractionSeedTarget, InteractionSeedValue>,
}

impl RuntimeUiLiveState {
    pub(crate) fn rebuild(
        seeds: &InteractionSeedRegistry,
        model: &RuntimeUiModel,
        overlay: &RuntimeUiLiveOnlyOverlay,
    ) -> Self {
        let mut values = seeds
            .iter()
            .filter_map(|(target, state)| {
                state
                    .normalized_value()
                    .cloned()
                    .map(|value| (target.clone(), value))
            })
            .collect::<BTreeMap<_, _>>();
        for (target, runtime_override) in model.iter() {
            values.insert(target.clone(), runtime_override.value.clone());
        }
        for (target, value) in &overlay.values {
            values.insert(target.clone(), value.clone());
        }
        Self { values }
    }

    pub(crate) fn get(&self, target: &InteractionSeedTarget) -> Option<&InteractionSeedValue> {
        self.values.get(target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeUiMutationScope {
    Target(InteractionSeedTarget),
    Batch(Vec<InteractionSeedTarget>),
}

impl RuntimeUiMutationScope {
    pub(crate) fn one(target: InteractionSeedTarget) -> Self {
        Self::Target(target)
    }

    pub(crate) fn batch(targets: impl IntoIterator<Item = InteractionSeedTarget>) -> Self {
        Self::Batch(targets.into_iter().collect())
    }

    pub(crate) fn canonical_targets(
        &self,
    ) -> Result<BTreeSet<InteractionSeedTarget>, MutationShapeError> {
        let targets: BTreeSet<InteractionSeedTarget> = match self {
            Self::Target(target) => std::iter::once(target.clone()).collect(),
            Self::Batch(targets) => targets.iter().cloned().collect(),
        };
        if targets.is_empty() {
            return Err(MutationShapeError::EmptyScope);
        }
        if let Some(target) = targets.iter().find(|target| !target.is_runtime_owned()) {
            return Err(MutationShapeError::ConfigTargetInRuntimeScope(
                target.clone(),
            ));
        }
        Ok(targets)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MutationShapeError {
    EmptyScope,
    ConfigTargetInRuntimeScope(InteractionSeedTarget),
    ValueDoesNotMatchTarget(InteractionSeedTarget),
    ValuesDoNotMatchPermitScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeUiMutationValues {
    values: BTreeMap<InteractionSeedTarget, InteractionSeedValue>,
}

impl RuntimeUiMutationValues {
    pub(crate) fn one(
        target: InteractionSeedTarget,
        value: InteractionSeedValue,
    ) -> Result<Self, MutationShapeError> {
        Self::batch([(target, value)])
    }

    pub(crate) fn batch(
        values: impl IntoIterator<Item = (InteractionSeedTarget, InteractionSeedValue)>,
    ) -> Result<Self, MutationShapeError> {
        let mut result = BTreeMap::new();
        for (target, value) in values {
            if !value.matches_target(&target) {
                return Err(MutationShapeError::ValueDoesNotMatchTarget(target));
            }
            result.insert(target, value);
        }
        Ok(Self { values: result })
    }

    pub(crate) fn targets(&self) -> BTreeSet<InteractionSeedTarget> {
        self.values.keys().cloned().collect()
    }

    pub(crate) fn values(&self) -> &BTreeMap<InteractionSeedTarget, InteractionSeedValue> {
        &self.values
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeUiMutationPermit {
    pub(crate) controller_id: ControllerId,
    pub(crate) authority_epoch: u64,
    pub(crate) mutation_id: u64,
    pub(crate) guards: Vec<SeedGuard>,
}

impl RuntimeUiMutationPermit {
    pub(crate) fn targets(&self) -> BTreeSet<InteractionSeedTarget> {
        self.guards
            .iter()
            .map(|guard| guard.target.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigPositionTarget {
    Top,
    Side,
}

impl ConfigPositionTarget {
    pub(crate) fn seed_targets(self) -> Vec<InteractionSeedTarget> {
        match self {
            Self::Top => vec![InteractionSeedTarget::TopPosition],
            // A side drag can reconcile and persist the top X offset when the
            // overlap-derived base changes, so it must be fenced by both
            // authored position seeds.
            Self::Side => vec![
                InteractionSeedTarget::TopPosition,
                InteractionSeedTarget::SidePosition,
            ],
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConfigInteractionPermit {
    pub(crate) controller_id: ControllerId,
    pub(crate) authority_epoch: u64,
    pub(crate) mutation_id: u64,
    pub(crate) guards: Vec<SeedGuard>,
    pub(crate) target: ConfigPositionTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PreviewRollbackSnapshot {
    pub(crate) values: BTreeMap<InteractionSeedTarget, InteractionSeedValue>,
}
