use std::{path::PathBuf, sync::Arc, time::SystemTime};

use wayscriber::config::Config;

pub(super) async fn load_config_from_disk() -> Result<Arc<Config>, String> {
    Config::load()
        .map(|loaded| Arc::new(loaded.config))
        .map_err(|err| err.to_string())
}

pub(super) async fn save_config_to_disk(
    config: Config,
) -> Result<(Option<PathBuf>, Arc<Config>), String> {
    let backup = config.save_with_backup().map_err(|err| err.to_string())?;
    Ok((backup, Arc::new(config)))
}

pub(super) fn load_config_mtime(path: &Option<PathBuf>) -> Option<SystemTime> {
    let path = path.as_ref()?;
    let metadata = std::fs::metadata(path).ok()?;
    metadata.modified().ok()
}
