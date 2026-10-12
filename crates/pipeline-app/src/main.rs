use eframe::egui;
use pipeline_engine::{
    BriefContent, CriterionSpec, DecisionInput, DependencyKind, DependencySpec, DiscoveryView,
    EpicSpec, InboxItem, MilestoneSpec, NextAction, NextDestination, PlanContent, PlanView,
    ProjectEngine, ProjectOverview, Proposal, ProposalPreview, ProposalStatus, ResearchInput,
    ReviewView, TaskRunReview, TaskSpec, sanitize_assistant_text,
};
use pipeline_terminal::TerminalSession;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;
mod assistant;
mod run_worker;
use assistant::{ChatConfig, ChatEvent, CompatibleProvider, ConversationProvider};
use run_worker::{RunCommand, RunControl, RunLaunch, RunUpdate};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct LayoutPrefs {
    show_portfolio: bool,
    show_terminal: bool,
    portfolio_width: f32,
    terminal_fraction: f32,
    text_scale: f32,
    dark_mode: bool,
}

impl Default for LayoutPrefs {
    fn default() -> Self {
        Self {
            show_portfolio: true,
            show_terminal: true,
            portfolio_width: 285.0,
            terminal_fraction: 0.35,
            text_scale: 1.0,
            dark_mode: true,
        }
    }
}

struct TerminalTab {
    id: u64,
    project_id: String,
    title: String,
    session: TerminalSession,
    error: Option<String>,
}

#[derive(Default)]
struct AssistantDrafts {
    portfolio: String,
    projects: BTreeMap<String, String>,
}

#[derive(Default)]
struct ChatSession {
    turns: Vec<ChatTurn>,
    receiver: Option<mpsc::Receiver<ChatEvent>>,
    cancel: Option<Arc<AtomicBool>>,
    fingerprint: String,
}

const MAX_ASSISTANT_WORKERS: usize = 2;

struct AssistantWorkerSlot(Arc<AtomicUsize>);

impl Drop for AssistantWorkerSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct ChatTurn {
    question: String,
    answer: String,
    status: String,
    sources: Vec<String>,
    usage: String,
    requested_identity: String,
    reported_model: Option<String>,
}

#[derive(Default)]
struct ProposalDraft {
    kind: usize,
    after: String,
    preview: Option<ProposalPreview>,
    preview_input: Option<ProposalInputFingerprint>,
    owner_confirmed: bool,
    message: String,
}

#[derive(Clone, PartialEq, Eq)]
struct ProposalInputFingerprint {
    kind: usize,
    target_id: String,
    after: String,
}

impl ProposalDraft {
    fn current_input(&self, target_id: &str) -> ProposalInputFingerprint {
        ProposalInputFingerprint {
            kind: self.kind,
            target_id: target_id.to_owned(),
            after: self.after.clone(),
        }
    }

    fn invalidate(&mut self, message: &str) {
        self.preview = None;
        self.preview_input = None;
        self.owner_confirmed = false;
        self.message = message.to_owned();
    }

    fn invalidate_if_input_changed(&mut self, target_id: &str) {
        if self.preview.is_some()
            && self.preview_input.as_ref() != Some(&self.current_input(target_id))
        {
            self.invalidate(
                "Proposal text or target changed. Review a new preview before applying.",
            );
        }
    }

    fn load_preview(&mut self, preview: ProposalPreview, target_id: &str, message: &str) {
        self.preview_input = Some(self.current_input(target_id));
        self.preview = Some(preview);
        self.owner_confirmed = false;
        self.message = message.to_owned();
    }
}

impl AssistantDrafts {
    fn active(&self, project_id: Option<&str>) -> &str {
        match project_id {
            Some(id) => self.projects.get(id).map_or("", String::as_str),
            None => &self.portfolio,
        }
    }

    fn active_mut(&mut self, project_id: Option<&str>) -> &mut String {
        match project_id {
            Some(id) => self.projects.entry(id.to_owned()).or_default(),
            None => &mut self.portfolio,
        }
    }
}

fn composer_shortcut(events: &[egui::Event]) -> bool {
    events.iter().any(|event| {
        matches!(event, egui::Event::Key { key: egui::Key::K, pressed: true, modifiers, .. } if modifiers.command || modifiers.ctrl)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Brief,
    Research,
    Plan,
    Decisions,
    Activity,
    Runs,
    About,
}

enum PlanAction {
    Save,
    Approve,
    Submit(String, i64),
    Verify(String, String, i64, BTreeMap<String, String>),
    Accept(String, i64),
    RequestChanges(String, i64, String),
}

enum DecisionAction {
    Save,
    Approve(String, i64, String),
    Reject(String, i64),
}

struct DesktopApp {
    #[cfg(feature = "screenshot-evidence")]
    capture_path: Option<String>,
    #[cfg(feature = "screenshot-evidence")]
    capture_frames: u8,
    modern_shell: bool,
    studio_home: bool,
    advanced_open: bool,
    assistant_open: bool,
    assistant_drafts: AssistantDrafts,
    assistant_config: ChatConfig,
    assistant_sessions: BTreeMap<String, ChatSession>,
    proposal_drafts: BTreeMap<String, ProposalDraft>,
    assistant_workers: Arc<AtomicUsize>,
    focus_composer: bool,
    focus_return: bool,
    next_action_cache: Option<(String, Instant, Result<NextAction, String>)>,
    data_dir: Option<PathBuf>,
    engine: Option<ProjectEngine>,
    projects: Vec<ProjectOverview>,
    global_inbox: Vec<InboxItem>,
    selected_project: Option<String>,
    active_tab: Tab,
    search: String,
    import_path: String,
    import_name: String,
    message: Option<String>,
    discovery: Option<DiscoveryView>,
    brief_draft: BriefContent,
    research_draft: ResearchInput,
    discovery_message: Option<String>,
    plan: Option<PlanView>,
    plan_draft: PlanContent,
    plan_message: Option<String>,
    review: Option<ReviewView>,
    run_reviews: Vec<TaskRunReview>,
    run_message: Option<String>,
    run_diff: Option<(String, String)>,
    change_reasons: BTreeMap<String, String>,
    run_control: Option<RunControl>,
    run_task_id: String,
    opencode_path: String,
    pi_path: String,
    pi_selected: bool,
    cli_path: String,
    provider_id: String,
    model_id: String,
    token_budget_input: String,
    pending_permissions: Vec<(String, String)>,
    last_run_refresh: Instant,
    decision_draft: DecisionInput,
    editing_decision: Option<String>,
    decision_message: Option<String>,
    selected_decision_options: BTreeMap<String, String>,
    evidence_drafts: BTreeMap<String, BTreeMap<String, String>>,
    new_dependency_from: String,
    new_dependency_to: String,
    new_dependency_kind: DependencyKind,
    show_portfolio: bool,
    show_terminal: bool,
    portfolio_width: f32,
    terminal_fraction: f32,
    text_scale: f32,
    dark_mode: bool,
    terminals: Vec<TerminalTab>,
    active_terminal: Option<u64>,
    next_terminal_id: u64,
    terminal_message: Option<String>,
}

impl Default for DesktopApp {
    fn default() -> Self {
        let data_dir = std::env::var_os("PIPELINE_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("dev", "Project Pipeline", "Project Pipeline")
                    .map(|dirs| dirs.data_local_dir().to_path_buf())
            });
        let prefs = data_dir
            .as_ref()
            .and_then(|dir| std::fs::read(dir.join("layout.json")).ok())
            .and_then(|bytes| serde_json::from_slice::<LayoutPrefs>(&bytes).ok())
            .unwrap_or_default();
        let mut app = Self {
            #[cfg(feature = "screenshot-evidence")]
            capture_path: std::env::var("PIPELINE_UX_SCREENSHOT").ok(),
            #[cfg(feature = "screenshot-evidence")]
            capture_frames: 0,
            modern_shell: std::env::var("PIPELINE_MODERN_SHELL").is_ok_and(|value| value == "1"),
            studio_home: true,
            advanced_open: false,
            assistant_open: false,
            assistant_drafts: AssistantDrafts::default(),
            assistant_config: ChatConfig::default(),
            assistant_sessions: BTreeMap::new(),
            proposal_drafts: BTreeMap::new(),
            assistant_workers: Arc::new(AtomicUsize::new(0)),
            focus_composer: false,
            focus_return: false,
            next_action_cache: None,
            data_dir: data_dir.clone(),
            engine: None,
            projects: Vec::new(),
            global_inbox: Vec::new(),
            selected_project: None,
            active_tab: Tab::Overview,
            search: String::new(),
            import_path: String::new(),
            import_name: String::new(),
            message: None,
            discovery: None,
            brief_draft: BriefContent::default(),
            research_draft: ResearchInput {
                confidence: "medium".to_owned(),
                ..Default::default()
            },
            discovery_message: None,
            plan: None,
            plan_draft: PlanContent::default(),
            plan_message: None,
            review: None,
            run_reviews: Vec::new(),
            run_message: None,
            run_diff: None,
            change_reasons: BTreeMap::new(),
            run_control: None,
            run_task_id: String::new(),
            opencode_path: std::env::var("PIPELINE_OPENCODE_EXE").unwrap_or_default(),
            pi_path: std::env::var("PIPELINE_PI_EXE").unwrap_or_default(),
            pi_selected: false,
            cli_path: std::env::var("PIPELINE_CLI_EXE").unwrap_or_else(|_| {
                std::env::current_exe()
                    .ok()
                    .and_then(|path| {
                        path.parent().map(|p| {
                            p.join(if cfg!(windows) {
                                "pipeline-cli.exe"
                            } else {
                                "pipeline-cli"
                            })
                        })
                    })
                    .map_or_else(String::new, |path| path.to_string_lossy().into_owned())
            }),
            provider_id: String::new(),
            model_id: String::new(),
            token_budget_input: "20000".into(),
            pending_permissions: Vec::new(),
            last_run_refresh: Instant::now(),
            decision_draft: DecisionInput {
                impact: "medium".into(),
                ..Default::default()
            },
            editing_decision: None,
            decision_message: None,
            selected_decision_options: BTreeMap::new(),
            evidence_drafts: BTreeMap::new(),
            new_dependency_from: String::new(),
            new_dependency_to: String::new(),
            new_dependency_kind: DependencyKind::Blocks,
            show_portfolio: prefs.show_portfolio,
            show_terminal: prefs.show_terminal,
            portfolio_width: prefs.portfolio_width.clamp(200.0, 700.0),
            terminal_fraction: prefs.terminal_fraction.clamp(0.2, 0.75),
            text_scale: prefs.text_scale.clamp(0.8, 2.0),
            dark_mode: prefs.dark_mode,
            terminals: Vec::new(),
            active_terminal: None,
            next_terminal_id: 1,
            terminal_message: None,
        };
        #[cfg(feature = "screenshot-evidence")]
        {
            if let Ok(scale) = std::env::var("PIPELINE_UX_SCALE") {
                app.text_scale = scale.parse::<f32>().unwrap_or(1.0).clamp(0.8, 2.0);
            }
            if let Ok(theme) = std::env::var("PIPELINE_UX_DARK") {
                app.dark_mode = theme == "1";
            }
            app.assistant_open = std::env::var("PIPELINE_UX_ASSISTANT").is_ok_and(|v| v == "1");
            app.advanced_open = std::env::var("PIPELINE_UX_ADVANCED").is_ok_and(|v| v == "1");
        }
        let database = data_dir.map(|dir| dir.join("portfolio.sqlite"));
        match database {
            Some(database) => {
                let result = database
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        let mut engine =
                            ProjectEngine::open(&database).map_err(|error| error.to_string())?;
                        engine
                            .reconcile_interrupted_runs()
                            .map_err(|error| error.to_string())?;
                        Ok(engine)
                    });
                match result {
                    Ok(engine) => {
                        match engine.list_overviews() {
                            Ok(projects) => app.projects = projects,
                            Err(error) => {
                                app.message = Some(format!("Unable to load projects: {error}"))
                            }
                        }
                        app.global_inbox = engine.global_inbox().unwrap_or_default();
                        app.selected_project =
                            app.projects.first().map(|project| project.id.clone());
                        #[cfg(feature = "screenshot-evidence")]
                        if std::env::var("PIPELINE_UX_OPEN_PROJECT").is_ok_and(|v| v == "1") {
                            app.studio_home = false;
                        }
                        if let Some(project_id) = &app.selected_project {
                            match engine.load_discovery(project_id) {
                                Ok(view) => {
                                    app.brief_draft = view.latest_content();
                                    app.discovery = Some(view);
                                }
                                Err(error) => {
                                    app.message = Some(format!("Unable to load brief: {error}"))
                                }
                            }
                            match engine.load_plan(project_id) {
                                Ok(view) => {
                                    app.plan_draft = view.latest_content();
                                    app.plan = Some(view);
                                }
                                Err(error) => {
                                    app.message = Some(format!("Unable to load plan: {error}"))
                                }
                            }
                            match engine.load_review(project_id) {
                                Ok(view) => app.review = Some(view),
                                Err(error) => {
                                    app.message = Some(format!("Unable to load decisions: {error}"))
                                }
                            }
                        }
                        app.engine = Some(engine);
                    }
                    Err(error) => app.message = Some(format!("Unable to open portfolio: {error}")),
                }
            }
            None => app.message = Some("No local application data directory found".to_owned()),
        }
        #[cfg(feature = "screenshot-evidence")]
        if let Ok(question) = std::env::var("PIPELINE_UX_ASK") {
            let context = app.assistant_context().map(str::to_owned);
            *app.assistant_drafts.active_mut(context.as_deref()) = question;
            app.send_assistant();
        }
        #[cfg(feature = "screenshot-evidence")]
        if std::env::var("PIPELINE_UX_PROPOSAL_DEMO").is_ok_and(|v| v == "1")
            && let (Some(engine), Some(project_id)) = (&app.engine, &app.selected_project)
            && let Ok(discovery) = engine.load_discovery(project_id)
            && let Ok(plan) = engine.load_plan(project_id)
            && let Some(brief) = discovery.briefs.last()
        {
            let raw = serde_json::json!({
                "version": 1, "id": "screenshot-preview", "project_id": project_id,
                "expected_revision": brief.revision, "expected_approved_brief_revision": discovery.approved_revision,
                "expected_scope_revision": plan.active_scope_revision, "original_evidence_ids": [brief.id],
                "reason": "Clarify the product idea", "impact": "A new brief draft revision",
                "affected_approved_scope": "None applied", "prerequisites": "Owner review",
                "risk": "Low; draft text only", "required_owner_approvals": "Exact apply, then separate brief approval",
                "recovery": "Restore previous text in a new draft revision",
                "change": {"type": "brief_field", "target_id": brief.id, "field": "idea", "before": brief.content.idea, "after": format!("{} — refined", brief.content.idea)}
            }).to_string();
            if let Ok(preview) =
                Proposal::parse_json(&raw).and_then(|p| engine.preview_proposal(&p))
            {
                app.proposal_drafts
                    .entry(project_id.clone())
                    .or_default()
                    .load_preview(preview, &brief.id, "Validated against current records.");
            }
        }
        app
    }
}

impl Drop for DesktopApp {
    fn drop(&mut self) {
        let Some(dir) = &self.data_dir else {
            return;
        };
        let prefs = LayoutPrefs {
            show_portfolio: self.show_portfolio,
            show_terminal: self.show_terminal,
            portfolio_width: self.portfolio_width,
            terminal_fraction: self.terminal_fraction,
            text_scale: self.text_scale,
            dark_mode: self.dark_mode,
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&prefs) {
            let _ = std::fs::write(dir.join("layout.json"), bytes);
        }
        // TerminalTab drops after this body and terminates any live shells.
    }
}

