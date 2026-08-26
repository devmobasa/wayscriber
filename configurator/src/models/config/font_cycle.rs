//! The font-cycle list as an ordered set of families.
//!
//! `[drawing] font_cycle` is a TOML array, and this is the editor's copy of it.
//! It used to be one comma-separated line, which meant a family whose own name
//! contained a comma could not be typed back in — and made every entry a string
//! the user had to spell exactly. Keeping the list a list removes both.

use std::fmt;

use wayscriber::draw::{families_match, system_font_families};

/// A rejected row edit that the configurator can leave visible and actionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontCycleEditError {
    BlankFamily,
    DuplicateFamily(String),
    NoInstalledFonts,
    AllInstalledFontsListed,
    MissingEntry,
}

impl fmt::Display for FontCycleEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankFamily => formatter.write_str("A font cycle entry cannot be blank"),
            Self::DuplicateFamily(family) => write!(
                formatter,
                "\"{family}\" is already in the font cycle; choose a different font or remove its other row"
            ),
            Self::NoInstalledFonts => {
                formatter.write_str("No installed fonts are available to add")
            }
            Self::AllInstalledFontsListed => {
                formatter.write_str("Every installed font is already in the font cycle")
            }
            Self::MissingEntry => formatter.write_str("That font cycle row no longer exists"),
        }
    }
}

/// The editor's ordered font-cycle entries.
///
/// Owning the entries here keeps blank and duplicate families from becoming
/// representable through the row editor. Config files from older versions are
/// normalized at the draft boundary while retaining their original order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontCycleDraft {
    entries: Vec<String>,
}

impl FontCycleDraft {
    pub(crate) fn from_entries(entries: Vec<String>) -> Self {
        let mut normalized = Vec::with_capacity(entries.len());
        for family in entries {
            let family = family.trim();
            if family.is_empty()
                || normalized
                    .iter()
                    .any(|held: &String| families_match(held, family))
            {
                continue;
            }
            normalized.push(family.to_string());
        }
        Self {
            entries: normalized,
        }
    }

    pub(crate) fn entries(&self) -> &[String] {
        &self.entries
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn add(&mut self) -> Result<(), FontCycleEditError> {
        self.add_from_installed(system_font_families())
    }

    fn add_from_installed(&mut self, installed: &[String]) -> Result<(), FontCycleEditError> {
        if installed.is_empty() {
            return Err(FontCycleEditError::NoInstalledFonts);
        }
        let Some(family) = installed
            .iter()
            .find(|family| !self.entries.iter().any(|held| families_match(held, family)))
        else {
            return Err(FontCycleEditError::AllInstalledFontsListed);
        };
        self.entries.push(family.clone());
        Ok(())
    }

    pub(crate) fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        true
    }

    pub(crate) fn move_entry(&mut self, index: usize, delta: isize) -> bool {
        if self.entries.is_empty() || index >= self.entries.len() {
            return false;
        }
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if target >= self.entries.len() {
            return false;
        }
        self.entries.swap(index, target);
        true
    }

    /// Set one row's family without allowing a blank or repeated identity.
    pub(crate) fn set(&mut self, index: usize, family: String) -> Result<bool, FontCycleEditError> {
        let Some(current) = self.entries.get(index) else {
            return Err(FontCycleEditError::MissingEntry);
        };
        let family = family.trim();
        if family.is_empty() {
            return Err(FontCycleEditError::BlankFamily);
        }
        if families_match(current, family) {
            return Ok(false);
        }
        if self
            .entries
            .iter()
            .enumerate()
            .any(|(other, held)| other != index && families_match(held, family))
        {
            return Err(FontCycleEditError::DuplicateFamily(family.to_string()));
        }
        self.entries[index] = family.to_string();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(entries: &[&str]) -> FontCycleDraft {
        FontCycleDraft::from_entries(entries.iter().map(|entry| (*entry).to_string()).collect())
    }

    #[test]
    fn loading_normalizes_blanks_and_duplicate_identities_once() {
        let draft = draft(&["  Sans  ", "", "sans", "Serif"]);

        assert_eq!(draft.entries(), ["Sans", "Serif"]);
    }

    #[test]
    fn adding_a_row_uses_an_installed_family_not_already_listed() {
        let mut draft = draft(&["Sans"]);
        let installed = vec!["sans".to_string(), "Serif".to_string()];

        assert_eq!(draft.add_from_installed(&installed), Ok(()));
        assert_eq!(draft.entries(), ["Sans", "Serif"]);
    }

    #[test]
    fn adding_is_rejected_when_no_valid_entry_exists() {
        let mut no_fonts = draft(&[]);
        assert_eq!(
            no_fonts.add_from_installed(&[]),
            Err(FontCycleEditError::NoInstalledFonts)
        );
        assert!(no_fonts.is_empty());

        let mut exhaustive = draft(&["Sans", "Serif"]);
        let installed = vec!["sans".to_string(), "SERIF".to_string()];
        assert_eq!(
            exhaustive.add_from_installed(&installed),
            Err(FontCycleEditError::AllInstalledFontsListed)
        );
        assert_eq!(exhaustive.entries(), ["Sans", "Serif"]);
    }

    #[test]
    fn a_row_cannot_be_set_to_a_family_another_row_already_holds() {
        let mut draft = draft(&["Sans", "Serif"]);

        assert_eq!(
            draft.set(1, "sans".to_string()),
            Err(FontCycleEditError::DuplicateFamily("sans".to_string()))
        );
        assert_eq!(draft.entries(), ["Sans", "Serif"]);

        assert_eq!(draft.set(1, "Monospace".to_string()), Ok(true));
        assert_eq!(draft.entries(), ["Sans", "Monospace"]);
    }

    #[test]
    fn blank_and_missing_row_edits_are_rejected() {
        let mut draft = draft(&["Sans"]);

        assert_eq!(
            draft.set(0, "   ".to_string()),
            Err(FontCycleEditError::BlankFamily)
        );
        assert_eq!(
            draft.set(4, "Serif".to_string()),
            Err(FontCycleEditError::MissingEntry)
        );
        assert_eq!(draft.entries(), ["Sans"]);
    }

    #[test]
    fn moving_keeps_the_order_the_cycle_walks() {
        let mut draft = draft(&["A", "B", "C"]);

        assert!(draft.move_entry(0, 1));
        assert_eq!(draft.entries(), ["B", "A", "C"]);
        assert!(draft.move_entry(2, -1));
        assert_eq!(draft.entries(), ["B", "C", "A"]);
    }

    #[test]
    fn moving_past_either_end_does_nothing() {
        let mut draft = draft(&["A", "B"]);

        assert!(!draft.move_entry(0, -1));
        assert!(!draft.move_entry(1, 1));
        assert_eq!(draft.entries(), ["A", "B"]);
    }

    #[test]
    fn the_last_row_can_be_removed_because_an_empty_list_turns_the_action_off() {
        let mut draft = draft(&["A"]);

        assert!(draft.remove(0));
        assert!(draft.is_empty());
        assert!(!draft.remove(0));
    }
}
