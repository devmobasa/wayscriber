use std::path::Path;

use crate::test_temp::TempDir;

use super::{ConfigStore, PRIMARY_CONFIG_DIR};

pub(crate) fn with_temp_config_home<F, T>(f: F) -> T
where
    F: FnOnce(&Path) -> T,
{
    let temp = TempDir::new().expect("tempdir");
    f(temp.path())
}

pub(crate) fn test_config_store(config_root: &Path) -> ConfigStore {
    ConfigStore::at_path(config_root.join(PRIMARY_CONFIG_DIR).join("config.toml"))
}
