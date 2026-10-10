//! Harness-neutral session and event contract.

mod opencode;
pub use opencode::{OpenCodeAdapter, OpenCodeServer, SseDecoder};
mod pi_rpc;
pub use pi_rpc::{PiJsonlDecoder, PiRpcInbox};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("harness unavailable: {0}")]
    Unavailable(String),
    #[error("unsupported harness version: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
    #[error("harness protocol error: {0}")]
    Protocol(String),
    #[error("harness request failed: {0}")]
    Transport(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AdapterError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Health {
    Ready,
    VersionMismatch,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub health: Health,
    pub version: Option<String>,
    pub detail: String,
    pub can_resume: bool,
    pub can_steer: bool,
    pub can_stop: bool,
    pub can_stream: bool,
    pub can_diff: bool,
    pub can_reply_permissions: bool,
}

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub run_id: String,
    pub checkout: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHandle {
    pub run_id: String,
    pub session_id: String,
    pub checkout: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Connected,
    Progress,
    Tool,
    PermissionRequest,
    Diff,
    Error,
    Idle,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub external_id: Option<String>,
    pub session_id: String,
    pub kind: EventKind,
    pub summary: String,
    pub permission_id: Option<String>,
    pub permission_class: Option<String>,
    pub permission_target: Option<String>,
    pub permission_digest_material: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryState {
    Busy,
    Idle,
    Disconnected,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    pub session_id: String,
    pub state: RecoveryState,
    pub diff_files: usize,
}

pub trait AgentAdapter {
    fn probe(&self) -> CapabilityReport;
    fn start(&self, request: RunRequest) -> Result<RunHandle>;
    fn events(
        &self,
        run: &RunHandle,
        on_event: &mut dyn FnMut(NormalizedEvent) -> bool,
    ) -> Result<()>;
    fn steer(&self, run: &RunHandle, message: &str) -> Result<()>;
    fn stop(&self, run: &RunHandle) -> Result<()>;
    fn recover(&self, run: &RunHandle) -> Result<RecoveryReport>;
    fn reply_permission(
        &self,
        run: &RunHandle,
        permission_id: &str,
        allow_once: bool,
    ) -> Result<()>;
}
