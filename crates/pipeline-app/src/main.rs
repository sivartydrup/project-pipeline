use eframe::egui;
use pipeline_engine::{BriefContent, DiscoveryView, ProjectEngine, ProjectOverview, ResearchInput};
use pipeline_terminal::TerminalSession;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

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
                ui.heading("Plan");
                ui.label("Task planning is coming in P09.");
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
