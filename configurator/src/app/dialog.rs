//! Confirmation lifecycle: which question is on screen, and what a refresh
//! has to do about it.
//!
//! A destructive action asks before it acts. Asking is model state — a
//! pending session id, a pending defaults reset — and answering is a
//! message. Presentation sits between the two: something has to go up when
//! the model accepts a question and come down when the model stops holding
//! it, exactly once each, and neither step may arrive back at the update
//! layer as an answer nobody gave.
//!
//! That step lives here, and it deliberately knows no widgets.
//!
//! - [`reconcile`] compares the identity on screen against the identity the
//!   model accepted, and reports the difference over that identity. Both
//!   libadwaita channels consume the same answer: the baseline reveals and
//!   hides the inline Confirm/Cancel controls, the modern channel presents
//!   and closes an `AdwAlertDialog`. Neither branch is written here, which
//!   is what lets one reducer be the live, tested rule in both builds.
//! - [`Confirmation::message_for`] maps a response id onto the message the
//!   consumer armed, so the channel that answers by name and the channel
//!   that answers by button reach the same two messages by the same rule.
//!
//! Identity is whatever tells one question from another for that consumer:
//! the session id for a per-row clear, and nothing at all for a reset the
//! app can only have one of. [`reconcile`] is generic over it, so a consumer
//! whose questions cannot be told apart is a consumer whose identity type
//! has one value — not a special case in the table.

use crate::messages::Message;

/// The response id that confirms.
///
/// Public because both channels name their answers with it: the modern
/// channel as an `AdwAlertDialog` response, the baseline channel as the
/// answer its Confirm button reports.
pub(crate) const CONFIRM_RESPONSE: &str = "confirm";

/// The response id that cancels, and the one a dialog reports when it closes
/// without an answer (Escape, the system close control).
pub(crate) const CANCEL_RESPONSE: &str = "cancel";

/// A confirmation a consumer has armed: how the answer that acts is labeled,
/// and which message each answer sends.
///
/// The heading and body a dialog shows are not here on purpose. They are
/// text one channel needs and the other has no place for — the baseline
/// channel already carries the same warning on the status line, put there by
/// the model — so they travel as arguments to the constructor that uses
/// them, not as state every channel has to hold.
#[derive(Debug, Clone)]
pub(crate) struct Confirmation {
    confirm_label: String,
    confirmed: Message,
    canceled: Message,
}

impl Confirmation {
    pub(crate) fn new(
        confirm_label: impl Into<String>,
        confirmed: Message,
        canceled: Message,
    ) -> Self {
        Self {
            confirm_label: confirm_label.into(),
            confirmed,
            canceled,
        }
    }

    /// Label of the control that acts — the destructive half of the pair.
    pub(crate) fn confirm_label(&self) -> &str {
        &self.confirm_label
    }

    /// The message a response id sends.
    ///
    /// Only [`CONFIRM_RESPONSE`] confirms. Escape, the window's close
    /// control, and any response id a channel grows later all read as the
    /// cancel, which is the safe answer to a destructive question and the
    /// one the model treats as withdrawing it.
    pub(crate) fn message_for(&self, response: &str) -> Message {
        if response == CONFIRM_RESPONSE {
            self.confirmed.clone()
        } else {
            self.canceled.clone()
        }
    }
}

/// What a refresh has to do to the confirmation on screen.
///
/// Channel-neutral by construction: it names identities, never widgets, so
/// the same value drives an inline reveal and a dialog present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DialogTransition<Id> {
    /// Nothing to do: the question on screen is the accepted one, or there is
    /// neither. This is the answer to every repeat refresh while a question
    /// stands, which is what makes presenting happen once per question
    /// rather than once per refresh.
    Unchanged,
    /// Put this identity on screen; nothing is on screen now.
    Present(Id),
    /// Take this identity off screen: the model no longer holds it. This is
    /// reconciliation, not an answer — the channel closes silently, because
    /// the model has already moved past the question.
    Close(Id),
    /// Both, in this order. The question on screen was overtaken by another
    /// one, so the old one comes down before the new one goes up and the
    /// channel is never holding two at once.
    Replace { close: Id, present: Id },
}