impl DesktopApp {
    fn add_project(&mut self, create: bool) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let result = if create {
            engine.create_new(&self.import_path, &self.import_name)
        } else {
            engine.import_existing(&self.import_path, &self.import_name)
        };
        match result {
            Ok(project) => {
                self.selected_project = Some(project.id);
                self.studio_home = false;
                self.next_action_cache = None;
                self.active_terminal = None;
                match engine.list_overviews() {
                    Ok(projects) => {
                        self.projects = projects;
                        self.message = Some("Project added to portfolio".to_owned());
                    }
                    Err(error) => {
                        self.message = Some(format!("Project added, but refresh failed: {error}"))
                    }
                }
                self.import_path.clear();
                self.import_name.clear();
                self.refresh_discovery();
                self.refresh_plan();
                self.refresh_review();
                self.refresh_global_inbox();
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn portfolio(&mut self, ui: &mut egui::Ui) {
        let panel = egui::Panel::left("portfolio")
            .resizable(true)
            .default_size(self.portfolio_width)
            .min_size(200.0)
            .show(ui, |ui| {
                ui.heading("Projects");
                egui::CollapsingHeader::new(format!("Owner inbox ({})", self.global_inbox.len()))
                    .show(ui, |ui| {
                        for item in self.global_inbox.clone() {
                            let project_name = self
                                .projects
                                .iter()
                                .find(|project| project.id == item.project_id)
                                .map_or("Project", |project| project.name.as_str());
                            if ui
                                .button(format!("{project_name}: {}", item.title))
                                .clicked()
                            {
                                self.select_project(item.project_id);
                                self.active_tab = match item.kind.as_str() {
                                    "brief" => Tab::Brief,
                                    "plan" | "task" => Tab::Plan,
                                    _ => Tab::Decisions,
                                };
                            }
                        }
                    });
                ui.text_edit_singleline(&mut self.search);
                ui.separator();
                ui.label("Folder path");
                ui.text_edit_singleline(&mut self.import_path);
                ui.label("Project name (optional for import)");
                ui.text_edit_singleline(&mut self.import_name);
                ui.horizontal(|ui| {
                    if ui.button("Import folder").clicked() {
                        self.add_project(false);
                    }
                    if ui.button("Create folder & project").clicked() {
                        self.add_project(true);
                    }
                });
                if let Some(message) = &self.message {
                    ui.label(message);
                }
                ui.separator();
                let needle = self.search.to_lowercase();
                let mut selected = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for project in &self.projects {
                        if !project.name.to_lowercase().contains(&needle) {
                            continue;
                        }
                        let label = format!(
                            "{}\n{} · {} · {:.1}% verified · {} blocked",
                            project.name,
                            project.stage.as_str(),
                            project.health.as_str(),
                            project.verified_completion_basis_points as f32 / 100.0,
                            project.blocked_count
                        );
                        if ui
                            .add_sized(
                                [ui.available_width(), 58.0],
                                egui::Button::new(label).selected(
                                    self.selected_project.as_deref() == Some(&project.id),
                                ),
                            )
                            .clicked()
                        {
                            selected = Some(project.id.clone());
                        }
                    }
                });
                if let Some(id) = selected {
                    self.select_project(id);
                }
            });
        self.portfolio_width = panel.response.rect.width();
    }

    fn select_project(&mut self, id: String) {
        for draft in self.proposal_drafts.values_mut() {
            draft.owner_confirmed = false;
        }
        self.selected_project = Some(id.clone());
        self.studio_home = false;
        self.next_action_cache = None;
        self.run_diff = None;
        self.run_message = None;
        self.active_terminal = self
            .terminals
            .iter()
            .find(|tab| tab.project_id == id)
            .map(|tab| tab.id);
        self.terminal_message = None;
        self.refresh_discovery();
        self.refresh_plan();
        self.refresh_review();
        self.refresh_runs();
    }

    fn refresh_discovery(&mut self) {
        let Some(project_id) = &self.selected_project else {
            self.discovery = None;
            self.brief_draft = BriefContent::default();
            return;
        };
        let Some(engine) = &self.engine else {
            return;
        };
        match engine.load_discovery(project_id) {
            Ok(view) => {
                self.brief_draft = view.latest_content();
                self.discovery = Some(view);
                self.discovery_message = None;
            }
            Err(error) => self.discovery_message = Some(error.to_string()),
        }
    }

    fn refresh_plan(&mut self) {
        let Some(project_id) = &self.selected_project else {
            self.plan = None;
            self.plan_draft = PlanContent::default();
            return;
        };
        let Some(engine) = &self.engine else {
            return;
        };
        match engine.load_plan(project_id) {
            Ok(view) => {
                self.plan_draft = view.latest_content();
                self.plan = Some(view);
                self.plan_message = None;
                self.evidence_drafts.clear();
                self.new_dependency_from.clear();
                self.new_dependency_to.clear();
            }
            Err(error) => self.plan_message = Some(error.to_string()),
        }
    }

    fn refresh_review(&mut self) {
        let Some(project_id) = &self.selected_project else {
            self.review = None;
            return;
        };
        let Some(engine) = &self.engine else {
            return;
        };
        match engine.load_review(project_id) {
            Ok(view) => {
                self.review = Some(view);
                self.decision_message = None;
            }
            Err(error) => self.decision_message = Some(error.to_string()),
        }
    }

    fn refresh_runs(&mut self) {
        self.run_reviews.clear();
        if let (Some(engine), Some(project_id)) = (&self.engine, &self.selected_project) {
            match engine.list_task_run_reviews(project_id) {
                Ok(reviews) => {
                    self.run_reviews = reviews;
                }
                Err(error) => self.run_message = Some(error.to_string()),
            }
        }
    }

    fn poll_run(&mut self, ctx: &egui::Context) {
        let mut finished = false;
        let mut permission_changed = false;
        if let Some(control) = &self.run_control {
            while let Ok(update) = control.updates.try_recv() {
                match update {
                    RunUpdate::Started(id) => self.run_message = Some(format!("Run {id} started")),
                    RunUpdate::Event(message) => self.run_message = Some(message),
                    RunUpdate::Permission(id, summary) => {
                        if !self
                            .pending_permissions
                            .iter()
                            .any(|(pending_id, _)| pending_id == &id)
                        {
                            self.pending_permissions.push((id, summary));
                        }
                        permission_changed = true;
                    }
                    RunUpdate::PermissionResolved(id) => {
                        self.pending_permissions
                            .retain(|(pending_id, _)| pending_id != &id);
                        permission_changed = true;
                    }
                    RunUpdate::Finished(message) => {
                        self.run_message = Some(message);
                        finished = true;
                    }
                    RunUpdate::Failed(error) => {
                        self.run_message = Some(format!("Run failed: {error}"));
                        finished = true;
                    }
                }
            }
        }
        if permission_changed {
            self.refresh_review();
        }
        if self.run_control.is_some() && self.last_run_refresh.elapsed() >= Duration::from_secs(2) {
            self.refresh_runs();
            if self.active_tab == Tab::Runs {
                self.refresh_plan();
            }
            self.last_run_refresh = Instant::now();
        }
        if finished {
            self.run_control = None;
            self.pending_permissions.clear();
            self.refresh_runs();
            self.refresh_plan();
            self.refresh_review();
        }
        if self.run_control.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn start_run(&mut self) {
        let (Some(project_id), Some(data_dir)) = (&self.selected_project, &self.data_dir) else {
            self.run_message = Some("Select a project first".into());
            return;
        };
        let Ok(token_budget) = self.token_budget_input.trim().parse::<i64>() else {
            self.run_message = Some("Enter an observed token limit from 1 to 160,000".into());
            return;
        };
        if !(1..=160_000).contains(&token_budget) {
            self.run_message = Some("Enter an observed token limit from 1 to 160,000".into());
            return;
        }
        let launch = RunLaunch {
            database: data_dir.join("portfolio.sqlite"),
            evidence_dir: data_dir.join("evidence"),
            project_id: project_id.clone(),
            task_id: self.run_task_id.clone(),
            opencode: PathBuf::from(if self.pi_selected {
                &self.pi_path
            } else {
                &self.opencode_path
            }),
            pi: self.pi_selected,
            cli: PathBuf::from(&self.cli_path),
            provider: self.provider_id.clone(),
            model: self.model_id.clone(),
            token_budget,
        };
        self.run_control = Some(run_worker::launch(launch));
        self.run_message = Some("Preparing approved task and isolated checkout…".into());
    }

    fn refresh_global_inbox(&mut self) {
        if let Some(engine) = &self.engine
            && let Ok(items) = engine.global_inbox()
        {
            self.global_inbox = items;
        }
    }

    fn clear_decision_draft(&mut self) {
        self.editing_decision = None;
        self.decision_draft = DecisionInput {
            impact: "medium".into(),
            ..Default::default()
        };
        self.selected_decision_options.clear();
    }

    fn run_decision_action(&mut self, action: DecisionAction) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        let result = match action {
            DecisionAction::Save => {
                if let Some(id) = &self.editing_decision {
                    let revision = self
                        .review
                        .as_ref()
                        .and_then(|view| view.decisions.iter().find(|item| &item.id == id))
                        .map_or(0, |item| item.revision);
                    engine.revise_decision(project_id, id, revision, &self.decision_draft)
                } else {
                    engine.propose_decision(project_id, &self.decision_draft)
                }
            }
            DecisionAction::Approve(id, revision, option) => {
                engine.resolve_decision(project_id, &id, revision, Some(&option), true)
            }
            DecisionAction::Reject(id, revision) => {
                engine.resolve_decision(project_id, &id, revision, None, false)
            }
        };
        match result {
            Ok(view) => {
                let projects = engine.list_overviews().ok();
                let inbox = engine.global_inbox().ok();
                self.review = Some(view);
                self.decision_message = Some("Decision saved with audit history".into());
                self.clear_decision_draft();
                if let Some(projects) = projects {
                    self.projects = projects;
                }
                if let Some(items) = inbox {
                    self.global_inbox = items;
                }
            }
            Err(error) => self.decision_message = Some(error.to_string()),
        }
    }

