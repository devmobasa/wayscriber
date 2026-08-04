use super::*;

/// Runs one effect as a Relm4 command; its result re-enters the component
/// as an ordinary message through `update_cmd`.
pub(super) fn spawn_effect(effect: Effect, sender: &ComponentSender<ConfiguratorApp>) {
    match effect {
        Effect::LoadConfig => sender.oneshot_command(async {
            CommandMessage::ConfigLoaded(io::load_config_from_disk().await)
        }),
        Effect::SaveConfig { document, config } => sender.oneshot_command(async move {
            CommandMessage::ConfigSaved(io::save_config_to_disk(document, *config).await)
        }),
        Effect::LoadDaemonStatus { request_id } => sender.oneshot_command(async move {
            CommandMessage::DaemonStatusLoaded(
                request_id,
                daemon_setup::load_daemon_runtime_status().await,
            )
        }),
        Effect::PerformDaemonAction {
            action,
            shortcut_input,
        } => sender.oneshot_command(async move {
            CommandMessage::DaemonActionCompleted(
                daemon_setup::perform_daemon_action(action, shortcut_input).await,
            )
        }),
        Effect::LoadSessionCatalog => sender.oneshot_command(async {
            CommandMessage::SessionCatalogLoaded(session_catalog::load_session_catalog().await)
        }),
        Effect::ForgetSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::forget_session_catalog_entry(id).await,
            )
        }),
        Effect::RenameSessionEntry { id, display_name } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::rename_session_catalog_entry(id, display_name).await,
            )
        }),
        Effect::DuplicateSessionEntry { id, target } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::duplicate_session_catalog_entry(id, target).await,
            )
        }),
        Effect::MoveSessionEntry { id, target } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::move_session_catalog_entry(id, target).await,
            )
        }),
        Effect::RevealSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::reveal_session_catalog_entry(id).await,
            )
        }),
        Effect::ClearSessionToolState { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::clear_session_catalog_tool_state_entry(id).await,
            )
        }),
        Effect::ClearSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::clear_session_catalog_entry(id).await,
            )
        }),
    }
}
