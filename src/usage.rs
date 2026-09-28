//! Read account quota through the local Codex app-server. Never start a model turn.
mod bridge;
mod discovery;
mod model;
mod service;

pub(crate) use model::State;
#[cfg(any(windows, test))]
pub(crate) use model::Window;
pub(crate) use service::{Action, start_with_notify};
