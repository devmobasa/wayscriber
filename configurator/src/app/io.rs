use wayscriber::config::{Config, ConfigDocument};

use crate::messages::ConfigSaveResult;

use super::blocking_jobs::{BlockingJobKind, run_blocking};

pub(super) async fn load_config_from_disk() -> Result<(Box<ConfigDocument>, Option<String>), String>
{
    run_blocking(BlockingJobKind::ConfigLoad, || {
        ConfigDocument::load_for_editing()
            .map(|(document, warning)| (Box::new(document), warning))
            .map_err(|err| format!("{err:#}"))
    })
    .await
}

/// Writes the configuration and hands the document back either way.
///
/// The model gave its only copy away for the duration of the write, so a
/// refused or failed write has to return one: the outcome's document on
/// success, the borrowed one on failure. The single case with nothing to
/// return is a blocking job that did not come back at all, and the document it
/// held went with it.
pub(super) async fn save_config_to_disk(
    document: Box<ConfigDocument>,
    config: Config,
) -> ConfigSaveResult {
    let outcome = run_blocking(BlockingJobKind::ConfigSave, move || {
        match document.save_with_backup(config) {
            Ok(outcome) => {
                let (saved, backup) = outcome.into_parts();
                Ok((Box::new(saved), Ok(backup)))
            }
            Err(err) => Ok((document, Err(format!("{err:#}")))),
        }
    })
    .await;

    match outcome {
        Ok((document, Ok(backup))) => Ok((backup, document)),
        Ok((document, Err(err))) => Err((Some(document), err)),
        Err(err) => Err((None, err)),
    }
}
