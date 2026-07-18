use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::wire::{
    DaemonRuntimeRecordV2, MAX_RUNTIME_RECORD_BYTES, canonical_json, parse_canonical_json,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClassifiedRuntimeRecord {
    V2(DaemonRuntimeRecordV2),
    LegacyV1 { pid: u32, token: Option<String> },
}

#[derive(Deserialize)]
struct LegacyRuntimeRecord {
    pid: u32,
    #[serde(default)]
    token: Option<String>,
}

pub(crate) fn read_runtime_record(path: &Path) -> Result<ClassifiedRuntimeRecord> {
    let bytes = super::linux::read_bounded_regular_file(path, MAX_RUNTIME_RECORD_BYTES)
        .with_context(|| format!("failed to read daemon runtime record {}", path.display()))?;
    match parse_canonical_json::<DaemonRuntimeRecordV2>(&bytes, MAX_RUNTIME_RECORD_BYTES) {
        Ok(record) => {
            record.validate()?;
            return Ok(ClassifiedRuntimeRecord::V2(record));
        }
        Err(error) if is_v2_shaped(&bytes) => {
            return Err(error).context("invalid daemon v2 runtime record");
        }
        Err(_) => {}
    }
    let text = std::str::from_utf8(&bytes).context("daemon runtime record is not UTF-8")?;
    if let Ok(record) = serde_json::from_str::<LegacyRuntimeRecord>(text) {
        if record.pid == 0 {
            bail!("legacy daemon runtime pid is zero");
        }
        return Ok(ClassifiedRuntimeRecord::LegacyV1 {
            pid: record.pid,
            token: record.token,
        });
    }
    let pid = text
        .trim()
        .parse::<u32>()
        .context("unrecognized daemon runtime record")?;
    if pid == 0 {
        bail!("legacy daemon runtime pid is zero");
    }
    Ok(ClassifiedRuntimeRecord::LegacyV1 { pid, token: None })
}

fn is_v2_shaped(bytes: &[u8]) -> bool {
    let Ok(serde_json::Value::Object(fields)) = serde_json::from_slice(bytes) else {
        return false;
    };
    [
        "runtime_record_version",
        "typed_control_protocol_version",
        "boot_id",
        "time_namespace",
        "pid_namespace",
        "process_start_ticks",
        "v2_instance_token",
    ]
    .iter()
    .any(|field| fields.contains_key(*field))
}

pub(crate) fn write_runtime_record_v2(path: &Path, record: &DaemonRuntimeRecordV2) -> Result<()> {
    record.validate()?;
    let bytes = canonical_json(record, MAX_RUNTIME_RECORD_BYTES)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    crate::durable_io::write_atomic(
        path,
        &bytes,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )
    .with_context(|| {
        format!(
            "failed to publish daemon v2 runtime record {}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::protocol_v2::ProtocolToken;

    #[test]
    fn exact_v2_is_strict_but_v1_remains_permissive() {
        let temp = crate::test_temp::tempdir().unwrap();
        let path = temp.path().join("runtime");
        let v2 = DaemonRuntimeRecordV2::current(ProtocolToken::generate().unwrap()).unwrap();
        write_runtime_record_v2(&path, &v2).unwrap();
        assert_eq!(
            read_runtime_record(&path).unwrap(),
            ClassifiedRuntimeRecord::V2(v2)
        );

        fs::write(&path, r#"{"pid":42,"token":"legacy","future":true}"#).unwrap();
        assert_eq!(
            read_runtime_record(&path).unwrap(),
            ClassifiedRuntimeRecord::LegacyV1 {
                pid: 42,
                token: Some("legacy".into())
            }
        );
    }

    #[test]
    fn malformed_v2_shaped_records_never_downgrade_to_legacy() {
        let temp = crate::test_temp::tempdir().unwrap();
        let path = temp.path().join("runtime");
        for record in [
            r#" {"pid":42,"runtime_record_version":2}"#,
            r#"{"pid":42,"runtime_record_version":2,"future":true}"#,
            r#"{"pid":42,"typed_control_protocol_version":2}"#,
            r#"{"pid":42,"v2_instance_token":"missing-v2-identity"}"#,
        ] {
            fs::write(&path, record).unwrap();
            assert!(
                read_runtime_record(&path).is_err(),
                "v2-shaped record was accepted as legacy: {record}"
            );
        }
    }
}
