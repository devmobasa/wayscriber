//! What the About dialog says, independent of how it is painted.
//!
//! Everything here is pure: the dialog's text is derived from build metadata
//! and the cached update status, so it can be asserted in tests without a
//! compositor.

use crate::update_check::{
    AvailableUpdate, CachedStatus, DEFAULT_NOTES_URL, Freshness, update_instructions_url,
};

/// wayscriber.com is the only outbound link: the site carries the docs,
/// release notes, and per-distro update instructions, so the dialog does not
/// need to send people to a code host to find them.
const WEBSITE_URL: &str = "https://wayscriber.com";
const DOCS_URL: &str = "https://wayscriber.com/docs/";

/// What activating a focusable element does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AboutAction {
    OpenUrl(String),
    CopyText(String),
    CheckForUpdates,
    Close,
}

/// A labelled pill button in the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ButtonSpec {
    pub(super) label: &'static str,
    pub(super) action: AboutAction,
}

/// One tappable link row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkRow {
    pub(super) title: &'static str,
    pub(super) detail: String,
    pub(super) url: String,
}

/// Live state of the update row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UpdateState {
    /// This build cannot check (`WAYSCRIBER_NO_UPDATE_CHECK` at compile time).
    Unavailable,
    /// Nothing checked yet on this machine.
    Unknown(Freshness),
    /// A check is in flight (the dialog blocks for at most a few seconds).
    Checking,
    UpToDate(Freshness),
    Available {
        update: Box<AvailableUpdate>,
        freshness: Freshness,
    },
    Failed(String),
}

impl UpdateState {
    /// Seed the row from the cache so opening About costs no network.
    pub(super) fn from_cache(status: CachedStatus) -> Self {
        if crate::update_check::compiled_out() {
            return Self::Unavailable;
        }
        Self::from_status(status)
    }

    /// The cache mapping on its own, so it can be asserted in either build.
    fn from_status(status: CachedStatus) -> Self {
        match status {
            CachedStatus::Never(freshness) => Self::Unknown(freshness),
            CachedStatus::UpToDate(freshness) => Self::UpToDate(freshness),
            CachedStatus::Update { update, freshness } => Self::Available {
                update: Box::new(update),
                freshness,
            },
        }
    }

    /// Headline shown in the update card.
    pub(super) fn headline(&self) -> String {
        match self {
            Self::Unavailable => "Update checks are off in this build".to_string(),
            Self::Unknown(_) => "Update status unknown".to_string(),
            Self::Checking => "Checking for updates…".to_string(),
            Self::UpToDate { .. } => "Wayscriber is up to date".to_string(),
            Self::Available { update, .. } => format!("Version {} is available", update.version),
            Self::Failed(_) => "Update check failed".to_string(),
        }
    }

    /// Supporting line under the headline.
    pub(super) fn detail(&self) -> String {
        match self {
            Self::Unavailable => "Your packager updates Wayscriber".to_string(),
            Self::Unknown(freshness) if freshness.last_attempt_failed => {
                "Last check failed · click to retry".to_string()
            }
            Self::Unknown(_) => "Click to check wayscriber.com".to_string(),
            Self::Checking => "Contacting wayscriber.com".to_string(),
            // A failed retry is named even when an older success is on record:
            // "checked 3 hours ago" alone would hide a check that just failed.
            Self::UpToDate(freshness) => {
                match (freshness.last_attempt_failed, freshness.checked_seconds_ago) {
                    (true, Some(age)) => {
                        format!("Last check failed · verified {}", humanize_age(age))
                    }
                    (true, None) => "Last check failed · click to retry".to_string(),
                    (false, Some(age)) => format!("Checked {}", humanize_age(age)),
                    (false, None) => "Click to check again".to_string(),
                }
            }
            Self::Available { freshness, .. } if freshness.last_attempt_failed => {
                match freshness.checked_seconds_ago {
                    Some(age) => format!("Last check failed · verified {}", humanize_age(age)),
                    None => "Last check failed · showing the last known release".to_string(),
                }
            }
            Self::Available { update, .. } => match update.released.as_deref() {
                Some(released) => format!("Released {released} — see how to update"),
                None => "See how to update".to_string(),
            },
            Self::Failed(reason) => reason.clone(),
        }
    }

    /// The update card is a button except while a check is running — or in a
    /// build that cannot check at all, where it is only a statement.
    pub(super) fn action(&self) -> Option<AboutAction> {
        match self {
            Self::Unavailable | Self::Checking => None,
            Self::Available { update, .. } => Some(AboutAction::OpenUrl(update.update_url.clone())),
            _ => Some(AboutAction::CheckForUpdates),
        }
    }

