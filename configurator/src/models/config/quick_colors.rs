use crate::models::color::ColorInput;
use crate::models::error::FormError;
use wayscriber::config::{ColorSpec, QuickColorConfig, QuickColorSlot, QuickColorsConfig};

const MIN_QUICK_COLOR_ENTRIES: usize = QuickColorSlot::ALL.len();

#[derive(Debug, Clone, PartialEq)]
pub struct QuickColorDraftEntry {
    pub label: String,
    pub color: ColorInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuickColorsDraft {
    pub entries: Vec<QuickColorDraftEntry>,
}

impl QuickColorsDraft {
    pub fn from_config(config: &QuickColorsConfig) -> Self {
        Self {
            entries: config
                .effective_entries()
                .iter()
                .enumerate()
                .map(|(index, entry)| QuickColorDraftEntry {
                    label: entry.resolved_label(index),
                    color: ColorInput::from_color(&entry.color),
                })
                .collect(),
        }
    }

    pub fn apply_to_config(&self, config: &mut QuickColorsConfig, errors: &mut Vec<FormError>) {
        let mut next_entries = Vec::with_capacity(self.entries.len());
        let initial_error_count = errors.len();

        if self.entries.len() < MIN_QUICK_COLOR_ENTRIES {
            errors.push(FormError::new(
                "drawing.quick_colors",
                "Keep at least eight quick colors so existing shortcuts stay bound.",
            ));
        }

        for (index, entry) in self.entries.iter().enumerate() {
            let label = resolved_label(&entry.label, index);

            match entry
                .color
                .to_known_color_spec_with_field(&color_field_name(index))
            {
                Ok(color) => next_entries.push(QuickColorConfig {
                    label: label.to_string(),
                    color,
                }),
                Err(error) => errors.push(error),
            }
        }

        if errors.len() == initial_error_count {
            if next_entries == config.effective_entries() {
                config.replace_entries_preserving_source(next_entries);
            } else {
                config.set_entries(next_entries);
            }
        }
    }

    pub fn get(&self, index: usize) -> Option<&QuickColorDraftEntry> {
        self.entries.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut QuickColorDraftEntry> {
        self.entries.get_mut(index)
    }

    pub fn add_entry(&mut self) {
        self.entries.push(QuickColorDraftEntry::default());
    }

    pub fn remove_entry(&mut self, index: usize) -> bool {
        if self.entries.len() <= MIN_QUICK_COLOR_ENTRIES || index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        true
    }

    pub fn move_entry(&mut self, index: usize, delta: isize) -> bool {
        let Some(target) = offset_index(index, delta, self.entries.len()) else {
            return false;
        };
        self.entries.swap(index, target);
        true
    }
}

impl Default for QuickColorsDraft {
    fn default() -> Self {
        Self::from_config(&QuickColorsConfig::default())
    }
}

impl Default for QuickColorDraftEntry {
    fn default() -> Self {
        Self {
            label: "New color".to_string(),
            color: ColorInput::from_color(&ColorSpec::Name("red".to_string())),
        }
    }
}

pub fn color_field_name(index: usize) -> String {
    format!("drawing.quick_colors[{index}].color")
}

fn resolved_label(label: &str, index: usize) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        QuickColorSlot::from_index(index)
            .map(QuickColorSlot::label)
            .unwrap_or("Quick color")
            .to_string()
    } else {
        trimmed.to_string()
    }
}

fn offset_index(index: usize, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let target = index.checked_add_signed(delta)?;
    (target < len).then_some(target)
}
