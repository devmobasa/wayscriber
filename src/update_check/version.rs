//! Minimal semver-style version ordering.
//!
//! Wayscriber only ever compares its own release numbers, so a full semver
//! implementation (and its dependency) is not warranted: parse
//! `major[.minor[.patch]]` with an optional `-prerelease` suffix, ignore build
//! metadata, and order pre-releases below the matching release.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Version {
    core: [u64; 3],
    /// Dot-separated pre-release identifiers; `None` for a final release.
    pre: Option<Vec<String>>,
}

impl Version {
    /// Parse a release string, tolerating a `v` prefix and surrounding space.
    /// Returns `None` for anything that is not `1`, `1.2`, or `1.2.3` (with an
    /// optional `-pre` suffix), so malformed manifests never produce a
    /// bogus "update available".
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches(['v', 'V']);
        if trimmed.is_empty() {
            return None;
        }
        // Build metadata never participates in ordering.
        let without_build = trimmed.split('+').next()?;
        let (core_part, pre_part) = match without_build.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (without_build, None),
        };

        let mut core = [0u64; 3];
        let mut segments = 0usize;
        for (index, segment) in core_part.split('.').enumerate() {
            if index >= 3 || segment.is_empty() {
                return None;
            }
            core[index] = segment.parse::<u64>().ok()?;
            segments = index + 1;
        }
        if segments == 0 {
            return None;
        }

        let pre = match pre_part {
            Some("") => return None,
            Some(pre) => {
                let identifiers: Vec<String> = pre.split('.').map(str::to_string).collect();
                if identifiers.iter().any(String::is_empty) {
                    return None;
                }
                Some(identifiers)
            }
            None => None,
        };

        Some(Self { core, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| {
            match (self.pre.as_deref(), other.pre.as_deref()) {
                // A final release outranks any pre-release of the same core.
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => compare_prerelease(left, right),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_prerelease(left: &[String], right: &[String]) -> Ordering {
    for (left_id, right_id) in left.iter().zip(right.iter()) {
        let ordering = match (left_id.parse::<u64>(), right_id.parse::<u64>()) {
            (Ok(left_num), Ok(right_num)) => left_num.cmp(&right_num),
            // Numeric identifiers rank below alphanumeric ones (semver 11.4.3).
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left_id.cmp(right_id),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

/// True when `candidate` is a strictly newer release than `current`.
/// Unparseable input on either side answers `false`: we never nag on a version
/// string we do not understand.
pub(crate) fn is_newer(candidate: &str, current: &str) -> bool {
    match (Version::parse(candidate), Version::parse(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_release_forms() {
        assert_eq!(Version::parse("0.9.22").unwrap().core, [0, 9, 22]);
        assert_eq!(Version::parse("v1.0.0").unwrap().core, [1, 0, 0]);
        assert_eq!(Version::parse(" 1.2 ").unwrap().core, [1, 2, 0]);
        assert_eq!(Version::parse("2").unwrap().core, [2, 0, 0]);
        assert_eq!(
            Version::parse("1.2.3-rc.1").unwrap().pre,
            Some(vec!["rc".to_string(), "1".to_string()])
        );
        // Build metadata is stripped before comparison.
        assert_eq!(Version::parse("1.2.3+build.7").unwrap().core, [1, 2, 3]);
    }

    #[test]
    fn rejects_malformed_versions() {
        for raw in ["", "  ", "abc", "1.2.3.4", "1..2", "1.x", "1.2.3-", "-1.0"] {
            assert!(Version::parse(raw).is_none(), "expected {raw:?} to fail");
        }
    }

    #[test]
    fn orders_by_numeric_core() {
        assert!(is_newer("0.9.23", "0.9.22"));
        assert!(is_newer("0.10.0", "0.9.99"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.9.22", "0.9.22"));
        assert!(!is_newer("0.9.21", "0.9.22"));
    }

    #[test]
    fn orders_prereleases_below_their_release() {
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0"));
        assert!(is_newer("1.0.0-rc.2", "1.0.0-rc.1"));
        assert!(is_newer("1.0.0-beta", "1.0.0-1"));
        assert!(is_newer("1.0.0-rc.1.1", "1.0.0-rc.1"));
    }

    #[test]
    fn unparseable_input_never_reports_an_update() {
        assert!(!is_newer("not-a-version", "0.9.22"));
        assert!(!is_newer("9.9.9", "garbage"));
    }
}