    pub(super) fn is_update_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

/// Static text of the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AboutContent {
    pub(super) title: &'static str,
    pub(super) tagline: &'static str,
    pub(super) version_line: String,
    pub(super) links: Vec<LinkRow>,
    pub(super) meta_lines: Vec<String>,
    pub(super) commit: Option<String>,
}

impl AboutContent {
    pub(super) fn build() -> Self {
        Self::from_parts(
            crate::build_info::version(),
            crate::build_info::commit_hash(),
            crate::build_info::build_date(),
            crate::build_info::install_source(),
        )
    }

    fn from_parts(
        version: &str,
        commit: &str,
        build_date: Option<&str>,
        install_source: Option<&str>,
    ) -> Self {
        let commit = (commit != "unknown" && !commit.is_empty()).then(|| commit.to_string());

        let mut build_line = match (commit.as_deref(), build_date) {
            (Some(commit), Some(date)) => format!("Commit {commit} · built {date}"),
            (Some(commit), None) => format!("Commit {commit}"),
            (None, Some(date)) => format!("Built {date}"),
            (None, None) => String::new(),
        };
        if let Some(source) = install_source {
            let installed = format!("installed via {source}");
            if build_line.is_empty() {
                build_line = capitalize(&installed);
            } else {
                build_line.push_str(" · ");
                build_line.push_str(&installed);
            }
        }

        let mut meta_lines = Vec::new();
        if !build_line.is_empty() {
            meta_lines.push(build_line);
        }
        meta_lines.push("MIT licensed · made for Wayland".to_string());

        Self {
            title: "Wayscriber",
            tagline: "Screen annotation for Wayland",
            version_line: format!("Version {version}"),
            links: vec![
                LinkRow {
                    title: "Website",
                    detail: "wayscriber.com".to_string(),
                    url: WEBSITE_URL.to_string(),
                },
                LinkRow {
                    title: "Documentation",
                    detail: "Setup, config, troubleshooting".to_string(),
                    url: DOCS_URL.to_string(),
                },
                LinkRow {
                    title: "Release notes",
                    detail: "What changed in each version".to_string(),
                    url: DEFAULT_NOTES_URL.to_string(),
                },
                LinkRow {
                    title: "How to update",
                    detail: "Steps for your install method".to_string(),
                    url: update_instructions_url(),
                },
            ],
            meta_lines,
            commit,
        }
    }
}

