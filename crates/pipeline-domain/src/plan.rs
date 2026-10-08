use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanContent {
    pub epics: Vec<EpicSpec>,
    pub tasks: Vec<TaskSpec>,
    pub dependencies: Vec<DependencySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpicSpec {
    pub id: String,
    pub title: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub epic_id: Option<String>,
    pub title: String,
    pub outcome: String,
    pub weight: i64,
    pub risk: String,
    pub criteria: Vec<CriterionSpec>,
    pub verification_commands: Vec<String>,
    pub deliverables: Vec<String>,
    pub context_links: Vec<String>,
    pub estimate_band: String,
    pub owner_decision_triggers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionSpec {
    pub id: String,
    pub assertion: String,
    pub verifier: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencySpec {
    /// Task that waits for `to_task_id`.
    pub from_task_id: String,
    /// Prerequisite task.
    pub to_task_id: String,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Blocks,
    Informs,
}

impl DependencyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Informs => "informs",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    EmptyPlan,
    EmptyField(&'static str),
    DuplicateId(String),
    MissingEpic(String),
    MissingTask(String),
    InvalidWeight(String),
    MissingCriterion(String),
    MissingEvidenceRequirement(String),
    DuplicateDependency(String, String),
    SelfDependency(String),
    Cycle,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPlan => write!(f, "plan needs at least one task"),
            Self::EmptyField(field) => write!(f, "{field} must not be empty"),
            Self::DuplicateId(id) => write!(f, "duplicate plan ID: {id}"),
            Self::MissingEpic(id) => write!(f, "epic not found: {id}"),
            Self::MissingTask(id) => write!(f, "task not found: {id}"),
            Self::InvalidWeight(id) => write!(f, "task weight must be 1–10,000: {id}"),
            Self::MissingCriterion(id) => write!(f, "task needs acceptance criteria: {id}"),
            Self::MissingEvidenceRequirement(id) => {
                write!(f, "criterion needs required evidence: {id}")
            }
            Self::DuplicateDependency(from, to) => {
                write!(f, "duplicate dependency: {from} -> {to}")
            }
            Self::SelfDependency(id) => write!(f, "task cannot depend on itself: {id}"),
            Self::Cycle => write!(f, "blocking dependencies contain a cycle"),
        }
    }
}

impl std::error::Error for PlanError {}

impl PlanContent {
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.tasks.is_empty() {
            return Err(PlanError::EmptyPlan);
        }
        let mut all_ids = HashSet::new();
        let mut epic_ids = HashSet::new();
        for epic in &self.epics {
            required("epic title", &epic.title)?;
            required("epic outcome", &epic.outcome)?;
            unique_id(&mut all_ids, &epic.id)?;
            epic_ids.insert(epic.id.as_str());
        }
        let mut task_ids = HashSet::new();
        for task in &self.tasks {
            required("task title", &task.title)?;
            required("task outcome", &task.outcome)?;
            required("task risk", &task.risk)?;
            unique_id(&mut all_ids, &task.id)?;
            task_ids.insert(task.id.as_str());
            if !(1..=10_000).contains(&task.weight) {
                return Err(PlanError::InvalidWeight(task.id.clone()));
            }
            if let Some(epic_id) = &task.epic_id
                && !epic_ids.contains(epic_id.as_str())
            {
                return Err(PlanError::MissingEpic(epic_id.clone()));
            }
            if task.criteria.is_empty() {
                return Err(PlanError::MissingCriterion(task.id.clone()));
            }
            for criterion in &task.criteria {
                required("criterion assertion", &criterion.assertion)?;
                required("criterion verifier", &criterion.verifier)?;
                unique_id(&mut all_ids, &criterion.id)?;
                if criterion.required_evidence.is_empty() {
                    return Err(PlanError::MissingEvidenceRequirement(criterion.id.clone()));
                }
                for requirement in &criterion.required_evidence {
                    required("evidence requirement", requirement)?;
                }
            }
        }
        let mut edges = HashSet::new();
        for dependency in &self.dependencies {
            if !task_ids.contains(dependency.from_task_id.as_str()) {
                return Err(PlanError::MissingTask(dependency.from_task_id.clone()));
            }
            if !task_ids.contains(dependency.to_task_id.as_str()) {
                return Err(PlanError::MissingTask(dependency.to_task_id.clone()));
            }
            if dependency.from_task_id == dependency.to_task_id {
                return Err(PlanError::SelfDependency(dependency.from_task_id.clone()));
            }
            if !edges.insert((
                dependency.from_task_id.as_str(),
                dependency.to_task_id.as_str(),
                dependency.kind,
            )) {
                return Err(PlanError::DuplicateDependency(
                    dependency.from_task_id.clone(),
                    dependency.to_task_id.clone(),
                ));
            }
        }
        if self.has_blocking_cycle() {
            return Err(PlanError::Cycle);
        }
        Ok(())
    }

    fn has_blocking_cycle(&self) -> bool {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.dependencies {
            if edge.kind == DependencyKind::Blocks {
                graph
                    .entry(edge.from_task_id.as_str())
                    .or_default()
                    .push(edge.to_task_id.as_str());
            }
        }
        let mut done = HashSet::new();
        let mut visiting = HashSet::new();
        self.tasks
            .iter()
            .any(|task| visit(&task.id, &graph, &mut visiting, &mut done))
    }
}

