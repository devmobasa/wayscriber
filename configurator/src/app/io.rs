use std::path::PathBuf;

use wayscriber::config::{Config, ConfigDocument};

pub(super) fn load_config_from_disk(
    config_store: &wayscriber::config::ConfigStore,
) -> Result<(Box<ConfigDocument>, Option<String>), String> {
    ConfigDocument::load_for_editing_from_path(config_store.config_path())
        .map(|(document, warning)| (Box::new(document), warning))
        .map_err(|err| format!("{err:#}"))
}

pub(super) fn save_config_to_disk(
    document: &ConfigDocument,
    config: Config,
) -> Result<(Option<PathBuf>, ConfigDocument), String> {
    let outcome = document
        .save_with_backup(config)
        .map_err(|err| format!("{err:#}"))?;
    let (document, backup) = outcome.into_parts();
    Ok((backup, document))
}
