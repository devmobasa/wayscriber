use std::{path::PathBuf, sync::Arc};

use wayscriber::config::{Config, ConfigDocument};

pub(super) async fn load_config_from_disk() -> Result<(Arc<ConfigDocument>, Option<String>), String>
{
    ConfigDocument::load_for_editing()
        .map(|(document, warning)| (Arc::new(document), warning))
        .map_err(|err| format!("{err:#}"))
}

pub(super) async fn save_config_to_disk(
    document: Arc<ConfigDocument>,
    config: Config,
) -> Result<(Option<PathBuf>, Arc<ConfigDocument>), String> {
    let outcome = document
        .save_with_backup(config)
        .map_err(|err| format!("{err:#}"))?;
    let (document, backup) = outcome.into_parts();
    Ok((backup, Arc::new(document)))
}