fn visit<'a>(
    id: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visiting: &mut HashSet<&'a str>,
    done: &mut HashSet<&'a str>,
) -> bool {
    if done.contains(id) {
        return false;
    }
    if !visiting.insert(id) {
        return true;
    }
    if graph.get(id).is_some_and(|next| {
        next.iter()
            .any(|target| visit(target, graph, visiting, done))
    }) {
        return true;
    }
    visiting.remove(id);
    done.insert(id);
    false
}

fn required(field: &'static str, value: &str) -> Result<(), PlanError> {
    if value.trim().is_empty() {
        Err(PlanError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn unique_id<'a>(ids: &mut HashSet<&'a str>, id: &'a str) -> Result<(), PlanError> {
    required("ID", id)?;
    if ids.insert(id) {
        Ok(())
    } else {
        Err(PlanError::DuplicateId(id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn task(id: &str) -> TaskSpec {
        TaskSpec {
            id: id.into(),
            epic_id: None,
            title: id.into(),
            outcome: "done".into(),
            weight: 1,
            risk: "low".into(),
            criteria: vec![CriterionSpec {
                id: format!("{id}-criterion"),
                assertion: "works".into(),
                verifier: "owner".into(),
                required_evidence: vec!["test-log".into()],
            }],
            verification_commands: Vec::new(),
            deliverables: Vec::new(),
            context_links: Vec::new(),
            estimate_band: String::new(),
            owner_decision_triggers: Vec::new(),
        }
    }

    #[test]
    fn every_two_and_three_node_blocking_cycle_is_rejected() {
        for count in [2, 3] {
            let mut plan = PlanContent {
                tasks: (0..count).map(|i| task(&format!("t{i}"))).collect(),
                ..Default::default()
            };
            for i in 0..count {
                plan.dependencies.push(DependencySpec {
                    from_task_id: format!("t{i}"),
                    to_task_id: format!("t{}", (i + 1) % count),
                    kind: DependencyKind::Blocks,
                });
            }
            assert_eq!(plan.validate(), Err(PlanError::Cycle));
            for edge in &mut plan.dependencies {
                edge.kind = DependencyKind::Informs;
            }
            assert!(plan.validate().is_ok());
        }
    }

    #[test]
    fn four_node_graphs_match_topological_sort() {
        let pairs: Vec<_> = (0..4)
            .flat_map(|from| {
                (0..4)
                    .filter(move |to| *to != from)
                    .map(move |to| (from, to))
            })
            .collect();
        for mask in 0..(1_u16 << pairs.len()) {
            let mut plan = PlanContent {
                tasks: (0..4).map(|i| task(&format!("t{i}"))).collect(),
                ..Default::default()
            };
            let mut indegree = [0_usize; 4];
            let mut graph = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            for (bit, &(from, to)) in pairs.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    plan.dependencies.push(DependencySpec {
                        from_task_id: format!("t{from}"),
                        to_task_id: format!("t{to}"),
                        kind: DependencyKind::Blocks,
                    });
                    graph[from].push(to);
                    indegree[to] += 1;
                }
            }
            let mut ready: VecDeque<_> = (0..4).filter(|i| indegree[*i] == 0).collect();
            let mut visited = 0;
            while let Some(node) = ready.pop_front() {
                visited += 1;
                for &next in &graph[node] {
                    indegree[next] -= 1;
                    if indegree[next] == 0 {
                        ready.push_back(next);
                    }
                }
            }
            assert_eq!(
                plan.validate() == Err(PlanError::Cycle),
                visited < 4,
                "mask {mask}"
            );
        }
    }
}
