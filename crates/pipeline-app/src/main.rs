use eframe::egui;
use pipeline_engine::{
    BriefContent, CriterionSpec, DependencyKind, DependencySpec, DiscoveryView, EpicSpec,
    PlanContent, PlanView, ProjectEngine, ProjectOverview, ResearchInput, TaskSpec,
};
use pipeline_terminal::TerminalSession;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Brief,
    Research,
    Plan,
    Decisions,
    Runs,
    About,
}

enum PlanAction {
    Save,
    Approve,
    Submit(String, i64),
    Verify(String, String, i64, BTreeMap<String, String>),
    Accept(String, i64),
}

struct DesktopApp {
    data_dir: Option<PathBuf>,
    engine: Option<ProjectEngine>,
    projects: Vec<ProjectOverview>,
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
            data_dir: data_dir.clone(),
            engine: None,
            projects: Vec::new(),
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
        let database = data_dir.map(|dir| dir.join("portfolio.sqlite"));
        match database {
            Some(database) => {
                let result = database
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        ProjectEngine::open(&database).map_err(|error| error.to_string())
                    });
                match result {
                    Ok(engine) => {
                        match engine.list_overviews() {
                            Ok(projects) => app.projects = projects,
                            Err(error) => {
                                app.message = Some(format!("Unable to load projects: {error}"))
                            }
                        }
                        app.selected_project =
                            app.projects.first().map(|project| project.id.clone());
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
                        }
                        app.engine = Some(engine);
                    }
                    Err(error) => app.message = Some(format!("Unable to open portfolio: {error}")),
                }
            }
            None => app.message = Some("No local application data directory found".to_owned()),
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
        self.selected_project = Some(id.clone());
        self.active_terminal = self
            .terminals
            .iter()
            .find(|tab| tab.project_id == id)
            .map(|tab| tab.id);
        self.terminal_message = None;
        self.refresh_discovery();
        self.refresh_plan();
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
            }
            ui.separator();
            ui.heading("Dependencies");
            ui.label("A blocking edge means the first task waits for the prerequisite.");
            let task_choices: Vec<_> = self
                .plan_draft
                .tasks
                .iter()
                .map(|task| (task.id.clone(), task.title.clone()))
                .collect();
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
                ui.heading("Active scope tasks");
                for task in &view.active_tasks {
                    ui.group(|ui| {
                        ui.strong(format!(
                            "{} · {} · weight {}",
                            task.title, task.status, task.weight
                        ));
                        ui.label(&task.outcome);
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
                    });
                }
            }
        });
        if let Some(action) = action {
            self.run_plan_action(action);
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
                    ui.label(format!("Next owner action: {}", project.next_owner_action));
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
                ui.heading("Decisions");
                ui.label("The decision inbox is not connected yet.");
            }
            Tab::Runs => {
                ui.heading("Agent runs");
                ui.label("Harness integration is not connected yet.");
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
        let ctx = ui.ctx().clone();
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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Project Pipeline",
        options,
        Box::new(|_cc| Ok(Box::<DesktopApp>::default())),
    )
}
