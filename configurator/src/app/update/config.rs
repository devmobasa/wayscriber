mod defaults;
mod load;
mod migration;
mod save;
mod status;
#[cfg(test)]
mod tests;

use wayscriber::config::{
    Config, ConfigDiagnosticKind, ConfigDocument, ConfigValidationReport, InvalidKeybinding,
    KeybindingConflictResolution, MigrationPreview,
};

use crate::messages::ConfigSaveResult;
use crate::models::error::FormError;
use crate::models::{ConfigDraft, KeybindingField};

use super::super::effects::Effect;
use super::super::state::{
    ConfiguratorApp, ConfirmationPrompt, PendingConfirmation, StatusMessage,
};

pub(crate) use status::migration_offer_text;
use status::{
    config_document_status, invalid_color_hex_message, list_with_overflow, save_validation_note,
};
