use iced::Task;

use crate::messages::Message;

use super::super::blocking_jobs::{
    BlockingJobId, BlockingJobOutput, BlockingJobPurpose, BlockingJobTransition,
};
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_blocking_job_ready(&mut self, id: BlockingJobId) -> Task<Message> {
        let update = self.blocking_jobs.handle_ready(id);
        let handled = match update.transition {
            BlockingJobTransition::Completed(output) => self.handle_blocking_job_output(output),
            BlockingJobTransition::Failed { purpose, failure } => {
                let description = failure.describe(purpose);
                self.handle_blocking_job_failure(purpose, description)
            }
            BlockingJobTransition::Canceled { purpose } => {
                let _ = purpose;
                Task::none()
            }
            BlockingJobTransition::Stale { id } => {
                let _ = id;
                Task::none()
            }
        };
        let started = update.started.map(Message::BlockingJobReady);
        Task::batch([handled, started])
    }

    fn handle_blocking_job_output(&mut self, output: BlockingJobOutput) -> Task<Message> {
        match output {
            #[cfg(test)]
            BlockingJobOutput::Fixture(_) => Task::none(),
            BlockingJobOutput::ConfigLoaded(result) => self.handle_config_loaded(result),
            BlockingJobOutput::ConfigSaved { document, outcome } => {
                self.handle_config_saved(document, outcome)
            }
            BlockingJobOutput::DaemonStatusLoaded {
                preserve_feedback,
                result,
            } => self.handle_daemon_status_loaded(preserve_feedback, result),
            BlockingJobOutput::DaemonActionCompleted(result) => {
                self.handle_daemon_action_completed(result)
            }
            BlockingJobOutput::SessionCatalogLoaded(result) => {
                self.handle_session_catalog_loaded(result)
            }
            BlockingJobOutput::SessionCatalogActionCompleted(result) => {
                self.handle_session_catalog_action_completed(result)
            }
        }
    }

    fn handle_blocking_job_failure(
        &mut self,
        purpose: BlockingJobPurpose,
        description: String,
    ) -> Task<Message> {
        match purpose {
            BlockingJobPurpose::ConfigLoad => self.handle_config_loaded(Err(description)),
            BlockingJobPurpose::ConfigSave => {
                self.base_document.fail_save();
                self.status = StatusMessage::error(format!(
                    "Failed to save configuration: {description}. Reload before saving again."
                ));
                Task::none()
            }
            BlockingJobPurpose::DaemonStatus { preserve_feedback } => {
                self.handle_daemon_status_loaded(preserve_feedback, Err(description))
            }
            BlockingJobPurpose::DaemonAction => {
                self.handle_daemon_action_completed(Err(description))
            }
            BlockingJobPurpose::SessionCatalogLoad => {
                self.handle_session_catalog_loaded(Err(description))
            }
            BlockingJobPurpose::SessionCatalogMutation => {
                self.handle_session_catalog_action_completed(Err(description))
            }
        }
    }
}
