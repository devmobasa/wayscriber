use std::collections::{BTreeMap, BTreeSet};

use super::{InteractionSeedTarget, InteractionSeedValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeedState {
    generation: u64,
    normalized_value: Option<InteractionSeedValue>,
}

impl SeedState {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn normalized_value(&self) -> Option<&InteractionSeedValue> {
        self.normalized_value.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SeedRegistryError {
    ValueDoesNotMatchTarget(InteractionSeedTarget),
    MissingTarget(InteractionSeedTarget),
    GenerationExhausted(InteractionSeedTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ValidatedInteractionSeeds {
    values: BTreeMap<InteractionSeedTarget, InteractionSeedValue>,
}

impl ValidatedInteractionSeeds {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(
        &mut self,
        target: InteractionSeedTarget,
        value: InteractionSeedValue,
    ) -> Result<Option<InteractionSeedValue>, SeedRegistryError> {
        if !value.matches_target(&target) {
            return Err(SeedRegistryError::ValueDoesNotMatchTarget(target));
        }
        Ok(self.values.insert(target, value))
    }

    pub(crate) fn get(&self, target: &InteractionSeedTarget) -> Option<&InteractionSeedValue> {
        self.values.get(target)
    }

    pub(crate) fn remove(
        &mut self,
        target: &InteractionSeedTarget,
    ) -> Option<InteractionSeedValue> {
        self.values.remove(target)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl Iterator<Item = (&InteractionSeedTarget, &InteractionSeedValue)> {
        self.values.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct InteractionSeedRegistry {
    states: BTreeMap<InteractionSeedTarget, SeedState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedSeedReload {
    registry: InteractionSeedRegistry,
    changed_targets: BTreeSet<InteractionSeedTarget>,
    tombstoned_targets: BTreeSet<InteractionSeedTarget>,
}

impl StagedSeedReload {
    pub(crate) fn stage(
        previous: Option<&Self>,
        current: &InteractionSeedRegistry,
        seeds: ValidatedInteractionSeeds,
    ) -> Result<Self, SeedRegistryError> {
        let mut registry = previous
            .map(|staged| staged.registry.clone())
            .unwrap_or_else(|| current.clone());
        let mut changed_targets = previous
            .map(|staged| staged.changed_targets.clone())
            .unwrap_or_default();
        let mut tombstoned_targets = previous
            .map(|staged| staged.tombstoned_targets.clone())
            .unwrap_or_default();
        tombstoned_targets.extend(
            registry
                .iter()
                .filter(|(target, state)| {
                    state.normalized_value().is_some() && seeds.get(target).is_none()
                })
                .map(|(target, _)| target.clone()),
        );
        changed_targets.extend(registry.update(seeds)?);
        Ok(Self {
            registry,
            changed_targets,
            tombstoned_targets,
        })
    }

    pub(crate) fn registry(&self) -> &InteractionSeedRegistry {
        &self.registry
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        InteractionSeedRegistry,
        BTreeSet<InteractionSeedTarget>,
        BTreeSet<InteractionSeedTarget>,
    ) {
        (self.registry, self.changed_targets, self.tombstoned_targets)
    }
}

impl InteractionSeedRegistry {
    pub(crate) fn from_validated(seeds: ValidatedInteractionSeeds) -> Self {
        let states = seeds
            .values
            .into_iter()
            .map(|(target, normalized_value)| {
                (
                    target,
                    SeedState {
                        generation: 1,
                        normalized_value: Some(normalized_value),
                    },
                )
            })
            .collect();
        Self { states }
    }

    pub(crate) fn state(&self, target: &InteractionSeedTarget) -> Option<&SeedState> {
        self.states.get(target)
    }

    pub(crate) fn current_value(
        &self,
        target: &InteractionSeedTarget,
    ) -> Option<&InteractionSeedValue> {
        self.state(target).and_then(SeedState::normalized_value)
    }

    pub(crate) fn contains_current(&self, target: &InteractionSeedTarget) -> bool {
        self.current_value(target).is_some()
    }

    pub(crate) fn update(
        &mut self,
        seeds: ValidatedInteractionSeeds,
    ) -> Result<BTreeSet<InteractionSeedTarget>, SeedRegistryError> {
        let mut next_registry = self.clone();
        let changed = next_registry.update_in_place(seeds)?;
        *self = next_registry;
        Ok(changed)
    }

    fn update_in_place(
        &mut self,
        seeds: ValidatedInteractionSeeds,
    ) -> Result<BTreeSet<InteractionSeedTarget>, SeedRegistryError> {
        let mut targets = self.states.keys().cloned().collect::<BTreeSet<_>>();
        targets.extend(seeds.values.keys().cloned());

        let mut changed = BTreeSet::new();
        for target in targets {
            let next = seeds.values.get(&target).cloned();
            match self.states.get_mut(&target) {
                Some(state) if state.normalized_value == next => {}
                Some(state) => {
                    state.generation = state
                        .generation
                        .checked_add(1)
                        .ok_or_else(|| SeedRegistryError::GenerationExhausted(target.clone()))?;
                    state.normalized_value = next;
                    changed.insert(target);
                }
                None => {
                    debug_assert!(next.is_some());
                    self.states.insert(
                        target.clone(),
                        SeedState {
                            generation: 1,
                            normalized_value: next,
                        },
                    );
                    changed.insert(target);
                }
            }
        }
        Ok(changed)
    }

    pub(crate) fn guard(
        &self,
        target: &InteractionSeedTarget,
    ) -> Result<SeedGuard, SeedRegistryError> {
        let state = self
            .state(target)
            .filter(|state| state.normalized_value.is_some())
            .ok_or_else(|| SeedRegistryError::MissingTarget(target.clone()))?;
        Ok(SeedGuard {
            target: target.clone(),
            generation: state.generation,
            normalized_seed: state
                .normalized_value
                .clone()
                .expect("current seed checked above"),
        })
    }

    pub(crate) fn guards(
        &self,
        targets: &BTreeSet<InteractionSeedTarget>,
    ) -> Result<Vec<SeedGuard>, SeedRegistryError> {
        targets.iter().map(|target| self.guard(target)).collect()
    }

    pub(crate) fn guard_is_current(&self, guard: &SeedGuard) -> bool {
        self.state(&guard.target).is_some_and(|state| {
            state.generation == guard.generation
                && state.normalized_value.as_ref() == Some(&guard.normalized_seed)
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&InteractionSeedTarget, &SeedState)> {
        self.states.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board_seeds(id: Option<&str>, pinned: bool) -> ValidatedInteractionSeeds {
        let mut seeds = ValidatedInteractionSeeds::new();
        if let Some(id) = id {
            seeds
                .insert(
                    InteractionSeedTarget::BoardPin(id.to_string()),
                    InteractionSeedValue::Bool(pinned),
                )
                .unwrap();
        }
        seeds
    }

    #[test]
    fn staged_remove_then_same_seed_readd_retains_identity_tombstone() {
        let target = InteractionSeedTarget::BoardPin("board-4".to_string());
        let current = InteractionSeedRegistry::from_validated(board_seeds(Some("board-4"), false));
        let removed = StagedSeedReload::stage(None, &current, board_seeds(None, false)).unwrap();
        let restored = StagedSeedReload::stage(
            Some(&removed),
            &current,
            board_seeds(Some("board-4"), false),
        )
        .unwrap();

        let (registry, changed, tombstoned) = restored.into_parts();
        assert_eq!(
            registry.current_value(&target),
            Some(&InteractionSeedValue::Bool(false))
        );
        assert!(changed.contains(&target));
        assert!(tombstoned.contains(&target));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeedGuard {
    pub(crate) target: InteractionSeedTarget,
    pub(crate) generation: u64,
    pub(crate) normalized_seed: InteractionSeedValue,
}
