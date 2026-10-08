//! Core project records and state vocabulary.
//!
//! Domain validation and transitions belong here, independent of the UI,
//! database, or agent harness.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStage {
    Idea,
    Discovery,
    Planning,
    Design,
    Build,
    Verify,
    Release,
    Operate,
    Paused,
    Archived,
}

impl ProjectStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Discovery => "discovery",
            Self::Planning => "planning",
            Self::Design => "design",
            Self::Build => "build",
            Self::Verify => "verify",
            Self::Release => "release",
            Self::Operate => "operate",
            Self::Paused => "paused",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "idea" => Self::Idea,
            "discovery" => Self::Discovery,
            "planning" => Self::Planning,
            "design" => Self::Design,
            "build" => Self::Build,
            "verify" => Self::Verify,
            "release" => Self::Release,
            "operate" => Self::Operate,
            "paused" => Self::Paused,
            "archived" => Self::Archived,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectHealth {
    OnTrack,
    NeedsInput,
    Blocked,
    AtRisk,
    Failed,
}

impl ProjectHealth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OnTrack => "on_track",
            Self::NeedsInput => "needs_input",
            Self::Blocked => "blocked",
            Self::AtRisk => "at_risk",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "on_track" => Self::OnTrack,
            "needs_input" => Self::NeedsInput,
            "blocked" => Self::Blocked,
            "at_risk" => Self::AtRisk,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub stage: ProjectStage,
    pub health: ProjectHealth,
    pub verified_completion_basis_points: u16,
}