    fn run_policy_review(&mut self, id: &str, revision: i64, approve: bool) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        let expiry = if approve {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
                    + 900,
            )
        } else {
            None
        };
        match engine.resolve_action_request(id, revision, approve, expiry) {
            Ok(_) => {
                self.review = engine.load_review(project_id).ok();
                self.global_inbox = engine.global_inbox().unwrap_or_default();
                self.decision_message = Some(if approve {
                    "Scoped action approved for one use".into()
                } else {
                    "Action denied".into()
                });
            }
            Err(error) => self.decision_message = Some(error.to_string()),
        }
    }

    fn run_plan_action(&mut self, action: PlanAction) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        let result = match action {
            PlanAction::Save => {
                let expected = self.plan.as_ref().map_or(0, PlanView::latest_revision);
                engine
                    .save_plan(project_id, expected, &self.plan_draft)
                    .map(|view| (view, format!("Saved plan revision {}", expected + 1), true))
            }
            PlanAction::Approve => {
                let expected = self.plan.as_ref().map_or(0, PlanView::latest_revision);
                engine
                    .approve_latest_plan(project_id, expected)
                    .map(|view| (view, format!("Approved plan revision {expected}"), true))
            }
            PlanAction::Submit(id, revision) => engine
                .submit_task_for_review(project_id, &id, revision)
                .map(|view| (view, "Task submitted for review".to_owned(), false)),
            PlanAction::Verify(task_id, criterion_id, revision, evidence) => engine
                .verify_criterion(project_id, &task_id, &criterion_id, revision, &evidence)
                .map(|view| (view, "Criterion verified with evidence".to_owned(), false)),
            PlanAction::Accept(id, revision) => engine
                .accept_task(project_id, &id, revision)
                .map(|view| (view, "Task accepted".to_owned(), false)),
            PlanAction::RequestChanges(id, revision, reason) => engine
                .request_task_changes(project_id, &id, revision, &reason)
                .map(|view| {
                    (
                        view,
                        "Changes requested; task is ready to rerun".to_owned(),
                        false,
                    )
                }),
        };
        match result {
            Ok((view, message, reset_draft)) => {
                if reset_draft {
                    self.plan_draft = view.latest_content();
                }
                self.plan = Some(view);
                self.plan_message = Some(message);
                if let Ok(projects) = engine.list_overviews() {
                    self.projects = projects;
                }
                if let Ok(review) = engine.load_review(project_id) {
                    self.review = Some(review);
                }
                if let Ok(items) = engine.global_inbox() {
                    self.global_inbox = items;
                }
                self.refresh_runs();
            }
            Err(error) => self.plan_message = Some(error.to_string()),
        }
    }

    fn save_brief(&mut self) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        let expected = self
            .discovery
            .as_ref()
            .map_or(0, DiscoveryView::latest_revision);
        match engine.save_brief(project_id, expected, &self.brief_draft) {
            Ok(view) => {
                self.discovery = Some(view);
                self.discovery_message = Some(format!("Saved brief revision {}", expected + 1));
                if let Ok(projects) = engine.list_overviews() {
                    self.projects = projects;
                }
                if let Ok(review) = engine.load_review(project_id) {
                    self.review = Some(review);
                }
                if let Ok(items) = engine.global_inbox() {
                    self.global_inbox = items;
                }
            }
            Err(error) => self.discovery_message = Some(error.to_string()),
        }
    }

    fn approve_brief(&mut self) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        let expected = self
            .discovery
            .as_ref()
            .map_or(0, DiscoveryView::latest_revision);
        match engine.approve_latest_brief(project_id, expected) {
            Ok(view) => {
                self.discovery = Some(view);
                self.discovery_message = Some(format!("Approved brief revision {expected}"));
                if let Ok(projects) = engine.list_overviews() {
                    self.projects = projects;
                }
                if let Ok(review) = engine.load_review(project_id) {
                    self.review = Some(review);
                }
                if let Ok(items) = engine.global_inbox() {
                    self.global_inbox = items;
                }
            }
            Err(error) => self.discovery_message = Some(error.to_string()),
        }
    }

    fn record_research(&mut self) {
        let (Some(engine), Some(project_id)) =
            (self.engine.as_mut(), self.selected_project.as_deref())
        else {
            return;
        };
        match engine.record_research(project_id, &self.research_draft) {
            Ok(view) => {
                self.discovery = Some(view);
                self.discovery_message = Some("Research finding recorded".to_owned());
                self.research_draft = ResearchInput {
                    confidence: "medium".to_owned(),
                    ..Default::default()
                };
            }
            Err(error) => self.discovery_message = Some(error.to_string()),
        }
    }

    fn start_terminal(&mut self) {
        let Some(project) = self
            .projects
            .iter()
            .find(|project| self.selected_project.as_deref() == Some(&project.id))
        else {
            self.terminal_message = Some("Select a project first".to_owned());
            return;
        };
        match TerminalSession::spawn(Path::new(&project.path)) {
            Ok(session) => {
                let id = self.next_terminal_id;
                self.next_terminal_id += 1;
                self.terminals.push(TerminalTab {
                    id,
                    project_id: project.id.clone(),
                    title: format!("Shell {id}"),
                    session,
                    error: None,
                });
                self.active_terminal = Some(id);
                self.terminal_message = None;
            }
            Err(error) => self.terminal_message = Some(error.to_string()),
        }
    }

    fn terminal_area(&mut self, ui: &mut egui::Ui) {
        let selected_id = self.selected_project.clone();
        let mut select = None;
        let mut close = None;
        let mut start = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button("+ New shell").clicked() {
                start = true;
            }
            for tab in self
                .terminals
                .iter()
                .filter(|tab| Some(&tab.project_id) == selected_id.as_ref())
            {
                if ui
                    .selectable_label(self.active_terminal == Some(tab.id), &tab.title)
                    .clicked()
                {
                    select = Some(tab.id);
                }
                if ui.small_button("×").clicked() {
                    close = Some(tab.id);
                }
            }
        });
        if let Some(id) = select {
            self.active_terminal = Some(id);
        }
        if let Some(id) = close {
            self.terminals.retain(|tab| tab.id != id);
            if self.active_terminal == Some(id) {
                self.active_terminal = self
                    .terminals
                    .iter()
                    .find(|tab| Some(&tab.project_id) == selected_id.as_ref())
                    .map(|tab| tab.id);
            }
        }
        if start {
            self.start_terminal();
        }
        if let Some(message) = &self.terminal_message {
            ui.label(message);
        }
        let active = self
            .active_terminal
            .and_then(|id| self.terminals.iter_mut().find(|tab| tab.id == id));
        if let Some(tab) = active {
            if let Some(error) = &tab.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if let Some(status) = tab.session.exit_status() {
                ui.label(format!("Shell exited: {status}"));
            }
            Self::terminal_screen(ui, tab);
        } else if let Some(project) = self
            .projects
            .iter()
            .find(|project| Some(&project.id) == selected_id.as_ref())
        {
            ui.label(format!("New shell starts in {}", project.path));
        } else {
            ui.label("Select a project to open a shell.");
        }
        // A resizable egui panel needs content that occupies its remaining
        // height even before the first terminal tab is opened.
        ui.allocate_space(ui.available_size());
    }

    fn terminal_screen(ui: &mut egui::Ui, tab: &mut TerminalTab) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click());
        if response.clicked() {
            ui.memory_mut(|memory| memory.request_focus(response.id));
        }
        let focused = ui.memory(|memory| memory.has_focus(response.id));
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(13, 17, 21));
        let font = egui::FontId::monospace(14.0);
        let glyph = ui
            .painter()
            .layout_no_wrap("M".to_owned(), font.clone(), egui::Color32::WHITE);
        let char_width = glyph.size().x.max(1.0);
        let line_height = glyph.size().y.max(1.0);
        let cols = ((rect.width() - 8.0) / char_width) as u16;
        let rows = ((rect.height() - 8.0) / line_height) as u16;
        if let Err(error) = tab.session.resize(rows, cols) {
            tab.error = Some(error.to_string());
        }
        if response.hovered() {
            let wheel = ui.input(|input| input.smooth_scroll_delta.y);
            if wheel.abs() > 1.0 {
                let lines = (wheel.abs() / line_height).ceil() as usize;
                let offset = if wheel > 0.0 {
                    tab.session.scrollback().saturating_add(lines)
                } else {
                    tab.session.scrollback().saturating_sub(lines)
                };
                tab.session.set_scrollback(offset);
            }
        }
        ui.painter().text(
            rect.min + egui::vec2(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            tab.session.contents(),
            font,
            egui::Color32::LIGHT_GRAY,
        );
        if focused {
            ui.painter().rect_stroke(
                rect,
                0.0,
                (1.0, egui::Color32::from_rgb(70, 130, 200)),
                egui::StrokeKind::Inside,
            );
            let events = ui.input(|input| input.events.clone());
            for event in events {
                let bytes = match event {
                    egui::Event::Text(text) => Some(text.into_bytes()),
                    egui::Event::Paste(text) => Some(text.into_bytes()),
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.ctrl {
                            match key {
                                egui::Key::C => Some(vec![3]),
                                egui::Key::D => Some(vec![4]),
                                _ => None,
                            }
                        } else {
                            match key {
                                egui::Key::Enter => Some(b"\r".to_vec()),
                                egui::Key::Backspace => Some(vec![8]),
                                egui::Key::Tab => Some(vec![9]),
                                egui::Key::Escape => Some(vec![27]),
                                egui::Key::ArrowUp => Some(b"\x1b[A".to_vec()),
                                egui::Key::ArrowDown => Some(b"\x1b[B".to_vec()),
                                egui::Key::ArrowRight => Some(b"\x1b[C".to_vec()),
                                egui::Key::ArrowLeft => Some(b"\x1b[D".to_vec()),
                                egui::Key::Home => Some(b"\x1b[H".to_vec()),
                                egui::Key::End => Some(b"\x1b[F".to_vec()),
                                egui::Key::Delete => Some(b"\x1b[3~".to_vec()),
                                _ => None,
                            }
                        }
                    }
                    _ => None,
                };
                if let Some(bytes) = bytes
                    && let Err(error) = tab.session.send(&bytes)
                {
                    tab.error = Some(error.to_string());
                    break;
                }
            }
        }
    }

    fn brief_workspace(&mut self, ui: &mut egui::Ui) {
        let latest = self
            .discovery
            .as_ref()
            .map_or(0, DiscoveryView::latest_revision);
        let approved = self
            .discovery
            .as_ref()
            .map_or(0, |view| view.approved_revision);
        let dirty = self
            .discovery
            .as_ref()
            .is_some_and(|view| self.brief_draft != view.latest_content());
        let mut save = false;
        let mut approve = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Product brief");
            if self.selected_project.is_none() {
                ui.label("Select a project first.");
                return;
            }
            ui.label(format!(
                "Latest revision: {latest} · Approved revision: {approved}"
            ));
            ui.label(
                "Start with the four required intake fields. Add detail as the idea develops.",
            );
            brief_field(ui, "Idea", &mut self.brief_draft.idea, 2);
            brief_field(ui, "Audience", &mut self.brief_draft.audience, 2);
            brief_field(ui, "Problem", &mut self.brief_draft.problem, 2);
            brief_field(
                ui,
                "Desired outcome",
                &mut self.brief_draft.desired_outcome,
                2,
            );
            brief_field(ui, "Constraints", &mut self.brief_draft.constraints, 2);
            brief_field(
                ui,
                "Value proposition",
                &mut self.brief_draft.value_proposition,
                2,
            );
            brief_field(ui, "Scope", &mut self.brief_draft.scope, 3);
            brief_field(ui, "Non-goals", &mut self.brief_draft.non_goals, 2);
            brief_field(ui, "User journeys", &mut self.brief_draft.user_journeys, 3);
            brief_field(
                ui,
                "Success metrics",
                &mut self.brief_draft.success_metrics,
                2,
            );
            brief_field(ui, "UX principles", &mut self.brief_draft.ux_principles, 2);
            brief_field(
                ui,
                "Architecture candidates",
                &mut self.brief_draft.architecture_candidates,
                2,
            );
            brief_field(ui, "Costs", &mut self.brief_draft.costs, 2);
            brief_field(ui, "Risks", &mut self.brief_draft.risks, 2);
            brief_field(
                ui,
                "Milestone plan",
                &mut self.brief_draft.milestone_plan,
                3,
            );
            ui.horizontal(|ui| {
                if ui.button("Save new revision").clicked() {
                    save = true;
                }
                if ui
                    .add_enabled(
                        latest > approved && !dirty && self.brief_draft.has_required_intake(),
                        egui::Button::new(format!("Approve saved revision {latest}")),
                    )
                    .clicked()
                {
                    approve = true;
                }
            });
            if dirty {
                ui.label("Unsaved changes must be saved before approval.");
            }
            if !self.brief_draft.has_required_intake() {
                ui.label("Approval requires idea, audience, problem, and desired outcome.");
            }
            if let Some(message) = &self.discovery_message {
                ui.label(message);
            }
            if let Some(view) = &self.discovery {
                if latest > 1 {
                    ui.separator();
                    ui.heading(format!("Changes in revision {latest}"));
                    if view.changes.is_empty() {
                        ui.label("No content changes from the prior revision.");
                    }
                    for change in &view.changes {
                        ui.collapsing(change.field, |ui| {
                            ui.label(format!("Before: {}", change.before));
                            ui.label(format!("After: {}", change.after));
                        });
                    }
                }
                ui.separator();
                ui.heading("Revision history");
                for brief in view.briefs.iter().rev() {
                    ui.label(format!(
                        "Revision {} · {} · {}",
                        brief.revision, brief.status, brief.created_at
                    ));
                }
            }
        });
        if save {
            self.save_brief();
        }
        if approve {
            self.approve_brief();
        }
    }

    fn research_workspace(&mut self, ui: &mut egui::Ui) {
        let mut record = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Research");
            if self.selected_project.is_none() {
                ui.label("Select a project first.");
                return;
            }
            ui.checkbox(
                &mut self.research_draft.is_hypothesis,
                "Hypothesis (not yet verified)",
            );
            brief_field(ui, "Claim", &mut self.research_draft.claim, 2);
            brief_field(ui, "Summary", &mut self.research_draft.summary, 3);
            brief_field(
                ui,
                "Relevance to this product",
                &mut self.research_draft.relevance,
                2,
            );
            ui.label("Source URI or file path");
            ui.text_edit_singleline(&mut self.research_draft.source_uri);
            ui.label("Accessed date (YYYY-MM-DD)");
            ui.text_edit_singleline(&mut self.research_draft.accessed_at);
            egui::ComboBox::from_label("Confidence")
                .selected_text(&self.research_draft.confidence)
                .show_ui(ui, |ui| {
                    for level in ["low", "medium", "high"] {
                        ui.selectable_value(
                            &mut self.research_draft.confidence,
                            level.to_owned(),
                            level,
                        );
                    }
                });
            if !self.research_draft.is_hypothesis {
                ui.label("Sourced facts require a source and access date.");
            }
            if ui.button("Record finding").clicked() {
                record = true;
            }
            if let Some(message) = &self.discovery_message {
                ui.label(message);
            }
            ui.separator();
            if let Some(view) = &self.discovery {
                ui.heading(format!("Findings ({})", view.research.len()));
                for finding in view.research.iter().rev() {
                    ui.group(|ui| {
                        ui.strong(format!(
                            "{} · {}",
                            if finding.input.is_hypothesis {
                                "Hypothesis"
                            } else {
                                "Sourced fact"
                            },
                            finding.input.claim
                        ));
                        ui.label(&finding.input.summary);
                        ui.label(format!("Relevance: {}", finding.input.relevance));
                        ui.label(format!("Confidence: {}", finding.input.confidence));
                        if !finding.input.source_uri.is_empty() {
                            ui.label(format!(
                                "Source: {} · accessed {}",
                                finding.input.source_uri, finding.input.accessed_at
                            ));
                        }
                    });
                }
            }
        });
        if record {
            self.record_research();
        }
    }

    fn plan_workspace(&mut self, ui: &mut egui::Ui) {
        let latest = self.plan.as_ref().map_or(0, PlanView::latest_revision);
        let active_scope = self
            .plan
            .as_ref()
            .map_or(0, |view| view.active_scope_revision);
        let dirty = self
            .plan
            .as_ref()
            .is_some_and(|view| self.plan_draft != view.latest_content());
        let brief_approved = self.discovery.as_ref().is_some_and(|view| {
            view.approved_revision > 0 && view.latest_revision() == view.approved_revision
        });
        let pending = self
            .plan
            .as_ref()
            .is_some_and(PlanView::has_pending_revision);
        let snapshot = self.plan.as_ref();
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Task plan");
            if self.selected_project.is_none() {
                ui.label("Select a project first.");
                return;
            }
            ui.label(format!(
                "Latest plan revision: {latest} · Active approved scope: {active_scope}"
            ));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        latest == 0 || dirty,
                        egui::Button::new("Save plan revision"),
                    )
                    .clicked()
                {
                    action = Some(PlanAction::Save);
                }
                if ui
                    .add_enabled(
                        brief_approved && pending && !dirty && self.plan_draft.validate().is_ok(),
                        egui::Button::new(format!("Approve revision {latest} as new scope")),
                    )
                    .clicked()
                {
                    action = Some(PlanAction::Approve);
                }
            });
            if !brief_approved && pending {
                ui.label("Approve the latest product brief before approving the task plan.");
            }
            if dirty {
                ui.label("Save changes before approval.");
            }
            if let Err(error) = self.plan_draft.validate() {
                ui.label(format!("Plan needs work before approval: {error}"));
            }
            if let Some(message) = &self.plan_message {
                ui.label(message);
            }
            if let Some(view) = &snapshot {
                let accepted = view
                    .active_tasks
                    .iter()
                    .filter(|task| task.status == "accepted")
                    .count();
                let runnable = view
                    .active_tasks
                    .iter()
                    .filter(|task| task.status == "ready" && task.unresolved_blockers.is_empty())
                    .count();
                ui.label(format!(
                    "Active scope: {accepted}/{} tasks accepted · {runnable} ready without blockers",
                    view.active_tasks.len()
                ));
                if view.has_pending_revision() {
                    ui.strong("A draft plan is awaiting review; current completion still uses the approved scope.");
                }
                if view.revisions.len() > 1 {
                    for summary in plan_changes(
                        &view.revisions[view.revisions.len() - 2].content,
                        &view.latest_content(),
                    ) {
                        ui.label(summary);
                    }
                }
            }
            ui.separator();
            ui.heading("Epics");
            if ui.button("+ Add epic").clicked() {
                self.plan_draft.epics.push(EpicSpec {
                    id: Uuid::new_v4().to_string(),
                    title: String::new(),
                    outcome: String::new(),
                });
            }
            let mut remove_epic = None;
            for (index, epic) in self.plan_draft.epics.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.label(format!("Epic {}", index + 1));
                    ui.text_edit_singleline(&mut epic.title);
                    brief_field(ui, "Outcome", &mut epic.outcome, 2);
                    if ui.button("Remove epic").clicked() {
                        remove_epic = Some(index);
                    }
                });
            }
            if let Some(index) = remove_epic {
                let removed = self.plan_draft.epics.remove(index);
                for task in &mut self.plan_draft.tasks {
                    if task.epic_id.as_deref() == Some(&removed.id) {
                        task.epic_id = None;
                    }
                }
            }
            ui.separator();
            ui.heading("Tasks");
            if ui.button("+ Add task").clicked() {
                self.plan_draft.tasks.push(TaskSpec {
                    id: Uuid::new_v4().to_string(),
                    epic_id: None,
                    title: String::new(),
                    outcome: String::new(),
                    weight: 1,
                    risk: "low".to_owned(),
                    criteria: vec![CriterionSpec {
                        id: Uuid::new_v4().to_string(),
                        assertion: String::new(),
                        verifier: "owner".to_owned(),
                        required_evidence: vec!["test-log".to_owned()],
                    }],
                    verification_commands: Vec::new(),
                    deliverables: Vec::new(),
                    context_links: Vec::new(),
                    estimate_band: String::new(),
                    owner_decision_triggers: Vec::new(),
                });
            }
            let epic_choices: Vec<_> = self
                .plan_draft
                .epics
                .iter()
                .map(|epic| (epic.id.clone(), epic.title.clone()))
                .collect();
            let mut remove_task = None;
            for (index, task) in self.plan_draft.tasks.iter_mut().enumerate() {
                let label = if task.title.trim().is_empty() {
                    format!("Task {} · untitled", index + 1)
                } else {
                    format!("Task {} · {}", index + 1, task.title)
                };
                egui::CollapsingHeader::new(label)
                    .id_salt(&task.id)
                    .show(ui, |ui| {
                        ui.label("Title");
                        ui.text_edit_singleline(&mut task.title);
                        brief_field(ui, "Desired outcome", &mut task.outcome, 2);
                        ui.horizontal(|ui| {
                            ui.label("Weight");
                            ui.add(egui::DragValue::new(&mut task.weight).range(1..=10_000));
                            ui.label("Risk");
                            ui.text_edit_singleline(&mut task.risk);
                        });
                        egui::ComboBox::from_id_salt(("task-epic", &task.id))
                            .selected_text(
                                epic_choices
                                    .iter()
                                    .find(|(id, _)| task.epic_id.as_deref() == Some(id))
                                    .map_or("No epic", |(_, title)| title.as_str()),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut task.epic_id, None, "No epic");
                                for (id, title) in &epic_choices {
                                    ui.selectable_value(&mut task.epic_id, Some(id.clone()), title);
                                }
                            });
                        plan_lines(ui, "Verification commands", &mut task.verification_commands);
                        plan_lines(ui, "Deliverables", &mut task.deliverables);
                        plan_lines(ui, "Context links", &mut task.context_links);
                        ui.label("Estimate band");
                        ui.text_edit_singleline(&mut task.estimate_band);
                        plan_lines(
                            ui,
                            "Owner decision triggers",
                            &mut task.owner_decision_triggers,
                        );
                        ui.label("Acceptance criteria");
                        if ui.button("+ Add criterion").clicked() {
                            task.criteria.push(CriterionSpec {
                                id: Uuid::new_v4().to_string(),
                                assertion: String::new(),
                                verifier: "owner".to_owned(),
                                required_evidence: vec!["test-log".to_owned()],
                            });
                        }
                        let mut remove_criterion = None;
                        for (criterion_index, criterion) in task.criteria.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.label(format!("Criterion {}", criterion_index + 1));
                                brief_field(ui, "Assertion", &mut criterion.assertion, 2);
                                ui.label("Verifier");
                                ui.text_edit_singleline(&mut criterion.verifier);
                                plan_lines(
                                    ui,
                                    "Required evidence kinds",
                                    &mut criterion.required_evidence,
                                );
                                if ui.button("Remove criterion").clicked() {
                                    remove_criterion = Some(criterion_index);
                                }
                            });
                        }
                        if let Some(criterion_index) = remove_criterion {
                            task.criteria.remove(criterion_index);
                        }
                        if ui.button("Remove task").clicked() {
                            remove_task = Some(index);
                        }
                    });
            }
            if let Some(index) = remove_task {
                let removed = self.plan_draft.tasks.remove(index);
                self.plan_draft.dependencies.retain(|edge| {
                    edge.from_task_id != removed.id && edge.to_task_id != removed.id
                });
                for milestone in &mut self.plan_draft.milestones {
                    milestone.task_ids.retain(|id| id != &removed.id);
                }
            }
            let task_choices: Vec<_> = self
                .plan_draft
                .tasks
                .iter()
                .map(|task| (task.id.clone(), task.title.clone()))
                .collect();
            ui.separator();
            ui.heading("Milestones");
            if ui.button("+ Add milestone").clicked() {
                self.plan_draft.milestones.push(MilestoneSpec {
                    id: Uuid::new_v4().to_string(),
                    title: String::new(),
                    outcome: String::new(),
                    task_ids: Vec::new(),
                });
            }
            let mut remove_milestone=None;
            for (index,milestone) in self.plan_draft.milestones.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.label(format!("Milestone {}",index+1));
                    ui.text_edit_singleline(&mut milestone.title);
                    brief_field(ui,"Outcome",&mut milestone.outcome,2);
                    ui.label("Tasks in this milestone");
                    for (id,title) in &task_choices {
                        let mut included=milestone.task_ids.contains(id);
                        if ui.checkbox(&mut included,title).changed() {
                            if included { milestone.task_ids.push(id.clone()); }
                            else { milestone.task_ids.retain(|task_id| task_id != id); }
                        }
                    }
                    if ui.button("Remove milestone").clicked() { remove_milestone=Some(index); }
                });
            }
            if let Some(index)=remove_milestone { self.plan_draft.milestones.remove(index); }
            ui.separator();
            ui.heading("Dependencies");
            ui.label("A blocking edge means the first task waits for the prerequisite.");
            if !task_choices
                .iter()
                .any(|(id, _)| id == &self.new_dependency_from)
            {
                self.new_dependency_from =
                    task_choices.first().map_or(String::new(), |t| t.0.clone());
            }
            if !task_choices
                .iter()
                .any(|(id, _)| id == &self.new_dependency_to)
            {
                self.new_dependency_to = task_choices.get(1).map_or(String::new(), |t| t.0.clone());
            }
            ui.horizontal_wrapped(|ui| {
                task_choice(
                    ui,
                    "Dependent task",
                    &mut self.new_dependency_from,
                    &task_choices,
                );
                ui.label("waits for");
                task_choice(
                    ui,
                    "Prerequisite",
                    &mut self.new_dependency_to,
                    &task_choices,
                );
                egui::ComboBox::from_id_salt("new-dependency-kind")
                    .selected_text(self.new_dependency_kind.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.new_dependency_kind,
                            DependencyKind::Blocks,
                            "blocks",
                        );
                        ui.selectable_value(
                            &mut self.new_dependency_kind,
                            DependencyKind::Informs,
                            "informs",
                        );
                    });
                if ui.button("Add edge").clicked()
                    && !self.new_dependency_from.is_empty()
                    && !self.new_dependency_to.is_empty()
                {
                    self.plan_draft.dependencies.push(DependencySpec {
                        from_task_id: self.new_dependency_from.clone(),
                        to_task_id: self.new_dependency_to.clone(),
                        kind: self.new_dependency_kind,
                    });
                }
            });
            let mut remove_edge = None;
            for (index, edge) in self.plan_draft.dependencies.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} {} {}",
                        task_title(&task_choices, &edge.from_task_id),
                        edge.kind.as_str(),
                        task_title(&task_choices, &edge.to_task_id)
                    ));
                    if ui.button("Remove").clicked() {
                        remove_edge = Some(index);
                    }
                });
            }
            if let Some(index) = remove_edge {
                self.plan_draft.dependencies.remove(index);
            }
            if let Some(view) = &snapshot {
                ui.separator();
                ui.heading("Plan review");
                if view.revisions.len() > 1 {
                    for summary in plan_changes(
                        &view.revisions[view.revisions.len() - 2].content,
                        &view.latest_content(),
                    ) {
                        ui.label(summary);
                    }
                }
                for revision in view.revisions.iter().rev() {
                    egui::CollapsingHeader::new(format!(
                        "Revision {} · {} · scope {} · {}",
                        revision.revision,
                        revision.status,
                        revision
                            .scope_revision
                            .map_or("—".to_owned(), |scope| scope.to_string()),
                        revision.created_at
                    ))
                    .id_salt(&revision.id)
                    .show(ui, |ui| {
                        ui.label(format!("Content hash: {}", revision.content_hash));
                        for milestone in &revision.content.milestones {
                            ui.label(format!("Milestone: {} — {}", milestone.title, milestone.outcome));
                        }
                        for epic in &revision.content.epics {
                            ui.label(format!("Epic: {} — {}", epic.title, epic.outcome));
                        }
                        for task in &revision.content.tasks {
                            ui.strong(format!("Task: {} · weight {}", task.title, task.weight));
                            ui.label(format!("Outcome: {}", task.outcome));
                            for criterion in &task.criteria {
                                ui.label(format!(
                                    "Criterion: {} · evidence: {}",
                                    criterion.assertion,
                                    criterion.required_evidence.join(", ")
                                ));
                            }
                        }
                        for edge in &revision.content.dependencies {
                            let choices: Vec<_> = revision
                                .content
                                .tasks
                                .iter()
                                .map(|task| (task.id.clone(), task.title.clone()))
                                .collect();
                            ui.label(format!(
                                "{} {} {}",
                                task_title(&choices, &edge.from_task_id),
                                edge.kind.as_str(),
                                task_title(&choices, &edge.to_task_id)
                            ));
                        }
                    });
                }
                ui.separator();
                ui.heading("Active milestones");
                if let Some(active_plan)=view.revisions.iter().find(|revision| revision.scope_revision==Some(active_scope)) {
                    for milestone in &active_plan.content.milestones {
                        ui.group(|ui| {
                            ui.strong(&milestone.title);
                            ui.label(&milestone.outcome);
                            for task_id in &milestone.task_ids {
                                let title=active_plan.content.tasks.iter().find(|task| &task.id==task_id)
                                    .map_or(task_id.as_str(),|task|task.title.as_str());
                                ui.label(format!("Task: {title}"));
                            }
                            let linked=self.review.as_ref().map_or(0,|review| review.decisions.iter()
                                .filter(|decision|decision.task_logical_id.as_ref()
                                    .is_some_and(|id|milestone.task_ids.contains(id))).count());
                            if linked>0 && ui.button(format!("Decisions ({linked})")).clicked() {
                                self.active_tab=Tab::Decisions;
                            }
                        });
                    }
                }
                ui.separator();
                ui.heading("Active scope tasks");
                for task in &view.active_tasks {
                    ui.group(|ui| {
                        ui.strong(format!(
                            "{} · {} · weight {}",
                            task.title, task.status, task.weight
                        ));
                        ui.label(&task.outcome);
                        let linked_decisions = self.review.as_ref().map_or(0, |review| review.decisions.iter()
                            .filter(|decision| decision.task_logical_id.as_deref() == Some(task.logical_id.as_str())).count());
                        if linked_decisions > 0 && ui.button(format!("Decisions ({linked_decisions})")).clicked() {
                            self.active_tab = Tab::Decisions;
                        }
                        if !task.unresolved_blockers.is_empty() {
                            ui.label(format!(
                                "Waiting for: {}",
                                task.unresolved_blockers.join(", ")
                            ));
                        }
                        if task.status == "ready"
                            && task.unresolved_blockers.is_empty()
                            && ui.button("Submit for review").clicked()
                        {
                            action =
                                Some(PlanAction::Submit(task.logical_id.clone(), task.revision));
                        }
                        for criterion in &task.criteria {
                            ui.label(format!(
                                "Criterion: {} · verifier: {}",
                                criterion.assertion, criterion.verifier
                            ));
                            if criterion.accepted_at.is_some() {
                                ui.label(format!(
                                    "Verified by {}",
                                    criterion.verified_by.as_deref().unwrap_or("unknown")
                                ));
                                for (kind, reference) in &criterion.evidence {
                                    ui.label(format!("{kind}: {reference}"));
                                }
                            } else if task.status == "review" {
                                let evidence = self
                                    .evidence_drafts
                                    .entry(criterion.logical_id.clone())
                                    .or_insert_with(|| criterion.evidence.clone());
                                for kind in &criterion.required_evidence {
                                    ui.horizontal(|ui| {
                                        ui.label(kind);
                                        ui.text_edit_singleline(
                                            evidence.entry(kind.clone()).or_default(),
                                        );
                                    });
                                }
                                if ui
                                    .button(format!("Verify criterion {}", criterion.assertion))
                                    .clicked()
                                {
                                    action = Some(PlanAction::Verify(
                                        task.logical_id.clone(),
                                        criterion.logical_id.clone(),
                                        task.revision,
                                        evidence.clone(),
                                    ));
                                }
                            }
                        }
                        if task.status == "review"
                            && task
                                .criteria
                                .iter()
                                .all(|criterion| criterion.accepted_at.is_some())
                            && ui.button("Accept task").clicked()
                        {
                            action =
                                Some(PlanAction::Accept(task.logical_id.clone(), task.revision));
                        }
                        if task.status == "review" || task.status == "blocked" {
                            ui.horizontal(|ui| {
                                ui.label(if task.status == "blocked" { "Retry reason" } else { "Changes needed" });
                                let reason = self.change_reasons.entry(task.logical_id.clone()).or_default();
                                ui.text_edit_singleline(reason);
                                let label = if task.status == "blocked" { "Retry task" } else { "Request changes" };
                                if ui.add_enabled(!reason.trim().is_empty(), egui::Button::new(label)).clicked() {
                                    action = Some(PlanAction::RequestChanges(
                                        task.logical_id.clone(), task.revision, reason.clone()));
                                }
                            });
                        }
                    });
                }
            }
        });
        if let Some(action) = action {
            self.run_plan_action(action);
        }
    }

    fn decisions_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(review) = self.review.clone() else {
            ui.heading("Decisions");
            ui.label("Select a project to review decisions.");
            return;
        };
        let mut action = None;
        let mut policy_action: Option<(String, i64, bool)> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Owner inbox");
            if review.inbox.is_empty() {
                ui.label("No pending requests.");
            }
            for item in &review.inbox {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("{} · {}", item.kind.to_uppercase(), item.title));
                    ui.label(&item.detail);
                    if item.kind == "brief" && ui.button("Open brief").clicked() {
                        self.active_tab = Tab::Brief;
                    }
                    if (item.kind == "plan" || item.kind == "task")
                        && ui.button("Open plan").clicked()
                    {
                        self.active_tab = Tab::Plan;
                    }
                });
            }
            ui.separator();
            if !review.policy_requests.is_empty() {
                ui.heading("Scoped action requests");
                for request in &review.policy_requests {
                    ui.group(|ui| {
                        ui.strong(&request.effect_summary);
                        ui.label(format!("Action: {}", request.action_class));
                        ui.label(format!("Exact target: {}", request.target));
                        ui.label(format!(
                            "Run: {} · Scope revision: {}",
                            request.run_id, request.scope_revision
                        ));
                        ui.label(format!("Command digest: {}", request.command_digest));
                        ui.horizontal(|ui| {
                            if ui.button("Approve once (15 min)").clicked() {
                                policy_action = Some((request.id.clone(), request.revision, true));
                            }
                            if ui.button("Deny").clicked() {
                                policy_action = Some((request.id.clone(), request.revision, false));
                            }
                        });
                    });
                }
                ui.separator();
            }
            ui.heading(if self.editing_decision.is_some() {
                "Edit proposed decision"
            } else {
                "Propose decision"
            });
            ui.label("Question");
            ui.text_edit_singleline(&mut self.decision_draft.question);
            ui.label("Task ID in active plan (optional)");
            let task_id = self
                .decision_draft
                .task_logical_id
                .get_or_insert_with(String::new);
            ui.text_edit_singleline(task_id);
            if task_id.trim().is_empty() {
                self.decision_draft.task_logical_id = None;
            }
            ui.label("Supersedes prior decision (optional)");
            ui.add_enabled_ui(self.editing_decision.is_none(), |ui| {
                egui::ComboBox::from_id_salt("supersedes-decision")
                    .selected_text(
                        self.decision_draft
                            .supersedes_id
                            .as_deref()
                            .unwrap_or("None"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.decision_draft.supersedes_id, None, "None");
                        for prior in &review.decisions {
                            if prior.status != "superseded" {
                                ui.selectable_value(
                                    &mut self.decision_draft.supersedes_id,
                                    Some(prior.id.clone()),
                                    format!("{} · {}", prior.id, prior.input.question),
                                );
                            }
                        }
                    });
            });
            ui.label("Alternatives (one per line)");
            let mut alternatives = self.decision_draft.alternatives.join("\n");
            if ui
                .add(egui::TextEdit::multiline(&mut alternatives).desired_rows(3))
                .changed()
            {
                self.decision_draft.alternatives = alternatives
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            ui.label("Recommended option (exactly as listed)");
            ui.text_edit_singleline(&mut self.decision_draft.recommendation);
            ui.label("Rationale");
            ui.add(egui::TextEdit::multiline(&mut self.decision_draft.rationale).desired_rows(2));
            ui.label("Evidence references (one per line)");
            let mut evidence = self.decision_draft.evidence.join("\n");
            if ui
                .add(egui::TextEdit::multiline(&mut evidence).desired_rows(2))
                .changed()
            {
                self.decision_draft.evidence = evidence
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            ui.horizontal(|ui| {
                ui.label("Impact");
                egui::ComboBox::from_id_salt("decision-impact")
                    .selected_text(&self.decision_draft.impact)
                    .show_ui(ui, |ui| {
                        for value in ["low", "medium", "high", "blocking"] {
                            ui.selectable_value(
                                &mut self.decision_draft.impact,
                                value.to_owned(),
                                value,
                            );
                        }
                    });
                if ui.button("Save proposal").clicked() {
                    action = Some(DecisionAction::Save);
                }
                if self.editing_decision.is_some() && ui.button("Cancel edit").clicked() {
                    self.clear_decision_draft();
                }
            });
            if let Some(message) = &self.decision_message {
                ui.label(message);
            }
            ui.separator();
            ui.heading("Decision register");
            for decision in &review.decisions {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(&decision.input.question);
                        ui.label(format!(
                            "{} · {} · revision {}",
                            decision.status, decision.input.impact, decision.revision
                        ));
                    });
                    if let Some(task) = &decision.task_logical_id {
                        ui.label(format!("Task: {task}"));
                    }
                    ui.label(format!(
                        "Alternatives: {}",
                        decision.input.alternatives.join(" / ")
                    ));
                    ui.label(format!("Recommended: {}", decision.input.recommendation));
                    ui.label(format!("Rationale: {}", decision.input.rationale));
                    if !decision.input.evidence.is_empty() {
                        ui.label(format!("Evidence: {}", decision.input.evidence.join(", ")));
                    }
                    if let Some(prior) = &decision.input.supersedes_id {
                        ui.label(format!("Supersedes: {prior}"));
                    }
                    ui.label(format!(
                        "Actor: {} · Created: {}",
                        decision.actor, decision.created_at
                    ));
                    if let Some(option) = &decision.selected_option {
                        ui.label(format!("Selected: {option}"));
                    }
                    if decision.status == "proposed" {
                        ui.horizontal(|ui| {
                            if ui.button("Edit").clicked() {
                                self.editing_decision = Some(decision.id.clone());
                                self.decision_draft = decision.input.clone();
                                self.selected_decision_options.insert(
                                    decision.id.clone(),
                                    decision.input.recommendation.clone(),
                                );
                            }
                            let choice_id = format!("choice-{}", decision.id);
                            let selected = self
                                .selected_decision_options
                                .entry(decision.id.clone())
                                .or_insert_with(|| decision.input.recommendation.clone());
                            egui::ComboBox::from_id_salt(choice_id)
                                .selected_text(selected.as_str())
                                .show_ui(ui, |ui| {
                                    for option in &decision.input.alternatives {
                                        ui.selectable_value(selected, option.clone(), option);
                                    }
                                });
                            if ui.button("Approve").clicked() {
                                action = Some(DecisionAction::Approve(
                                    decision.id.clone(),
                                    decision.revision,
                                    selected.clone(),
                                ));
                            }
                            if ui.button("Reject").clicked() {
                                action = Some(DecisionAction::Reject(
                                    decision.id.clone(),
                                    decision.revision,
                                ));
                            }
                        });
                    }
                });
            }
        });
        if let Some(action) = action {
            self.run_decision_action(action);
        }
        if let Some((id, revision, approve)) = policy_action {
            self.run_policy_review(&id, revision, approve);
        }
    }

    fn activity_workspace(&mut self, ui: &mut egui::Ui) {
        ui.heading("Activity history");
        let Some(review) = &self.review else {
            ui.label("Select a project.");
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            for event in &review.activity {
                ui.group(|ui| {
                    ui.strong(format!(
                        "{} · {} · {}",
                        event.created_at, event.operation, event.actor
                    ));
                    ui.label(format!(
                        "{} {} · revision {}",
                        event.subject_type, event.subject_id, event.subject_revision
                    ));
                    if let Some(after) = &event.after {
                        ui.label(after.to_string());
                    }
                    let destination = match event.subject_type.as_str() {
                        "decision" => Some((Tab::Decisions, "Open decision register")),
                        "brief" => Some((Tab::Brief, "Open brief")),
                        "plan" | "task" | "criterion" => Some((Tab::Plan, "Open plan")),
                        _ => None,
                    };
                    if let Some((tab, label)) = destination
                        && ui.button(label).clicked()
                    {
                        self.active_tab = tab;
                    }
                });
            }
        });
    }

    fn runs_workspace(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Agent runs");
            if ui.button("Refresh").clicked() {
                self.refresh_runs();
                self.refresh_plan();
                self.refresh_review();
            }
        });
        if let Some(message) = &self.run_message {
            ui.label(message);
        }
        if self.selected_project.is_none() {
            ui.label("Select a project to review runs.");
            return;
        }
        ui.group(|ui| {
            ui.strong("Start approved task");
            let ready: Vec<_> = self.plan.as_ref().map_or_else(Vec::new, |plan| plan.active_tasks.iter()
                .filter(|task| task.status == "ready" && task.unresolved_blockers.is_empty())
                .map(|task| (task.logical_id.clone(), task.title.clone())).collect());
            egui::ComboBox::from_id_salt("run-task")
                .selected_text(ready.iter().find(|(id, _)| id == &self.run_task_id)
                    .map_or("Select ready task", |(_, title)| title.as_str()))
                .show_ui(ui, |ui| {
                    for (id, title) in &ready {
                        ui.selectable_value(&mut self.run_task_id, id.clone(), title);
                    }
                });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.pi_selected, false, "OpenCode");
                ui.selectable_value(&mut self.pi_selected, true, "Pi RPC");
            });
            if self.pi_selected {
                ui.label("Pi executable or CLI JavaScript path (tested version 1.1.0)");
                ui.text_edit_singleline(&mut self.pi_path);
            } else {
                ui.label("OpenCode executable");
                ui.text_edit_singleline(&mut self.opencode_path);
            }
            ui.label("Project Pipeline CLI executable");
            ui.text_edit_singleline(&mut self.cli_path);
            ui.horizontal(|ui| {
                ui.label("Provider"); ui.text_edit_singleline(&mut self.provider_id);
                ui.label("Model"); ui.text_edit_singleline(&mut self.model_id);
            });
            ui.horizontal(|ui| {
                ui.label("Observed token limit");
                ui.add(egui::TextEdit::singleline(&mut self.token_budget_input).desired_width(90.0));
            });
            ui.label("Starting approves one prompt with the selected provider and model, then sends the approved task packet. Provider charges may apply. The run stops at 15 minutes or when observed usage reaches the selected token limit (which can be exceeded between observations).");
            let selected_ready = ready.iter().any(|(id, _)| id == &self.run_task_id);
            let token_budget = self.token_budget_input.trim().parse::<i64>().ok()
                .filter(|value| (1..=160_000).contains(value));
            if ui.add_enabled(self.run_control.is_none() && selected_ready
                && !self.provider_id.trim().is_empty() && !self.model_id.trim().is_empty()
                && token_budget.is_some(),
                egui::Button::new(format!("Approve {}-token stop and start run", token_budget.unwrap_or(0)))).clicked() { self.start_run(); }
            if self.run_control.is_some() && ui.button("Stop run").clicked()
                && let Some(control) = &self.run_control
            {
                let _ = control.commands.send(RunCommand::Stop);
            }
            let mut continued = Vec::new();
            for (id, summary) in &self.pending_permissions {
                ui.push_id(id, |ui| {
                    ui.label(format!("Permission pending: {summary}"));
                    if ui.button("Continue after owner decision").clicked() {
                        if let Some(control) = &self.run_control {
                            let _ = control.commands.send(RunCommand::ContinuePermission(id.clone()));
                        }
                        continued.push(id.clone());
                    }
                });
            }
            self.pending_permissions.retain(|(id, _)| !continued.contains(id));
        });
        if self.run_reviews.is_empty() {
            ui.label("No agent runs for this project.");
            return;
        }
        let mut requested_diff = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for run in &self.run_reviews {
                ui.push_id(&run.run_id, |ui| {
                    ui.group(|ui| {
                        ui.strong(format!(
                            "Task {} · run {} · {}",
                            run.task_id, run.run_id, run.state
                        ));
                        ui.label(format!("Checkout: {}", run.checkout_path));
                        if let Some(packet) = &run.packet {
                            ui.label(format!(
                                "Task packet SHA256: {} · {}",
                                run.packet_sha256.as_deref().unwrap_or("missing"),
                                if run.packet_verified {
                                    "verified"
                                } else {
                                    "INVALID"
                                }
                            ));
                            ui.collapsing("Approved task packet", |ui| {
                                ui.monospace(packet.to_string());
                            });
                        }
                        if ui.button("Review current diff").clicked() {
                            requested_diff = Some(run.run_id.clone());
                        }
                        if let Some((id, diff)) = &self.run_diff
                            && id == &run.run_id
                        {
                            ui.collapsing("Diff preview", |ui| {
                                ui.monospace(diff);
                            });
                        }
                        if let Some(submission) = &run.submission {
                            ui.collapsing("Agent submission", |ui| {
                                ui.label(submission.to_string());
                            });
                        }
                        ui.collapsing(
                            format!("Recent events ({}, max 200)", run.events.len()),
                            |ui| {
                                for event in &run.events {
                                    ui.label(format!(
                                        "{} · {} · {}",
                                        event.sequence, event.kind, event.summary
                                    ));
                                }
                            },
                        );
                        ui.collapsing(format!("Tests ({})", run.tests.len()), |ui| {
                            for test in &run.tests {
                                ui.label(format!(
                                    "{} · {} · exit {:?} · log {}",
                                    test.id,
                                    test.command,
                                    test.exit_code,
                                    test.log_artifact_id.as_deref().unwrap_or("missing")
                                ));
                            }
                        });
                        ui.collapsing(format!("Artifacts ({})", run.artifacts.len()), |ui| {
                            for artifact in &run.artifacts {
                                ui.label(format!(
                                    "{} · {} · {} · SHA256 {}",
                                    artifact.id, artifact.kind, artifact.uri, artifact.sha256
                                ));
                            }
                        });
                        let decisions = self.review.as_ref().map_or(0, |review| {
                            review
                                .decisions
                                .iter()
                                .filter(|decision| {
                                    decision.task_logical_id.as_deref()
                                        == Some(run.task_id.as_str())
                                })
                                .count()
                        });
                        if decisions > 0 && ui.button(format!("Decisions ({decisions})")).clicked()
                        {
                            self.active_tab = Tab::Decisions;
                        }
                        if run.submission.is_some() && ui.button("Review task criteria").clicked() {
                            self.active_tab = Tab::Plan;
                        }
                    })
                });
            }
        });
        if let Some(id) = requested_diff
            && let Some(engine) = &self.engine
        {
            match engine.review_run_diff(&id) {
                Ok(diff) => self.run_diff = Some((id, diff)),
                Err(error) => self.run_message = Some(error.to_string()),
            }
        }
    }

    fn management(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, name) in [
                (Tab::Overview, "Overview"),
                (Tab::Brief, "Brief"),
                (Tab::Research, "Research"),
                (Tab::Plan, "Plan"),
                (Tab::Decisions, "Decisions"),
                (Tab::Activity, "Activity"),
                (Tab::Runs, "Runs"),
                (Tab::About, "About"),
            ] {
                if ui.selectable_label(self.active_tab == tab, name).clicked() {
                    self.active_tab = tab;
                }
            }
        });
        ui.separator();
        let selected = self
            .projects
            .iter()
            .find(|project| self.selected_project.as_deref() == Some(&project.id));
        match self.active_tab {
            Tab::Overview => {
                if let Some(project) = selected {
                    ui.heading(&project.name);
                    ui.label(format!("Folder: {}", project.path));
                    ui.label(format!(
                        "Git root: {}",
                        project
                            .git_root
                            .as_deref()
                            .unwrap_or("No Git repository detected")
                    ));
                    ui.label(format!("Stage: {}", project.stage.as_str()));
                    ui.label(format!("Health: {}", project.health.as_str()));
                    ui.label(format!(
                        "Verified completion: {:.1}%",
                        project.verified_completion_basis_points as f32 / 100.0
                    ));
                    ui.label(format!("Blocked tasks: {}", project.blocked_count));
                    ui.separator();
                    if ui
                        .button(format!("Next owner action: {}", project.next_owner_action))
                        .clicked()
                    {
                        self.active_tab = Tab::Decisions;
                    }
                } else {
                    ui.heading("No project selected");
                    ui.label("Import an existing folder or create a new project.");
                }
            }
            Tab::Plan => {
                self.plan_workspace(ui);
            }
            Tab::Brief => self.brief_workspace(ui),
            Tab::Research => self.research_workspace(ui),
            Tab::Decisions => {
                self.decisions_workspace(ui);
            }
            Tab::Activity => self.activity_workspace(ui),
            Tab::Runs => {
                self.runs_workspace(ui);
            }
            Tab::About => {
                ui.heading("Project Pipeline");
                ui.label(format!(
                    "Version {} · local portfolio build",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.label("Windows and Apple Silicon macOS are the primary targets.");
            }
        }
    }

    fn open_next_action(&mut self, action: &NextAction) {
        self.active_tab = match action.destination {
            NextDestination::Brief => Tab::Brief,
            NextDestination::Plan => Tab::Plan,
            NextDestination::Decisions => Tab::Decisions,
            NextDestination::Runs => Tab::Runs,
            NextDestination::Overview => Tab::Overview,
        };
        self.advanced_open = true;
    }

    fn studio_card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
        let fill = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(34, 39, 39)
        } else {
            egui::Color32::from_rgb(255, 254, 251)
        };
        let stroke = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(68, 77, 75)
        } else {
            egui::Color32::from_rgb(218, 222, 216)
        };
        egui::Frame::group(ui.style())
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .inner_margin(egui::Margin::same(16))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.set_min_width((ui.available_width() - 32.0).clamp(120.0, 850.0));
                content(ui);
            });
    }

    fn studio_home(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your projects");
        ui.label("A clear view of what needs attention, based on saved project records.");
        ui.add_space(12.0);
        if self.projects.is_empty() {
            Self::studio_card(ui, |ui| {
                ui.heading("Start with a project");
                ui.label("Import an existing folder or create a new project in Advanced. No progress has been recorded yet.");
            });
            if ui.button("Open project setup").clicked() {
                self.advanced_open = true;
            }
            return;
        }
        if let Some(item) = self.global_inbox.first().cloned() {
            let name = self
                .projects
                .iter()
                .find(|p| p.id == item.project_id)
                .map_or("Project", |p| p.name.as_str())
                .to_owned();
            Self::studio_card(ui, |ui| {
                ui.strong("Needs attention");
                ui.heading(format!("{name}: {}", item.title));
                ui.label(&item.detail);
            });
            if ui.button("Open attention item").clicked() {
                self.select_project(item.project_id);
                self.active_tab = match item.kind.as_str() {
                    "brief" => Tab::Brief,
                    "plan" | "task" => Tab::Plan,
                    _ => Tab::Decisions,
                };
                self.advanced_open = true;
            }
        } else {
            ui.label("No owner requests are currently recorded.");
        }
        ui.add_space(12.0);
        let projects = self.projects.clone();
        for project in projects {
            Self::studio_card(ui, |ui| {
                ui.strong(&project.name);
                ui.label(format!(
                    "{} · {} · {:.1}% verified · {} blocked",
                    project.stage.as_str(),
                    project.health.as_str(),
                    project.verified_completion_basis_points as f32 / 100.0,
                    project.blocked_count
                ));
                ui.label(&project.next_owner_action);
            });
            if ui.button(format!("Open {}", project.name)).clicked() {
                self.select_project(project.id);
            }
            ui.add_space(8.0);
        }
    }

    fn studio_project(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self
            .projects
            .iter()
            .find(|p| self.selected_project.as_deref() == Some(&p.id))
            .cloned()
        else {
            ui.heading("Choose a project");
            return;
        };
        ui.heading(&project.name);
        ui.label(format!(
            "{} · {}",
            project.stage.as_str(),
            project.health.as_str().replace('_', " ")
        ));
        ui.add_space(12.0);
        if self
            .next_action_cache
            .as_ref()
            .is_none_or(|(id, at, _)| id != &project.id || at.elapsed() > Duration::from_secs(3))
        {
            let result = self
                .engine
                .as_ref()
                .ok_or_else(|| "Project data is unavailable".to_owned())
                .and_then(|engine| {
                    engine
                        .next_action(&project.id)
                        .map_err(|error| error.to_string())
                });
            self.next_action_cache = Some((project.id.clone(), Instant::now(), result));
        }
        let result = self
            .next_action_cache
            .as_ref()
            .map(|(_, _, result)| result.clone());
        match result {
            Some(Ok(action)) => {
                Self::studio_card(ui, |ui| {
                    ui.strong("Recommended next");
                    ui.heading(&action.title);
                    ui.label(&action.reason);
                    ui.add_space(6.0);
                    ui.label(format!("Before acting: {}", action.prerequisite));
                    ui.label(format!("Alternative: {}", action.alternative));
                    ui.small(
                        "Based on saved project records. Exact IDs are available in Advanced.",
                    )
                    .on_hover_text(format!("Source IDs: {}", action.source_ids.join(", ")));
                });
                if ui.button("Open record").clicked() {
                    self.open_next_action(&action);
                }
            }
            Some(Err(error)) => {
                ui.colored_label(
                    egui::Color32::from_rgb(175, 65, 55),
                    format!("Guidance unavailable: {error}"),
                );
            }
            None => {
                ui.label("Project data is unavailable.");
            }
        }
        ui.add_space(12.0);
        Self::studio_card(ui, |ui| {
            ui.strong("Project snapshot");
            if self
                .plan
                .as_ref()
                .is_some_and(|plan| plan.active_scope_revision > 0)
            {
                ui.label(format!(
                    "Verified completion: {:.1}% of approved scope",
                    project.verified_completion_basis_points as f32 / 100.0
                ));
            } else {
                ui.label("No approved work scope yet. Verified completion is unavailable.");
            }
            ui.label(format!("Blocked tasks: {}", project.blocked_count));
            ui.label(format!("Folder: {}", project.path));
        });
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (Tab::Brief, "Product direction"),
                (Tab::Plan, "Work plan"),
                (Tab::Decisions, "Decisions"),
                (Tab::Runs, "Work and evidence"),
            ] {
                if ui.button(label).clicked() {
                    self.active_tab = tab;
                    self.advanced_open = true;
                }
            }
        });
    }

    fn assistant_context(&self) -> Option<&str> {
        if self.studio_home {
            None
        } else {
            self.selected_project.as_deref()
        }
    }

    fn assistant_key(&self) -> String {
        self.assistant_context().unwrap_or("").to_owned()
    }

    fn send_assistant(&mut self) {
        let key = self.assistant_key();
        let question = self
            .assistant_drafts
            .active(if key.is_empty() { None } else { Some(&key) })
            .trim();
        if question.is_empty() || question.len() > assistant::MAX_QUESTION {
            return;
        }
        let question = sanitize_assistant_text(question, assistant::MAX_QUESTION);
        if self
            .assistant_sessions
            .get(&key)
            .is_some_and(|s| s.receiver.is_some())
        {
            return;
        }
        if !self.assistant_sessions.contains_key(&key)
            && self.assistant_sessions.len() >= 32
            && let Some(oldest) = self.assistant_sessions.keys().next().cloned()
            && let Some(session) = self.assistant_sessions.remove(&oldest)
            && let Some(cancel) = session.cancel
        {
            cancel.store(true, Ordering::Relaxed);
        }
        let session = self.assistant_sessions.entry(key.clone()).or_default();
        if session.turns.len() >= 12 {
            session.turns.remove(0);
        }
        let mut turn = ChatTurn {
            question: question.clone(),
            answer: String::new(),
            status: "Loading".into(),
            sources: Vec::new(),
            usage: "Cost unavailable".into(),
            requested_identity: if self.assistant_config.provider.is_empty() {
                "No provider requested".into()
            } else {
                format!(
                    "Requested: {}/{}",
                    sanitize_assistant_text(&self.assistant_config.provider, 80),
                    sanitize_assistant_text(&self.assistant_config.model, 80)
                )
            },
            reported_model: None,
        };
        if key.is_empty() {
            turn.answer =
                "Select a project for record-backed guidance. No provider was contacted.".into();
            turn.status = "Offline guidance".into();
            session.turns.push(turn);
            self.assistant_drafts.active_mut(None).clear();
            return;
        }
        let Some(engine) = self.engine.as_ref() else {
            turn.status = "Project data unavailable".into();
            session.turns.push(turn);
            return;
        };
        let context = match engine.assistant_context(&key) {
            Ok(context) => context,
            Err(_) => {
                turn.status = "Project context unavailable".into();
                session.turns.push(turn);
                return;
            }
        };
        turn.sources = context.records.clone();
        session.fingerprint = context.fingerprint.clone();
        if let Err(error) = self.assistant_config.validate() {
            let action = engine.next_action(&key).ok();
            let overview = self.projects.iter().find(|project| project.id == key);
            turn.answer = format!(
                "Observed: {}.\n\nInterpretation: {}.\n\nUnknown: Live answers are unavailable ({error}). No provider was contacted. Changes require a future typed proposal and owner review.",
                overview.map_or("Project status unavailable".to_owned(), |project| format!(
                    "{} is in {:?} with {} blocked tasks",
                    sanitize_assistant_text(&project.name, 100),
                    project.stage,
                    project.blocked_count
                )),
                action.map_or("Review the project records".to_owned(), |action| format!(
                    "{} — {}",
                    sanitize_assistant_text(&action.title, 120),
                    sanitize_assistant_text(action.reason.trim_end_matches('.'), 240)
                ))
            );
            turn.status = "Offline guidance".into();
            session.turns.push(turn);
        } else {
            if self
                .assistant_workers
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                    (n < MAX_ASSISTANT_WORKERS).then_some(n + 1)
                })
                .is_err()
            {
                turn.status = "Earlier requests are finishing; retry shortly".into();
                session.turns.push(turn);
                return;
            }
            let config = self.assistant_config.clone();
            let (tx, rx) = mpsc::sync_channel(128);
            let cancel = Arc::new(AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let worker_count = self.assistant_workers.clone();
            std::thread::spawn(move || {
                let _slot = AssistantWorkerSlot(worker_count);
                CompatibleProvider.respond(
                    &config,
                    &context,
                    &question,
                    &worker_cancel,
                    &mut |event| {
                        if !worker_cancel.load(Ordering::Acquire) && tx.try_send(event).is_err() {
                            worker_cancel.store(true, Ordering::Release);
                        }
                    },
                );
            });
            session.turns.push(turn);
            session.receiver = Some(rx);
            session.cancel = Some(cancel);
        }
        self.assistant_drafts.active_mut(Some(&key)).clear();
    }

    fn poll_assistant(&mut self, ctx: &egui::Context) {
        let current_project = self.assistant_key();
        for (project_id, session) in &mut self.assistant_sessions {
            if *project_id != current_project && session.receiver.is_some() {
                if let Some(cancel) = &session.cancel {
                    cancel.store(true, Ordering::Release);
                }
                session.receiver = None;
                session.cancel = None;
                if let Some(turn) = session.turns.last_mut() {
                    turn.status = "Cancelled on project switch".into();
                }
                continue;
            }
            let mut finished = false;
            if let Some(receiver) = &session.receiver {
                loop {
                    let event = match receiver.try_recv() {
                        Ok(event) => event,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            finished = true;
                            if let Some(turn) = session.turns.last_mut()
                                && (turn.status == "Loading" || turn.status == "Streaming")
                            {
                                turn.status = "Provider disconnected".into();
                            }
                            break;
                        }
                    };
                    let Some(turn) = session.turns.last_mut() else {
                        break;
                    };
                    match event {
                        ChatEvent::Delta(chunk) => {
                            turn.answer.push_str(&chunk);
                            if turn.status != "Buffered" {
                                turn.status = "Streaming".into();
                            }
                        }
                        ChatEvent::Usage(value) => turn.usage = value,
                        ChatEvent::Model(value) => {
                            turn.reported_model = Some(sanitize_assistant_text(&value, 120));
                        }
                        ChatEvent::Buffered => turn.status = "Buffered".into(),
                        ChatEvent::Done => {
                            if turn.status != "Buffered" {
                                turn.status = "Complete".into();
                            }
                            finished = true;
                        }
                        ChatEvent::Error(error) => {
                            turn.status = error;
                            finished = true;
                        }
                    }
                }
            }
            if finished {
                session.receiver = None;
                session.cancel = None;
                if !project_id.is_empty()
                    && self
                        .engine
                        .as_ref()
                        .and_then(|engine| engine.assistant_context(project_id).ok())
                        .is_none_or(|fresh| fresh.fingerprint != session.fingerprint)
                    && let Some(turn) = session.turns.last_mut()
                {
                    turn.status = "Stale project context — refresh and retry".into();
                }
            } else if session.receiver.is_some() {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }

    fn cancel_assistant(&mut self) {
        if let Some(session) = self.assistant_sessions.get_mut(&self.assistant_key()) {
            if let Some(cancel) = &session.cancel {
                cancel.store(true, Ordering::Release);
            }
            session.receiver = None;
            session.cancel = None;
            if let Some(turn) = session.turns.last_mut() {
                turn.status = "Cancelled".into();
            }
        }
    }

    fn proposal_content(&mut self, ui: &mut egui::Ui, project_id: &str) {
        ui.separator();
        ui.heading("Propose a draft change");
        ui.label("Manual entry works offline. Assistant replies are untrusted suggestions until validated below.");
        let mut candidates: Vec<(String, String, String, i64, i64, i64, String)> = Vec::new();
        if let Some(engine) = &self.engine
            && let (Ok(discovery), Ok(plan)) = (
                engine.load_discovery(project_id),
                engine.load_plan(project_id),
            )
        {
            if let Some(brief) = discovery.briefs.last() {
                candidates.push((
                    "Brief · idea".into(),
                    "brief_field".into(),
                    brief.id.clone(),
                    brief.revision,
                    discovery.approved_revision,
                    plan.active_scope_revision,
                    brief.content.idea.clone(),
                ));
            }
            if let Some(revision) = plan.revisions.last()
                && revision.status == "draft"
            {
                for task in &revision.content.tasks {
                    candidates.push((
                        format!("Task · {}", task.id),
                        "task_draft".into(),
                        task.id.clone(),
                        revision.revision,
                        discovery.approved_revision,
                        plan.active_scope_revision,
                        task.title.clone(),
                    ));
                }
                for milestone in &revision.content.milestones {
                    candidates.push((
                        format!("Plan milestone · {}", milestone.id),
                        "plan_draft".into(),
                        milestone.id.clone(),
                        revision.revision,
                        discovery.approved_revision,
                        plan.active_scope_revision,
                        milestone.title.clone(),
                    ));
                }
            }
        }
        if !self.proposal_drafts.contains_key(project_id)
            && self.proposal_drafts.len() >= 32
            && let Some(oldest) = self.proposal_drafts.keys().next().cloned()
        {
            self.proposal_drafts.remove(&oldest);
        }
        let state = self
            .proposal_drafts
            .entry(project_id.to_owned())
            .or_default();
        if let Some(previous) = state.preview.as_ref() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
            match self
                .engine
                .as_ref()
                .unwrap()
                .preview_proposal(&previous.proposal)
            {
                Ok(fresh) if fresh == *previous => {}
                Ok(_) | Err(_) => {
                    state.invalidate("Proposal changed or became stale. Review a fresh proposal from current values.");
                }
            }
        }
        if candidates.is_empty() {
            ui.label("Create a brief or a plan draft to begin.");
            return;
        }
        if state.kind >= candidates.len() {
            state.kind = 0;
            state.invalidate("Proposal target changed. Review a new preview before applying.");
        }
        let old_kind = state.kind;
        egui::ComboBox::from_id_salt("proposal_kind")
            .selected_text(&candidates[state.kind].0)
            .show_ui(ui, |ui| {
                for (index, candidate) in candidates.iter().enumerate() {
                    ui.selectable_value(&mut state.kind, index, &candidate.0);
                }
            });
        if state.kind != old_kind {
            state.after.clear();
            state.invalidate("Proposal target changed. Review a new preview before applying.");
        }
        let (label, kind, target, revision, approved_brief, scope, before) =
            &candidates[state.kind];
        state.invalidate_if_input_changed(target);
        ui.label(format!("Current {label}: {before}"));
        let edit = ui.add(
            egui::TextEdit::multiline(&mut state.after)
                .desired_rows(3)
                .char_limit(2_000)
                .hint_text("Proposed text"),
        );
        if edit.changed() {
            state.invalidate_if_input_changed(target);
        }
        let build = ui.button("Review manual proposal").clicked();
        let parse_answer = ui
            .button("Review latest assistant JSON suggestion")
            .clicked();
        if build {
            let evidence_id = if kind == "brief_field" {
                target.clone()
            } else {
                self.engine
                    .as_ref()
                    .and_then(|engine| engine.load_plan(project_id).ok())
                    .and_then(|plan| plan.revisions.last().map(|item| item.id.clone()))
                    .unwrap_or_default()
            };
            let raw = serde_json::json!({
                "version": 1, "id": Uuid::new_v4().to_string(), "project_id": project_id,
                "expected_revision": revision, "expected_approved_brief_revision": approved_brief,
                "expected_scope_revision": scope, "original_evidence_ids": [evidence_id],
                "reason": "Owner-entered draft suggestion", "impact": "New draft revision only",
                "affected_approved_scope": "None applied", "prerequisites": "Review current draft",
                "risk": "Review content before apply", "required_owner_approvals": "Explicit apply; later brief or plan approval remains separate",
                "recovery": "Restore previous text in a new draft revision",
                "change": {"type": kind, "target_id": target, "field": if kind == "brief_field" { "idea" } else { "title" }, "before": before, "after": state.after}
            }).to_string();
            match Proposal::parse_json(&raw)
                .and_then(|proposal| self.engine.as_ref().unwrap().preview_proposal(&proposal))
            {
                Ok(preview) => {
                    state.load_preview(preview, target, "Validated against current records.");
                }
                Err(error) => {
                    state.invalidate(&error.to_string());
                }
            }
        }
        if parse_answer {
            let answer = self
                .assistant_sessions
                .get(project_id)
                .and_then(|session| session.turns.last())
                .map(|turn| turn.answer.as_str())
                .unwrap_or("");
            match Proposal::parse_json(answer).and_then(|proposal| {
                if proposal.project_id != project_id {
                    return Err(pipeline_engine::ProposalError::Conflict);
                }
                self.engine.as_ref().unwrap().preview_proposal(&proposal)
            }) {
                Ok(preview) => {
                    state.load_preview(
                        preview,
                        target,
                        "Assistant suggestion validated against current records.",
                    );
                }
                Err(error) => {
                    state.invalidate(&format!("Non-executable suggestion: {error}"));
                }
            }
        }
        if !state.message.is_empty() {
            ui.label(&state.message);
        }
        let mut apply = false;
        let mut discard = false;
        let mut repreview = false;
        if let Some(preview) = &state.preview {
            ui.group(|ui| {
                ui.strong(match preview.status {
                    ProposalStatus::Validated => "Validated proposal",
                    ProposalStatus::RequiresSeparateApproval => {
                        "Requires separate approval / Not supported yet"
                    }
                });
                ui.label(format!(
                    "Project: {} · Revision: {}",
                    self.projects
                        .iter()
                        .find(|p| p.id == project_id)
                        .map_or(project_id, |p| p.name.as_str()),
                    preview.proposal.expected_revision
                ));
                let (target, before, after) = match &preview.proposal.change {
                    pipeline_engine::ProposalChange::BriefField {
                        target_id,
                        before,
                        after,
                        ..
                    }
                    | pipeline_engine::ProposalChange::TaskDraft {
                        target_id,
                        before,
                        after,
                        ..
                    }
                    | pipeline_engine::ProposalChange::PlanDraft {
                        target_id,
                        before,
                        after,
                        ..
                    } => (target_id, before, after),
                };
                ui.label(format!(
                    "Target: {} · ID: {target}",
                    match &preview.proposal.change {
                        pipeline_engine::ProposalChange::BriefField { field, .. } =>
                            format!("Product brief · {field:?}"),
                        pipeline_engine::ProposalChange::TaskDraft { field, .. } =>
                            format!("Task draft · {field:?}"),
                        pipeline_engine::ProposalChange::PlanDraft { field, .. } =>
                            format!("Plan milestone draft · {field:?}"),
                    }
                ));
                ui.label(format!("Before: {before}"));
                ui.label(format!("After: {after}"));
                ui.label("Suggestion notes (unverified; these cannot grant authority):");
                ui.label(format!("Reason: {}", preview.proposal.reason));
                ui.label(format!(
                    "Impact: {} · Claimed scope: {}",
                    preview.proposal.impact, preview.proposal.affected_approved_scope
                ));
                ui.label(format!(
                    "Risk: {} · Prerequisites: {}",
                    preview.proposal.risk, preview.proposal.prerequisites
                ));
                ui.label(format!(
                    "Claimed approvals: {}",
                    preview.proposal.required_owner_approvals
                ));
                ui.label(format!("Recovery guidance: {}", preview.proposal.recovery));
                ui.label(&preview.explanation);
                ui.collapsing("Advanced evidence", |ui| {
                    ui.label(format!("Project ID: {}", preview.proposal.project_id));
                    ui.label(format!("Record ID: {}", preview.authoritative_record_id));
                    ui.label(format!("Target ID: {target}"));
                    ui.label(format!(
                        "Evidence IDs: {:?}",
                        preview.proposal.original_evidence_ids
                    ));
                    ui.label(format!("Proposal ID: {}", preview.proposal.id));
                });
                if preview.status == ProposalStatus::Validated {
                    ui.checkbox(
                        &mut state.owner_confirmed,
                        format!(
                            "I approve this exact Before / After as a new draft at revision {} (proposal {}).",
                            preview.proposal.expected_revision,
                            preview.proposal.id
                        ),
                    );
                    apply = ui
                        .add_enabled(
                            state.owner_confirmed,
                            egui::Button::new("Apply draft change"),
                        )
                        .clicked();
                }
                repreview = ui.button("Re-preview current state").clicked();
                discard = ui.button("Cancel / Discard").clicked();
            });
        }
        if repreview && let Some(preview) = &state.preview {
            match self
                .engine
                .as_ref()
                .unwrap()
                .preview_proposal(&preview.proposal)
            {
                Ok(fresh) => {
                    state.load_preview(fresh, target, "Preview refreshed; confirm again to apply.");
                }
                Err(error) => {
                    state.invalidate(&format!(
                        "Conflict: {error}. Create a new proposal from current values."
                    ));
                }
            }
        }
        if discard {
            state.invalidate("Proposal discarded without changes.");
        }
        if apply && let Some(preview) = state.preview.take() {
            state.preview_input = None;
            state.owner_confirmed = false;
            match self.engine.as_mut().unwrap().apply_proposal(&preview, true) {
                Ok(()) => {
                    state.message = "Applied as a new draft revision. Separate brief or plan approval is still required.".into();
                    self.refresh_discovery();
                    self.refresh_plan();
                    self.refresh_review();
                    if let Some(engine) = &self.engine
                        && let Ok(projects) = engine.list_overviews()
                    {
                        self.projects = projects;
                    }
                    self.refresh_global_inbox();
                }
                Err(error) => {
                    state.message =
                        format!("Apply blocked: {error}. Re-preview before trying again.");
                }
            }
        }
    }

    fn assistant_content(&mut self, ui: &mut egui::Ui) {
        ui.heading("Project assistant");
        ui.label("Chat is read-only. Typed changes require validation and a separate owner Apply.");
        ui.separator();
        ui.label(if self.studio_home {
            "Context: all projects".to_owned()
        } else {
            format!(
                "Context: {}",
                self.projects
                    .iter()
                    .find(|p| self.selected_project.as_deref() == Some(&p.id))
                    .map_or("no project", |p| p.name.as_str())
            )
        });
        ui.collapsing("Provider settings", |ui| {
            ui.label("Local loopback inference and verified OpenRouter free variants are supported. Paid remote calls remain blocked.");
            if ui.button("Use OpenRouter free model").clicked() {
                self.assistant_config.provider = "OpenRouter".into();
                self.assistant_config.model = "nvidia/nemotron-3.5-lightning:free".into();
                self.assistant_config.endpoint = "https://openrouter.ai/api/v1/chat/completions".into();
                self.assistant_config.credential_env = "OPENROUTER_API_KEY".into();
            }
            ui.label("Provider"); ui.text_edit_singleline(&mut self.assistant_config.provider);
            ui.label("Model"); ui.text_edit_singleline(&mut self.assistant_config.model);
            ui.label("Endpoint"); ui.text_edit_singleline(&mut self.assistant_config.endpoint);
            ui.label("Credential environment variable (PIPELINE_ASSISTANT_* locally; OPENROUTER_API_KEY for OpenRouter)");
            ui.text_edit_singleline(&mut self.assistant_config.credential_env);
        });
        let key = self.assistant_key();
        ui.label(format!(
            "Configured provider: {} · Requested model: {} · Cost: unavailable",
            if self.assistant_config.provider.is_empty() {
                "none"
            } else {
                &self.assistant_config.provider
            },
            if self.assistant_config.model.is_empty() {
                "none"
            } else {
                &self.assistant_config.model
            }
        ));
        if let Some(session) = self.assistant_sessions.get(&key) {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for turn in &session.turns {
                        ui.separator();
                        ui.strong(&turn.question);
                        ui.label(&turn.status);
                        ui.label(&turn.answer);
                        ui.small(&turn.requested_identity);
                        if turn.status != "Offline guidance" {
                            ui.small(format!(
                                "Provider-reported model: {}",
                                turn.reported_model.as_deref().unwrap_or("unavailable")
                            ));
                        }
                        ui.collapsing(format!("Sources ({})", turn.sources.len()), |ui| {
                            for source in &turn.sources {
                                ui.small(source);
                            }
                        });
                        ui.small(&turn.usage);
                    }
                });
        }
        if !key.is_empty() {
            self.proposal_content(ui, &key);
        }
        let context = self.assistant_context().map(str::to_owned);
        ui.add(
            egui::TextEdit::multiline(self.assistant_drafts.active_mut(context.as_deref()))
                .desired_rows(4)
                .hint_text("Ask about this project"),
        );
        ui.horizontal(|ui| {
            if ui.button("Send question").clicked() {
                self.send_assistant();
            }
            if self
                .assistant_sessions
                .get(&key)
                .is_some_and(|s| s.receiver.is_some())
                && ui.button("Cancel").clicked()
            {
                self.cancel_assistant();
            }
            if ui.button("Retry").clicked()
                && let Some(last) = self
                    .assistant_sessions
                    .get(&key)
                    .and_then(|s| s.turns.last())
            {
                let question = last.question.clone();
                *self
                    .assistant_drafts
                    .active_mut(if key.is_empty() { None } else { Some(&key) }) = question;
                self.send_assistant();
            }
        });
        if ui.button("Close guidance (Esc)").clicked() {
            self.assistant_open = false;
            self.focus_return = true;
        }
    }

    fn modern_ui(&mut self, ui: &mut egui::Ui) {
        if self.studio_home {
            for draft in self.proposal_drafts.values_mut() {
                draft.owner_confirmed = false;
            }
        }
        let ctx = ui.ctx().clone();
        self.poll_assistant(&ctx);
        self.poll_run(&ctx);
        let mut visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.override_text_color = Some(if self.dark_mode {
            egui::Color32::from_rgb(235, 237, 235)
        } else {
            egui::Color32::from_rgb(42, 48, 47)
        });
        visuals.selection.bg_fill = egui::Color32::from_rgb(25, 119, 112);
        visuals.panel_fill = if self.dark_mode {
            egui::Color32::from_rgb(26, 30, 30)
        } else {
            egui::Color32::from_rgb(248, 247, 243)
        };
        visuals.widgets.inactive.bg_fill = if self.dark_mode {
            egui::Color32::from_rgb(48, 57, 55)
        } else {
            egui::Color32::from_rgb(232, 238, 234)
        };
        visuals.widgets.hovered.bg_fill = if self.dark_mode {
            egui::Color32::from_rgb(29, 103, 97)
        } else {
            egui::Color32::from_rgb(190, 226, 218)
        };
        ctx.set_visuals(visuals);
        ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 8.0);
        ui.style_mut().spacing.button_padding = egui::vec2(10.0, 6.0);
        ctx.set_zoom_factor(self.text_scale);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.assistant_open {
            self.assistant_open = false;
            self.focus_return = true;
        }
        if ctx.input(|i| composer_shortcut(&i.events)) {
            self.focus_composer = true;
        }
        let narrow = ui.available_width() < 860.0;
        egui::Panel::top("studio_header").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(egui::RichText::new("Project Pipeline").size(18.0).color(
                    if self.dark_mode {
                        egui::Color32::from_rgb(80, 190, 177)
                    } else {
                        egui::Color32::from_rgb(26, 122, 115)
                    },
                ));
                ui.separator();
                if ui.button("Home").clicked() {
                    self.studio_home = true;
                    self.advanced_open = false;
                }
                if ui.button("Projects").clicked() {
                    self.studio_home = true;
                    self.advanced_open = false;
                }
                let current = self
                    .projects
                    .iter()
                    .find(|p| self.selected_project.as_deref() == Some(&p.id))
                    .map_or("Select project", |p| p.name.as_str())
                    .to_owned();
                let mut picked = None;
                egui::ComboBox::from_id_salt("studio_project_switcher")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for project in &self.projects {
                            if ui
                                .selectable_label(
                                    self.selected_project.as_deref() == Some(&project.id),
                                    &project.name,
                                )
                                .clicked()
                            {
                                picked = Some(project.id.clone());
                            }
                        }
                    });
                if let Some(id) = picked {
                    self.select_project(id);
                    self.advanced_open = false;
                }
                if ui
                    .button(format!("Inbox ({})", self.global_inbox.len()))
                    .clicked()
                {
                    self.studio_home = true;
                }
                if ui.button("Advanced").clicked() {
                    self.advanced_open = !self.advanced_open;
                }
                let guidance = ui
                    .push_id("assistant_toggle", |ui| ui.button("Guidance"))
                    .inner;
                if self.focus_return {
                    guidance.request_focus();
                    self.focus_return = false;
                }
                if guidance.clicked() {
                    self.assistant_open = !self.assistant_open;
                }
                if ui.button("A−").clicked() {
                    self.text_scale = (self.text_scale - 0.1).max(0.8);
                }
                if ui.button("A+").clicked() {
                    self.text_scale = (self.text_scale + 0.1).min(2.0);
                }
                ui.checkbox(&mut self.dark_mode, "Dark");
            });
        });
        if !self.advanced_open {
            egui::Panel::bottom("studio_composer").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ask");
                    let context = self.assistant_context().map(str::to_owned);
                    let response = ui.add(
                        egui::TextEdit::singleline(
                            self.assistant_drafts.active_mut(context.as_deref()),
                        )
                        .desired_width(ui.available_width() - 140.0)
                        .hint_text("Ask about the selected project"),
                    );
                    if self.focus_composer {
                        response.request_focus();
                        self.focus_composer = false;
                    }
                    if ui.button("Send").clicked() {
                        self.send_assistant();
                        self.assistant_open = true;
                    }
                    if ui.button("Open").clicked() {
                        self.assistant_open = true;
                    }
                });
            });
        }
        if self.assistant_open && !narrow && !self.advanced_open {
            egui::Panel::right("studio_assistant")
                .resizable(true)
                .default_size(400.0)
                .min_size(320.0)
                .show(ui, |ui| {
                    let scroll = egui::ScrollArea::vertical();
                    #[cfg(feature = "screenshot-evidence")]
                    let scroll = if let Ok(offset) = std::env::var("PIPELINE_UX_PROPOSAL_SCROLL") {
                        scroll.vertical_scroll_offset(offset.parse().unwrap_or(0.0))
                    } else {
                        scroll
                    };
                    scroll.show(ui, |ui| self.assistant_content(ui));
                });
        }
        if self.advanced_open {
            egui::Panel::top("advanced_tools").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_portfolio, "Project list (F1)");
                    ui.checkbox(&mut self.show_terminal, "Terminal (F2)");
                    if ui.button("Back to overview").clicked() {
                        self.advanced_open = false;
                        self.next_action_cache = None;
                    }
                });
            });
            if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
                self.show_portfolio = !self.show_portfolio;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
                self.show_terminal = !self.show_terminal;
            }
            if self.show_portfolio {
                self.portfolio(ui);
            }
            for tab in &mut self.terminals {
                if let Err(error) = tab.session.poll(32 * 1024) {
                    tab.error = Some(error.to_string());
                }
            }
            if !self.terminals.is_empty() {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
            if self.show_terminal {
                let height = ui.available_height();
                let panel = egui::Panel::bottom("terminal")
                    .resizable(true)
                    .default_size(height * self.terminal_fraction)
                    .min_size(120.0)
                    .show(ui, |ui| self.terminal_area(ui));
                self.terminal_fraction =
                    (panel.response.rect.height() / height.max(1.0)).clamp(0.2, 0.75);
            }
            egui::CentralPanel::default().show(ui, |ui| self.management(ui));
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.studio_home {
                        self.studio_home(ui);
                    } else {
                        self.studio_project(ui);
                    }
                });
            });
            if self.assistant_open && narrow {
                egui::Window::new("Guidance")
                    .id(egui::Id::new("studio_assistant_drawer"))
                    .anchor(egui::Align2::RIGHT_CENTER, [-8.0, 0.0])
                    .default_width(300.0)
                    .show(&ctx, |ui| {
                        ui.set_max_height(ctx.content_rect().height() - 24.0);
                        egui::ScrollArea::vertical().show(ui, |ui| self.assistant_content(ui));
                    });
            }
        }
        #[cfg(feature = "screenshot-evidence")]
        self.capture_native(&ctx);
    }

    #[cfg(feature = "screenshot-evidence")]
    fn capture_native(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.capture_path.as_ref() {
            for event in ctx.input(|input| input.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(error) = image::save_buffer(
                        path,
                        &pixels,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("screenshot failed: {error}");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if self.capture_frames == 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            } else if self.capture_frames < 4 {
                self.capture_frames += 1;
                ctx.request_repaint();
            }
        }
    }
}

