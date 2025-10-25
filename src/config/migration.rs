use super::{legacy_config_dir, primary_config_dir};
use anyhow::{Context, Result, anyhow};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Describes the actions taken (or planned) while migrating configuration files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationActions {
    /// No legacy configuration directory was found; nothing to do.
    NoLegacyConfig,
    /// Dry-run summary with metadata about what would happen.
    DryRun {
        target_exists: bool,
        files_to_copy: usize,
    },
    /// Migration completed successfully.
    Migrated {
        target_existed: bool,
        files_copied: usize,
        backup_path: Option<PathBuf>,
    },
}

/// Report returned after attempting to migrate configuration files.
#[derive(Debug, Clone)]
pub struct MigrationReport {
    pub legacy_dir: PathBuf,
    pub target_dir: PathBuf,
    pub actions: MigrationActions,
}

/// Copies configuration files from the legacy hyprmarker directory into the new Wayscriber
/// directory. When `dry_run` is true the function only reports what would happen.
pub fn migrate_config(dry_run: bool) -> Result<MigrationReport> {
    let legacy_dir = legacy_config_dir()?;
    let target_dir = primary_config_dir()?;

    if !legacy_dir.exists() {
        return Ok(MigrationReport {
            legacy_dir,
            target_dir,
            actions: MigrationActions::NoLegacyConfig,
        });
    }

    let target_exists = target_dir.exists();
    let files_to_copy = copy_directory(&legacy_dir, &target_dir, true)?;

    if dry_run {
        return Ok(MigrationReport {
            legacy_dir: legacy_dir.clone(),
            target_dir: target_dir.clone(),
            actions: MigrationActions::DryRun {
                target_exists,
                files_to_copy,
            },
        });
    }

    let mut backup_path = None;
    let target_existed = target_exists;

    if target_existed {
        let timestamp = Local::now().format("%Y%m%d%H%M%S");
        let candidate = target_dir
            .parent()
            .map(|parent| parent.join(format!("wayscriber.backup.{timestamp}")))
            .ok_or_else(|| anyhow!("Could not determine parent directory for {:?}", target_dir))?;

        if candidate.exists() {
            return Err(anyhow!(
                "Backup directory already exists at {}",
                candidate.display()
            ));
        }

        fs::create_dir_all(candidate.parent().unwrap()).with_context(|| {
            format!("Failed to prepare backup directory {}", candidate.display())
        })?;
        fs::rename(&target_dir, &candidate).with_context(|| {
            format!(
                "Failed to move existing directory {} to {}",
                target_dir.display(),
                candidate.display()
            )
        })?;
        backup_path = Some(candidate);
    }

    let files_copied = copy_directory(&legacy_dir, &target_dir, false)?;

    Ok(MigrationReport {
        legacy_dir,
        target_dir,
        actions: MigrationActions::Migrated {
            target_existed,
            files_copied,
            backup_path,
        },
    })
}

fn copy_directory(src: &Path, dest: &Path, dry_run: bool) -> Result<usize> {
    if !dry_run {
        fs::create_dir_all(dest)
            .with_context(|| format!("Failed to create directory {}", dest.display()))?;
    }

    let mut file_count = 0usize;

    for entry in
        fs::read_dir(src).with_context(|| format!("Failed to list directory {}", src.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", entry_path.display()))?;

        if file_type.is_dir() {
            file_count += copy_directory(&entry_path, &dest_path, dry_run)?;
        } else {
            file_count += 1;
            if !dry_run {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create parent directory {}", parent.display())
                    })?;
                }

                fs::copy(&entry_path, &dest_path).with_context(|| {
                    format!(
                        "Failed to copy {} to {}",
                        entry_path.display(),
                        dest_path.display()
                    )
                })?;
            }
        }
    }

    Ok(file_count)
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::with_temp_config_home;
    use super::*;
    use std::fs;

    #[test]
    fn migrate_copies_legacy_into_new_directory() {
        with_temp_config_home(|config_root| {
            let legacy_dir = config_root.join("hyprmarker");
            fs::create_dir_all(&legacy_dir).unwrap();
            fs::write(legacy_dir.join("config.toml"), "legacy = true").unwrap();
            fs::write(legacy_dir.join("extra.txt"), "payload").unwrap();

            let report = migrate_config(false).expect("migration succeeds");
            let target = report.target_dir.join("config.toml");

            assert!(target.exists(), "target config should be created");
            assert_eq!(
                fs::read_to_string(target).unwrap(),
                "legacy = true",
                "config contents copied"
            );

            assert!(
                matches!(
                    report.actions,
                    MigrationActions::Migrated {
                        target_existed: false,
                        files_copied: 2,
                        backup_path: None
                    }
                ),
                "expected migration report with copied files"
            );
        });
    }

    #[test]
    fn migrate_creates_backup_when_target_exists() {
        with_temp_config_home(|config_root| {
            let legacy_dir = config_root.join("hyprmarker");
            fs::create_dir_all(&legacy_dir).unwrap();
            fs::write(legacy_dir.join("config.toml"), "legacy = true").unwrap();

            let target_dir = config_root.join("wayscriber");
            fs::create_dir_all(&target_dir).unwrap();
            fs::write(target_dir.join("config.toml"), "replacement = false").unwrap();

            let report = migrate_config(false).expect("migration succeeds");

            match report.actions {
                MigrationActions::Migrated {
                    target_existed,
                    files_copied,
                    backup_path: Some(ref backup),
                } => {
                    assert!(target_existed);
                    assert_eq!(files_copied, 1);
                    assert!(backup.exists(), "backup directory should exist");
                    assert!(
                        backup
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with("wayscriber.backup."),
                        "backup directory name should be timestamped"
                    );
                    let backup_file = backup.join("config.toml");
                    assert!(backup_file.exists());
                    assert_eq!(
                        fs::read_to_string(backup_file).unwrap(),
                        "replacement = false"
                    );
                }
                other => panic!("unexpected migration result: {:?}", other),
            }

            assert_eq!(
                fs::read_to_string(report.target_dir.join("config.toml")).unwrap(),
                "legacy = true"
            );
        });
    }

    #[test]
    fn migrate_supports_dry_run() {
        with_temp_config_home(|config_root| {
            let legacy_dir = config_root.join("hyprmarker");
            fs::create_dir_all(&legacy_dir).unwrap();
            fs::write(legacy_dir.join("config.toml"), "legacy = true").unwrap();

            let report = migrate_config(true).expect("dry-run succeeds");

            match report.actions {
                MigrationActions::DryRun {
                    target_exists,
                    files_to_copy,
                } => {
                    assert!(!target_exists);
                    assert_eq!(files_to_copy, 1);
                }
                other => panic!("unexpected dry-run result: {:?}", other),
            }

            let config_dir = primary_config_dir().unwrap();
            assert!(
                !config_dir.exists(),
                "dry-run should not create target directories"
            );
        });
    }
}
