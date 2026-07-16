use std::{path::PathBuf, sync::Arc};

use wayscriber::config::{Config, ConfigDocument};

use super::blocking_jobs::{BlockingJobKind, run_blocking};

pub(super) async fn load_config_from_disk() -> Result<(Arc<ConfigDocument>, Option<String>), String>
{
    run_blocking(BlockingJobKind::ConfigLoad, || {
        ConfigDocument::load_for_editing()
            .map(|(document, warning)| (Arc::new(document), warning))
            .map_err(|err| format!("{err:#}"))
    })
    .await
}

pub(super) async fn save_config_to_disk(
    document: Arc<ConfigDocument>,
    config: Config,
) -> Result<(Option<PathBuf>, Arc<ConfigDocument>), String> {
    run_blocking(BlockingJobKind::ConfigSave, move || {
        let outcome = document
            .save_with_backup(config)
            .map_err(|err| format!("{err:#}"))?;
        let (document, backup) = outcome.into_parts();
        Ok((backup, Arc::new(document)))
    })
    .await
}