/// Compares the confirmation on screen with the one the model accepted.
///
/// `accepted` is deliberately the model's own pending state, never the
/// request that asked for it: a request the model refused never becomes
/// pending, so it never reaches this table and never presents.
pub(crate) fn reconcile<Id>(presented: Option<&Id>, accepted: Option<&Id>) -> DialogTransition<Id>
where
    Id: Clone + PartialEq,
{
    match (presented, accepted) {
        (None, None) => DialogTransition::Unchanged,
        (None, Some(accepted)) => DialogTransition::Present(accepted.clone()),
        (Some(presented), None) => DialogTransition::Close(presented.clone()),
        (Some(presented), Some(accepted)) if presented == accepted => DialogTransition::Unchanged,
        (Some(presented), Some(accepted)) => DialogTransition::Replace {
            close: presented.clone(),
            present: accepted.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::{DaemonRuntimeStatus, DesktopEnvironment, LightShortcutApplyCapability};
    use crate::models::{SessionCatalogState, ShortcutApplyCapability, ShortcutBackend};

    use super::super::effects::Effect;
    use super::super::state::{ConfiguratorApp, StatusMessage};

    /// A consumer with one question and no way to tell two of them apart —
    /// the shape the Defaults reset uses.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Solitary;

    fn id(value: &str) -> String {
        value.to_string()
    }

    // ---- The four rows of the transition table ---------------------------

    #[test]
    fn an_accepted_question_with_nothing_on_screen_is_presented() {
        assert_eq!(
            reconcile(None, Some(&id("a"))),
            DialogTransition::Present(id("a"))
        );
    }

    #[test]
    fn the_question_already_on_screen_is_left_alone() {
        assert_eq!(
            reconcile(Some(&id("a")), Some(&id("a"))),
            DialogTransition::Unchanged
        );
    }

    #[test]
    fn a_question_the_model_no_longer_holds_is_closed() {
        assert_eq!(
            reconcile(Some(&id("a")), None),
            DialogTransition::Close(id("a"))
        );
    }

    #[test]
    fn a_new_question_closes_the_old_one_before_it_goes_up() {
        assert_eq!(
            reconcile(Some(&id("a")), Some(&id("b"))),
            DialogTransition::Replace {
                close: id("a"),
                present: id("b"),
            }
        );
    }

    #[test]
    fn nothing_on_either_side_is_nothing_to_do() {
        assert_eq!(reconcile::<String>(None, None), DialogTransition::Unchanged);
    }

    /// The same table with an identity that has a single value: a consumer
    /// that can only ever ask one question still gets present-once and a
    /// silent reconcile close, without a rule of its own.
    #[test]
    fn a_single_valued_identity_uses_the_same_table() {
        assert_eq!(
            reconcile(None, Some(&Solitary)),
            DialogTransition::Present(Solitary)
        );
        assert_eq!(
            reconcile(Some(&Solitary), Some(&Solitary)),
            DialogTransition::Unchanged
        );
        assert_eq!(
            reconcile(Some(&Solitary), None),
            DialogTransition::Close(Solitary)
        );
    }

    // ---- Present-once ----------------------------------------------------

    /// What an owner does with the outputs, run as a refresh loop: an armed
    /// question is presented on the refresh that first sees it and on no
    /// later one, however many refreshes the rest of the app causes.
    #[test]
    fn repeated_refreshes_present_one_armed_question_once() {
        let mut presented: Option<String> = None;
        let mut presents = 0_u32;
        let mut closes = 0_u32;

        let accepted = Some(id("a"));
        for _ in 0..5 {
            match reconcile(presented.as_ref(), accepted.as_ref()) {
                DialogTransition::Unchanged => {}
                DialogTransition::Close(_) => {
                    closes += 1;
                    presented = None;
                }
                DialogTransition::Present(next)
                | DialogTransition::Replace { present: next, .. } => {
                    presents += 1;
                    presented = Some(next);
                }
            }
        }

        assert_eq!(presents, 1);
        assert_eq!(closes, 0);
        assert_eq!(presented.as_deref(), Some("a"));
    }

    /// The other half of present-once: once the model drops the question, the
    /// refresh that notices closes it, and every refresh after that is idle.
    #[test]
    fn a_withdrawn_question_closes_once_and_then_stays_quiet() {
        let mut presented = Some(id("a"));
        let mut closes = 0_u32;

        for _ in 0..5 {
            match reconcile::<String>(presented.as_ref(), None) {
                DialogTransition::Unchanged => {}
                DialogTransition::Close(_) => {
                    closes += 1;
                    presented = None;
                }
                DialogTransition::Present(next)
                | DialogTransition::Replace { present: next, .. } => {
                    presented = Some(next);
                }
            }
        }

        assert_eq!(closes, 1);
        assert!(presented.is_none());
    }

    // ---- Response mapping ------------------------------------------------

    #[test]
    fn the_confirm_response_sends_the_message_that_acts() {
        let confirmation = Confirmation::new(
            "Confirm Defaults",
            Message::ResetToDefaultsConfirmed,
            Message::ResetToDefaultsCanceled,
        );

        assert!(matches!(
            confirmation.message_for(CONFIRM_RESPONSE),
            Message::ResetToDefaultsConfirmed
        ));
        assert_eq!(confirmation.confirm_label(), "Confirm Defaults");
    }

    /// Every other way a confirmation can end is a cancel: the explicit
    /// response, the close response a dialog reports for Escape or the
    /// system close control, and an id no channel here recognizes.
    #[test]
    fn every_other_response_sends_the_message_that_withdraws() {
        let confirmation = Confirmation::new(
            "Confirm Clear",
            Message::SessionCatalogClearConfirmed(id("s-1")),
            Message::SessionCatalogClearCanceled,
        );

        for response in [CANCEL_RESPONSE, "", "close", "Confirm", "confirm "] {
            assert!(
                matches!(
                    confirmation.message_for(response),
                    Message::SessionCatalogClearCanceled
                ),
                "response {response:?} must withdraw the question"
            );
        }
    }

    /// The identity a confirmation carries rides its message, so the answer
    /// names the same row the question did.
    #[test]
    fn a_per_row_confirmation_answers_for_its_own_row() {
        let confirmation = Confirmation::new(
            "Confirm Clear",
            Message::SessionCatalogClearConfirmed(id("s-2")),
            Message::SessionCatalogClearCanceled,
        );

        let answered = match confirmation.message_for(CONFIRM_RESPONSE) {
            Message::SessionCatalogClearConfirmed(answered) => Some(answered),
            _ => None,
        };
        assert_eq!(answered.as_deref(), Some("s-2"));
    }

    // ---- The reducer against the model it follows ------------------------

    fn inactive_daemon_status() -> DaemonRuntimeStatus {
        DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Unknown,
            shortcut_backend: ShortcutBackend::Manual,
            shortcut_apply_capability: ShortcutApplyCapability::Manual,
            light_shortcut_apply_capability: LightShortcutApplyCapability::Manual,
            systemctl_available: false,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    fn app_with_sessions(ids: &[&str]) -> ConfiguratorApp {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.session_catalog = SessionCatalogState::loading();
        app.session_catalog
            .replace_items(ids.iter().map(|id| catalog_item(id)).collect());
        app.daemon_status = Some(inactive_daemon_status());
        app
    }

    fn status_text(status: &StatusMessage) -> Option<&str> {
        match status {
            StatusMessage::Idle => None,
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Warning(text)
            | StatusMessage::Error(text) => Some(text.as_str()),
        }
    }

    fn catalog_item(id: &str) -> crate::models::SessionCatalogItem {
        crate::models::SessionCatalogItem {
            id: id.to_string(),
            display_name: format!("Session {id}"),
            path: std::path::PathBuf::from(format!("/tmp/{id}.wayscriber-session")),
            path_label: format!("/tmp/{id}.wayscriber-session"),
            canonical_path_label: None,
            created_label: "now".to_string(),
            last_opened_label: "Never".to_string(),
            last_saved_label: "Never".to_string(),
            artifacts: crate::models::session::SessionArtifactSummary {
                primary_exists: true,
                backup_exists: false,
                recovery_exists: false,
                clear_marker_exists: false,
                lock_exists: false,
                non_lock_size_bytes: 1024,
            },
        }
    }

    /// Confirming consumes the pending id, so the very next refresh reads the
    /// `A -> none` row and takes the question down. The close is silent by
    /// construction: the transition carries no message, so nothing answers a
    /// question the model already acted on.
    #[test]
    fn a_confirmed_clear_takes_its_question_down_through_the_close_row() {
        let mut app = app_with_sessions(&["s-1"]);
        let _ = app.update_message(Message::SessionCatalogClearRequested(id("s-1")));
        let presented = app.session_catalog.pending_clear_id.clone();
        assert_eq!(
            reconcile(None, presented.as_ref()),
            DialogTransition::Present(id("s-1"))
        );

        let effects = app.update_message(Message::SessionCatalogClearConfirmed(id("s-1")));

        assert!(app.session_catalog.pending_clear_id.is_none());
        assert!(matches!(
            effects.as_slice(),
            [Effect::ClearSessionEntry { id }] if id == "s-1"
        ));
        assert_eq!(
            reconcile(
                presented.as_ref(),
                app.session_catalog.pending_clear_id.as_ref()
            ),
            DialogTransition::Close(id("s-1"))
        );
    }

    /// Arming a second row while the first is still armed is one transition,
    /// not two independent ones: the row that was up comes down and exactly
    /// the newly accepted row goes up.
    #[test]
    fn arming_another_row_leaves_exactly_the_new_one_presented() {
        let mut app = app_with_sessions(&["s-1", "s-2"]);
        let _ = app.update_message(Message::SessionCatalogClearRequested(id("s-1")));
        let mut presented = app.session_catalog.pending_clear_id.clone();

        let _ = app.update_message(Message::SessionCatalogClearRequested(id("s-2")));

        let transition = reconcile(
            presented.as_ref(),
            app.session_catalog.pending_clear_id.as_ref(),
        );
        assert_eq!(
            transition,
            DialogTransition::Replace {
                close: id("s-1"),
                present: id("s-2"),
            }
        );

        match transition {
            DialogTransition::Present(next) | DialogTransition::Replace { present: next, .. } => {
                presented = Some(next);
            }
            DialogTransition::Close(_) => presented = None,
            DialogTransition::Unchanged => {}
        }
        assert_eq!(presented.as_deref(), Some("s-2"));
        assert_eq!(
            reconcile(
                presented.as_ref(),
                app.session_catalog.pending_clear_id.as_ref()
            ),
            DialogTransition::Unchanged
        );
    }

    /// Reconciling is a read: the status and the effects the handler produced
    /// are what the refresh finds, and running the reducer over them leaves
    /// both exactly as the newer transition wrote them.
    #[test]
    fn reconciling_leaves_the_status_and_effects_the_handler_produced() {
        let mut app = app_with_sessions(&["s-1"]);
        let _ = app.update_message(Message::SessionCatalogClearRequested(id("s-1")));
        assert!(matches!(app.status, StatusMessage::Warning(_)));
        let presented = app.session_catalog.pending_clear_id.clone();

        let effects = app.update_message(Message::SessionCatalogClearConfirmed(id("s-1")));

        let status_after_handler = status_text(&app.status).map(str::to_string);
        let transition = reconcile(
            presented.as_ref(),
            app.session_catalog.pending_clear_id.as_ref(),
        );

        assert_eq!(transition, DialogTransition::Close(id("s-1")));
        assert_eq!(status_text(&app.status), status_after_handler.as_deref());
        assert!(matches!(app.status, StatusMessage::Info(_)));
        assert!(app.session_catalog.busy);
        assert!(matches!(
            effects.as_slice(),
            [Effect::ClearSessionEntry { id }] if id == "s-1"
        ));
    }

    /// The Defaults question follows the same path with an identity that has
    /// one value, and a request the model refuses never reaches the table.
    #[test]
    fn the_defaults_question_presents_only_while_the_model_holds_it() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let armed = |app: &ConfiguratorApp| app.defaults_reset_pending.then_some(Solitary);

        // Refused: the initial load is still in flight, so nothing is armed
        // and nothing is presented.
        assert!(app.is_loading);
        let _ = app.update_message(Message::ResetToDefaultsRequested);
        assert_eq!(
            reconcile(None, armed(&app).as_ref()),
            DialogTransition::Unchanged
        );

        app.is_loading = false;
        let _ = app.update_message(Message::ResetToDefaultsRequested);
        assert_eq!(
            reconcile(None, armed(&app).as_ref()),
            DialogTransition::Present(Solitary)
        );
        // A repeat press is not a second question.
        let _ = app.update_message(Message::ResetToDefaultsRequested);
        assert_eq!(
            reconcile(Some(&Solitary), armed(&app).as_ref()),
            DialogTransition::Unchanged
        );

        let _ = app.update_message(Message::ResetToDefaultsConfirmed);
        assert!(!app.defaults_reset_pending);
        assert_eq!(
            reconcile(Some(&Solitary), armed(&app).as_ref()),
            DialogTransition::Close(Solitary)
        );
    }
}
