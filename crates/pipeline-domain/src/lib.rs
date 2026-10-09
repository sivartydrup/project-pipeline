//! Core project records and state vocabulary.
//!
//! Domain validation and transitions belong here, independent of the UI,
//! database, or agent harness.

use serde::{Deserialize, Serialize};

mod plan;
pub use plan::{
    CriterionSpec, DependencyKind, DependencySpec, EpicSpec, MilestoneSpec, PlanContent, PlanError,
    TaskSpec,
};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BriefContent {
    pub idea: String,
    pub audience: String,
    pub problem: String,
    pub desired_outcome: String,
    pub constraints: String,
    pub value_proposition: String,
    pub scope: String,
    pub non_goals: String,
    pub user_journeys: String,
    pub success_metrics: String,
    pub ux_principles: String,
    pub architecture_candidates: String,
    pub costs: String,
    pub risks: String,
    pub milestone_plan: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefChange {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

impl BriefContent {
    pub fn changes_from(&self, previous: &Self) -> Vec<BriefChange> {
        let fields = [
            ("Idea", &previous.idea, &self.idea),
            ("Audience", &previous.audience, &self.audience),
            ("Problem", &previous.problem, &self.problem),
            (
                "Desired outcome",
                &previous.desired_outcome,
                &self.desired_outcome,
            ),
            ("Constraints", &previous.constraints, &self.constraints),
            (
                "Value proposition",
                &previous.value_proposition,
                &self.value_proposition,
            ),
            ("Scope", &previous.scope, &self.scope),
            ("Non-goals", &previous.non_goals, &self.non_goals),
            (
                "User journeys",
                &previous.user_journeys,
                &self.user_journeys,
            ),
            (
                "Success metrics",
                &previous.success_metrics,
                &self.success_metrics,
            ),
            (
                "UX principles",
                &previous.ux_principles,
                &self.ux_principles,
            ),
            (
                "Architecture candidates",
                &previous.architecture_candidates,
                &self.architecture_candidates,
            ),
            ("Costs", &previous.costs, &self.costs),
            ("Risks", &previous.risks, &self.risks),
            (
                "Milestone plan",
                &previous.milestone_plan,
                &self.milestone_plan,
            ),
        ];
        fields
            .into_iter()
            .filter(|(_, before, after)| before != after)
            .map(|(field, before, after)| BriefChange {
                field,
                before: before.clone(),
                after: after.clone(),
            })
            .collect()
    }

    pub fn has_required_intake(&self) -> bool {
        [
            self.idea.as_str(),
            self.audience.as_str(),
            self.problem.as_str(),
            self.desired_outcome.as_str(),
        ]
        .into_iter()
        .all(|value| !value.trim().is_empty())
    }
}

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