impl AboutContent {
    /// Footer buttons. The diagnostics button copies everything a bug report
    /// needs in one go; the commit button stays for the common "which build is
    /// this?" question and disappears in builds with no commit metadata.
    pub(super) fn buttons(&self) -> Vec<ButtonSpec> {
        let mut buttons = Vec::new();
        if let Some(commit) = self.commit.as_deref() {
            buttons.push(ButtonSpec {
                label: "Copy commit",
                action: AboutAction::CopyText(commit.to_string()),
            });
        }
        buttons.push(ButtonSpec {
            label: "Copy diagnostics",
            action: AboutAction::CopyText(super::diagnostics::report()),
        });
        buttons
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Coarse "how long ago" phrasing. Precision beyond this is noise for a check
/// that runs once a day.
pub(super) fn humanize_age(seconds: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    match seconds {
        0..MINUTE => "just now".to_string(),
        MINUTE..HOUR => plural(seconds / MINUTE, "minute"),
        HOUR..DAY => plural(seconds / HOUR, "hour"),
        _ => plural(seconds / DAY, "day"),
    }
}

fn plural(count: u64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_check::DEFAULT_UPDATE_URL;

    fn content() -> AboutContent {
        AboutContent::from_parts("0.9.22", "51113dd1", Some("2026-07-20"), Some("apt"))
    }

    #[test]
    fn links_stay_on_wayscriber_com() {
        for link in content().links {
            assert!(
                link.url.starts_with("https://wayscriber.com"),
                "unexpected link target: {}",
                link.url
            );
        }
    }

    #[test]
    fn build_metadata_collapses_gracefully() {
        let full = content();
        assert_eq!(full.version_line, "Version 0.9.22");
        assert_eq!(
            full.meta_lines[0],
            "Commit 51113dd1 · built 2026-07-20 · installed via apt"
        );
        assert_eq!(full.commit.as_deref(), Some("51113dd1"));

        let bare = AboutContent::from_parts("1.0.0", "unknown", None, None);
        assert_eq!(bare.commit, None);
        assert_eq!(bare.meta_lines, vec!["MIT licensed · made for Wayland"]);

        let packaged = AboutContent::from_parts("1.0.0", "unknown", None, Some("nix"));
        assert_eq!(packaged.meta_lines[0], "Installed via nix");
    }

    #[test]
    fn update_state_maps_from_cache() {
        assert_eq!(
            UpdateState::from_status(CachedStatus::Never(Freshness::default())),
            UpdateState::Unknown(Freshness::default())
        );

        let up_to_date = UpdateState::from_status(CachedStatus::UpToDate(Freshness {
            checked_seconds_ago: Some(7_200),
            last_attempt_failed: false,
        }));
        assert_eq!(up_to_date.headline(), "Wayscriber is up to date");
        assert_eq!(up_to_date.detail(), "Checked 2 hours ago");
        assert!(!up_to_date.is_update_available());

        let update = AvailableUpdate {
            version: "0.9.23".to_string(),
            released: Some("2026-07-20".to_string()),
            update_url: DEFAULT_UPDATE_URL.to_string(),
            notes_url: DEFAULT_NOTES_URL.to_string(),
        };
        let available = UpdateState::from_status(CachedStatus::Update {
            update: update.clone(),
            freshness: Freshness {
                checked_seconds_ago: Some(60),
                last_attempt_failed: false,
            },
        });
        assert_eq!(available.headline(), "Version 0.9.23 is available");
        assert!(available.detail().contains("2026-07-20"));
        assert!(available.is_update_available());
        assert_eq!(
            available.action(),
            Some(AboutAction::OpenUrl(DEFAULT_UPDATE_URL.to_string()))
        );
    }

    /// The sequence the daemon actually produces: one good check, then a failed
    /// one. The card must not read as a clean "Checked N ago".
    #[test]
    fn a_failed_retry_after_a_success_is_named_not_hidden() {
        let after_failure = UpdateState::from_status(CachedStatus::UpToDate(Freshness {
            checked_seconds_ago: Some(7_200),
            last_attempt_failed: true,
        }));
        assert_eq!(after_failure.headline(), "Wayscriber is up to date");
        assert_eq!(
            after_failure.detail(),
            "Last check failed · verified 2 hours ago"
        );
        // Still a button, so the user can retry from the dialog.
        assert_eq!(
            after_failure.action(),
            Some(AboutAction::CheckForUpdates),
            "a failed retry must stay retryable"
        );

        // No success on record at all: the age is not invented.
        let never_succeeded = UpdateState::from_status(CachedStatus::Never(Freshness {
            checked_seconds_ago: None,
            last_attempt_failed: true,
        }));
        assert_eq!(never_succeeded.headline(), "Update status unknown");
        assert_eq!(
            never_succeeded.detail(),
            "Last check failed · click to retry"
        );
    }

    #[test]
    fn an_available_update_does_not_hide_a_failed_retry() {
        let update = AvailableUpdate {
            version: "0.9.23".to_string(),
            released: Some("2026-07-20".to_string()),
            update_url: DEFAULT_UPDATE_URL.to_string(),
            notes_url: DEFAULT_NOTES_URL.to_string(),
        };
        let after_failure = UpdateState::from_status(CachedStatus::Update {
            update,
            freshness: Freshness {
                checked_seconds_ago: Some(7_200),
                last_attempt_failed: true,
            },
        });

        assert_eq!(
            after_failure.detail(),
            "Last check failed · verified 2 hours ago"
        );
        assert_eq!(
            after_failure.action(),
            Some(AboutAction::OpenUrl(DEFAULT_UPDATE_URL.to_string()))
        );
    }

    /// In a build with the check compiled out, the card states that and stops
    /// being a button — whatever the cache happens to hold.
    #[test]
    fn a_compiled_out_build_shows_an_inert_card() {
        let seeded = UpdateState::from_cache(CachedStatus::Never(Freshness::default()));

        if crate::update_check::compiled_out() {
            assert_eq!(seeded, UpdateState::Unavailable);
            assert_eq!(seeded.action(), None);
            assert!(seeded.headline().contains("off in this build"));
        } else {
            assert_eq!(seeded, UpdateState::Unknown(Freshness::default()));
            assert_eq!(seeded.action(), Some(AboutAction::CheckForUpdates));
        }
    }

    #[test]
    fn a_running_check_is_not_clickable() {
        assert_eq!(UpdateState::Checking.action(), None);
        assert_eq!(
            UpdateState::Unknown(Freshness::default()).action(),
            Some(AboutAction::CheckForUpdates)
        );
        assert_eq!(
            UpdateState::Failed("offline".to_string()).action(),
            Some(AboutAction::CheckForUpdates)
        );
    }

    #[test]
    fn ages_are_humanized_in_coarse_units() {
        assert_eq!(humanize_age(0), "just now");
        assert_eq!(humanize_age(59), "just now");
        assert_eq!(humanize_age(60), "1 minute ago");
        assert_eq!(humanize_age(3_599), "59 minutes ago");
        assert_eq!(humanize_age(3_600), "1 hour ago");
        assert_eq!(humanize_age(86_399), "23 hours ago");
        assert_eq!(humanize_age(86_400), "1 day ago");
        assert_eq!(humanize_age(3 * 86_400), "3 days ago");
    }
}
