mod blocking_jobs;
mod component;
mod daemon_setup;
mod daemon_workflow;
mod document_workflow;
mod effects;
mod io;
mod migration_workflow;
mod pages;
mod search;
mod session_catalog;
mod shortcut_workflow;
mod startup;
mod state;
mod update;

pub(crate) use component::run;