fn brief_field(ui: &mut egui::Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
}

fn plan_lines(ui: &mut egui::Ui, label: &str, values: &mut Vec<String>) {
    ui.label(label);
    let mut remove = None;
    for (index, value) in values.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(value);
            if ui.button("Remove").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        values.remove(index);
    }
    if ui.button(format!("+ Add {label}")).clicked() {
        values.push(String::new());
    }
}

fn task_title<'a>(choices: &'a [(String, String)], id: &'a str) -> &'a str {
    choices
        .iter()
        .find(|(candidate, _)| candidate == id)
        .map_or(id, |(_, title)| title.as_str())
}

fn task_choice(
    ui: &mut egui::Ui,
    label: &str,
    selected: &mut String,
    choices: &[(String, String)],
) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(task_title(choices, selected))
        .show_ui(ui, |ui| {
            for (id, title) in choices {
                ui.selectable_value(selected, id.clone(), title);
            }
        });
}

fn plan_changes(before: &PlanContent, after: &PlanContent) -> Vec<String> {
    let mut changes = Vec::new();
    for milestone in &after.milestones {
        match before.milestones.iter().find(|old| old.id == milestone.id) {
            None => changes.push(format!("Added milestone: {}", milestone.title)),
            Some(old) if old != milestone => {
                changes.push(format!("Changed milestone: {}", milestone.title))
            }
            _ => {}
        }
    }
    for milestone in &before.milestones {
        if !after.milestones.iter().any(|new| new.id == milestone.id) {
            changes.push(format!("Removed milestone: {}", milestone.title));
        }
    }
    for epic in &after.epics {
        match before.epics.iter().find(|old| old.id == epic.id) {
            None => changes.push(format!("Added epic: {}", epic.title)),
            Some(old) if old != epic => {
                changes.push(format!(
                    "Changed epic: {} → {}; outcome: {} → {}",
                    old.title, epic.title, old.outcome, epic.outcome
                ));
            }
            _ => {}
        }
    }
    for epic in &before.epics {
        if !after.epics.iter().any(|new| new.id == epic.id) {
            changes.push(format!("Removed epic: {}", epic.title));
        }
    }
    for task in &after.tasks {
        match before.tasks.iter().find(|old| old.id == task.id) {
            None => changes.push(format!("Added task: {}", task.title)),
            Some(old) if old != task => {
                for (field, from, to) in [
                    ("title", old.title.as_str(), task.title.as_str()),
                    ("outcome", old.outcome.as_str(), task.outcome.as_str()),
                    ("risk", old.risk.as_str(), task.risk.as_str()),
                    (
                        "estimate",
                        old.estimate_band.as_str(),
                        task.estimate_band.as_str(),
                    ),
                ] {
                    if from != to {
                        changes.push(format!("{} {field}: {from} → {to}", task.title));
                    }
                }
                if old.weight != task.weight {
                    changes.push(format!(
                        "{} weight: {} → {}",
                        task.title, old.weight, task.weight
                    ));
                }
                if old.epic_id != task.epic_id {
                    changes.push(format!("{} epic assignment changed", task.title));
                }
                for (field, from, to) in [
                    (
                        "verification commands",
                        &old.verification_commands,
                        &task.verification_commands,
                    ),
                    ("deliverables", &old.deliverables, &task.deliverables),
                    ("context links", &old.context_links, &task.context_links),
                    (
                        "owner decision triggers",
                        &old.owner_decision_triggers,
                        &task.owner_decision_triggers,
                    ),
                ] {
                    if from != to {
                        changes.push(format!(
                            "{} {field}: {} → {}",
                            task.title,
                            from.join(", "),
                            to.join(", ")
                        ));
                    }
                }
                for criterion in &task.criteria {
                    match old.criteria.iter().find(|prior| prior.id == criterion.id) {
                        None => changes.push(format!(
                            "{} added criterion: {}",
                            task.title, criterion.assertion
                        )),
                        Some(prior) if prior != criterion => changes.push(format!(
                            "{} criterion: {} [{}] → {} [{}]",
                            task.title,
                            prior.assertion,
                            prior.required_evidence.join(", "),
                            criterion.assertion,
                            criterion.required_evidence.join(", ")
                        )),
                        _ => {}
                    }
                }
                for criterion in &old.criteria {
                    if !task.criteria.iter().any(|new| new.id == criterion.id) {
                        changes.push(format!(
                            "{} removed criterion: {}",
                            task.title, criterion.assertion
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    for task in &before.tasks {
        if !after.tasks.iter().any(|new| new.id == task.id) {
            changes.push(format!("Removed task: {}", task.title));
        }
    }
    for edge in &after.dependencies {
        if !before.dependencies.contains(edge) {
            changes.push(format!(
                "Added {} edge: {} → {}",
                edge.kind.as_str(),
                after
                    .tasks
                    .iter()
                    .find(|task| task.id == edge.from_task_id)
                    .map_or(edge.from_task_id.as_str(), |task| task.title.as_str()),
                after
                    .tasks
                    .iter()
                    .find(|task| task.id == edge.to_task_id)
                    .map_or(edge.to_task_id.as_str(), |task| task.title.as_str())
            ));
        }
    }
    for edge in &before.dependencies {
        if !after.dependencies.contains(edge) {
            changes.push(format!(
                "Removed {} edge: {} → {}",
                edge.kind.as_str(),
                before
                    .tasks
                    .iter()
                    .find(|task| task.id == edge.from_task_id)
                    .map_or(edge.from_task_id.as_str(), |task| task.title.as_str()),
                before
                    .tasks
                    .iter()
                    .find(|task| task.id == edge.to_task_id)
                    .map_or(edge.to_task_id.as_str(), |task| task.title.as_str())
            ));
        }
    }
    if changes.is_empty() {
        changes.push("No plan content changes from the prior revision.".to_owned());
    }
    changes
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.modern_shell {
            self.modern_ui(ui);
            return;
        }
        let ctx = ui.ctx().clone();
        self.poll_run(&ctx);
        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        if ctx.input(|input| input.key_pressed(egui::Key::F1)) {
            self.show_portfolio = !self.show_portfolio;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::F2)) {
            self.show_terminal = !self.show_terminal;
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Project Pipeline {}", env!("CARGO_PKG_VERSION")));
                ui.separator();
                ui.checkbox(&mut self.show_portfolio, "Portfolio (F1)");
                ui.checkbox(&mut self.show_terminal, "Terminal (F2)");
                ui.separator();
                if ui.button("A−").clicked() {
                    self.text_scale = (self.text_scale - 0.1).max(0.8);
                }
                if ui.button("A+").clicked() {
                    self.text_scale = (self.text_scale + 0.1).min(2.0);
                }
                ui.checkbox(&mut self.dark_mode, "Dark");
            });
        });
        ctx.set_zoom_factor(self.text_scale);
        if self.show_portfolio {
            self.portfolio(ui);
        }
        for tab in &mut self.terminals {
            if let Err(error) = tab.session.poll(32 * 1024) {
                tab.error = Some(error.to_string());
            }
        }
        if !self.terminals.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if self.show_terminal {
            let height = ui.available_height();
            let panel = egui::Panel::bottom("terminal")
                .resizable(true)
                .default_size(height * self.terminal_fraction)
                .min_size(120.0)
                .show(ui, |ui| self.terminal_area(ui));
            self.terminal_fraction =
                (panel.response.rect.height() / height.max(1.0)).clamp(0.2, 0.75);
        }
        egui::CentralPanel::default().show(ui, |ui| self.management(ui));
    }
}

fn headless_run(args: &[String]) -> Result<(), String> {
    headless_run_with_harness(args, false)
}

fn headless_pi_run(args: &[String]) -> Result<(), String> {
    headless_run_with_harness(args, true)
}

fn headless_run_with_harness(args: &[String], pi: bool) -> Result<(), String> {
    if args.len() != 9 {
        return Err("usage: pipeline-app --headless-run <database> <evidence_dir> <project_id> <task_id> <opencode_exe> <pipeline_cli_exe> <provider> <model> <token_budget>".into());
    }
    let launch = RunLaunch {
        database: PathBuf::from(&args[0]),
        evidence_dir: PathBuf::from(&args[1]),
        project_id: args[2].clone(),
        task_id: args[3].clone(),
        opencode: PathBuf::from(&args[4]),
        pi,
        cli: PathBuf::from(&args[5]),
        provider: args[6].clone(),
        model: args[7].clone(),
        token_budget: args[8].parse().map_err(|_| "invalid token budget")?,
    };
    let control = run_worker::launch(launch);
    while let Ok(update) = control.updates.recv() {
        match update {
            RunUpdate::Started(id) => println!("run started: {id}"),
            RunUpdate::Event(message) => println!("event: {message}"),
            RunUpdate::Permission(id, summary) => println!("permission pending: {id}: {summary}"),
            RunUpdate::PermissionResolved(id) => println!("permission resolved: {id}"),
            RunUpdate::Finished(message) => {
                println!("finished: {message}");
                return Ok(());
            }
            RunUpdate::Failed(error) => return Err(error),
        }
    }
    Err("run worker ended without a result".into())
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(feature = "screenshot-evidence")]
    if args.get(1).is_some_and(|arg| arg == "--seed-ux-smoke") {
        if args.len() != 4 {
            eprintln!("usage: --seed-ux-smoke <database> <project-folder>");
            std::process::exit(2);
        }
        let mut engine = ProjectEngine::open(&args[2]).expect("open smoke database");
        engine
            .create_new(&args[3], "UX smoke sample")
            .expect("create sample through domain service");
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "--headless-run") {
        if let Err(error) = headless_run(&args[2..]) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.get(1).is_some_and(|arg| arg == "--headless-pi-run") {
        if let Err(error) = headless_pi_run(&args[2..]) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let size = [
        std::env::var("PIPELINE_UX_WIDTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1100.0),
        std::env::var("PIPELINE_UX_HEIGHT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(650.0),
    ];
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size),
        ..Default::default()
    };
    eframe::run_native(
        "Project Pipeline",
        options,
        Box::new(|_cc| Ok(Box::<DesktopApp>::default())),
    )
}

#[cfg(test)]
mod assistant_draft_tests {
    use super::*;

    fn sample_preview(id: &str) -> ProposalPreview {
        let raw = serde_json::json!({
            "version": 1, "id": id, "project_id": "alpha", "expected_revision": 1,
            "expected_approved_brief_revision": 0, "expected_scope_revision": 0,
            "original_evidence_ids": ["brief-1"], "reason": "Refine", "impact": "Draft",
            "affected_approved_scope": "None", "prerequisites": "Owner", "risk": "Low",
            "required_owner_approvals": "Apply", "recovery": "Restore prior draft",
            "change": {"type": "brief_field", "target_id": "brief-1", "field": "idea", "before": "Old", "after": "New"}
        }).to_string();
        ProposalPreview {
            proposal: Proposal::parse_json(&raw).unwrap(),
            status: ProposalStatus::Validated,
            explanation: "Draft only".into(),
            authoritative_record_id: "brief-1".into(),
        }
    }

    #[test]
    fn editing_after_preview_revokes_preview_and_confirmation() {
        let mut draft = ProposalDraft {
            after: "New".into(),
            ..Default::default()
        };
        draft.load_preview(sample_preview("first"), "brief-1", "Validated");
        draft.owner_confirmed = true;
        draft.after = "Different".into();
        draft.invalidate_if_input_changed("brief-1");
        assert!(draft.preview.is_none());
        assert!(!draft.owner_confirmed);
    }

    #[test]
    fn switching_target_or_project_revokes_confirmation() {
        let mut draft = ProposalDraft {
            after: "New".into(),
            ..Default::default()
        };
        draft.load_preview(sample_preview("first"), "brief-1", "Validated");
        draft.owner_confirmed = true;
        draft.kind = 1;
        draft.invalidate_if_input_changed("task-1");
        assert!(draft.preview.is_none());
        assert!(!draft.owner_confirmed);

        let mut app = DesktopApp::default();
        let mut draft = ProposalDraft::default();
        draft.load_preview(sample_preview("second"), "brief-1", "Validated");
        draft.owner_confirmed = true;
        app.proposal_drafts.insert("alpha".into(), draft);
        app.select_project("beta".into());
        assert!(!app.proposal_drafts["alpha"].owner_confirmed);
    }

    #[test]
    fn discard_and_new_assistant_preview_require_fresh_confirmation() {
        let mut draft = ProposalDraft::default();
        draft.load_preview(sample_preview("first"), "brief-1", "Validated");
        draft.owner_confirmed = true;
        draft.invalidate("Proposal discarded without changes.");
        assert!(draft.preview.is_none());
        assert!(!draft.owner_confirmed);
        draft.load_preview(
            sample_preview("assistant-second"),
            "brief-1",
            "Assistant suggestion validated",
        );
        assert!(!draft.owner_confirmed);
        assert_eq!(
            draft.preview.as_ref().unwrap().proposal.id,
            "assistant-second"
        );
    }

    #[test]
    fn drafts_are_isolated_by_project_and_portfolio() {
        let mut drafts = AssistantDrafts::default();
        *drafts.active_mut(None) = "portfolio question".into();
        *drafts.active_mut(Some("alpha")) = "alpha question".into();
        assert_eq!(drafts.active(Some("beta")), "");
        *drafts.active_mut(Some("beta")) = "beta question".into();
        assert_eq!(drafts.active(Some("alpha")), "alpha question");
        assert_eq!(drafts.active(Some("beta")), "beta question");
        assert_eq!(drafts.active(None), "portfolio question");
    }

    #[test]
    fn proposal_review_is_project_scoped_and_never_preconfirmed() {
        let mut app = DesktopApp::default();
        app.proposal_drafts.entry("alpha".into()).or_default().after = "Only alpha".into();
        assert_eq!(
            app.proposal_drafts.get("beta").map(|s| s.after.as_str()),
            None
        );
        assert!(!app.proposal_drafts.get("alpha").unwrap().owner_confirmed);
        app.proposal_drafts.remove("alpha");
        assert!(app.proposal_drafts.is_empty());
    }

    #[test]
    fn active_context_survives_panel_toggle_and_switching() {
        let mut app = DesktopApp::default();
        *app.assistant_drafts.active_mut(None) = "portfolio".into();
        app.selected_project = Some("alpha".into());
        app.studio_home = false;
        let context = app.assistant_context().map(str::to_owned);
        *app.assistant_drafts.active_mut(context.as_deref()) =
            "shared compact and expanded draft".into();
        app.assistant_open = true;
        app.assistant_open = false;
        app.selected_project = Some("beta".into());
        assert_eq!(app.assistant_drafts.active(app.assistant_context()), "");
        app.selected_project = Some("alpha".into());
        assert_eq!(
            app.assistant_drafts.active(app.assistant_context()),
            "shared compact and expanded draft"
        );
        app.studio_home = true;
        assert_eq!(
            app.assistant_drafts.active(app.assistant_context()),
            "portfolio"
        );
    }

    #[test]
    fn composer_shortcut_uses_modifiers_on_the_key_event() {
        let key = |modifiers| egui::Event::Key {
            key: egui::Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        assert!(composer_shortcut(&[key(egui::Modifiers::CTRL)]));
        assert!(composer_shortcut(&[key(egui::Modifiers::COMMAND)]));
        assert!(!composer_shortcut(&[key(egui::Modifiers::NONE)]));
    }

    #[test]
    fn offline_turns_and_inflight_channels_stay_under_origin_project() {
        let temp = tempfile::tempdir().unwrap();
        let mut engine = ProjectEngine::open(temp.path().join("assistant.sqlite")).unwrap();
        let alpha = engine
            .create_new(temp.path().join("alpha"), "Alpha")
            .unwrap();
        let beta = engine.create_new(temp.path().join("beta"), "Beta").unwrap();
        let mut app = DesktopApp::default();
        app.engine = Some(engine);
        app.selected_project = Some(alpha.id.clone());
        app.studio_home = false;
        *app.assistant_drafts.active_mut(Some(&alpha.id)) = "What next?".into();
        app.send_assistant();
        assert_eq!(app.assistant_sessions[&alpha.id].turns.len(), 1);
        assert!(
            app.assistant_sessions[&alpha.id].turns[0]
                .sources
                .contains(&alpha.id)
        );
        app.selected_project = Some(beta.id.clone());
        assert!(!app.assistant_sessions.contains_key(&beta.id));
        *app.assistant_drafts.active_mut(Some(&beta.id)) = "Status?".into();
        app.send_assistant();
        assert!(
            !app.assistant_sessions[&beta.id].turns[0]
                .answer
                .contains(&alpha.id)
        );
        assert_eq!(
            app.assistant_sessions[&alpha.id].turns[0].question,
            "What next?"
        );
    }

    #[test]
    fn project_switch_and_retry_discard_late_worker_events() {
        let temp = tempfile::tempdir().unwrap();
        let mut engine = ProjectEngine::open(temp.path().join("assistant.sqlite")).unwrap();
        let alpha = engine
            .create_new(temp.path().join("alpha"), "Alpha")
            .unwrap();
        let beta = engine.create_new(temp.path().join("beta"), "Beta").unwrap();
        let mut app = DesktopApp::default();
        app.engine = Some(engine);
        app.selected_project = Some(alpha.id.clone());
        app.studio_home = false;
        let (old_tx, old_rx) = mpsc::sync_channel(4);
        let cancelled = Arc::new(AtomicBool::new(false));
        let session = app.assistant_sessions.entry(alpha.id.clone()).or_default();
        session.receiver = Some(old_rx);
        session.cancel = Some(cancelled.clone());
        session.turns.push(ChatTurn {
            question: "old".into(),
            answer: String::new(),
            status: "Loading".into(),
            sources: vec![],
            usage: String::new(),
            requested_identity: String::new(),
            reported_model: None,
        });
        app.selected_project = Some(beta.id.clone());
        app.poll_assistant(&egui::Context::default());
        assert!(cancelled.load(Ordering::Acquire));
        assert!(old_tx.send(ChatEvent::Delta("late".into())).is_err());
        assert_eq!(
            app.assistant_sessions[&alpha.id].turns[0].status,
            "Cancelled on project switch"
        );
        assert!(!app.assistant_sessions.contains_key(&beta.id));
        app.selected_project = Some(alpha.id.clone());
        *app.assistant_drafts.active_mut(Some(&alpha.id)) = "retry".into();
        app.send_assistant();
        assert_eq!(app.assistant_sessions[&alpha.id].turns.len(), 2);
        assert_eq!(app.assistant_sessions[&alpha.id].turns[1].question, "retry");
        assert!(
            !app.assistant_sessions[&alpha.id].turns[1]
                .answer
                .contains("late")
        );
    }
}
