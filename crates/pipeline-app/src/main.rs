use eframe::egui;
use pipeline_engine::{ProjectEngine, ProjectOverview};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Plan,
    Decisions,
    Runs,
    About,
}

struct DesktopApp {
    engine: Option<ProjectEngine>,
    projects: Vec<ProjectOverview>,
    selected_project: Option<String>,
    active_tab: Tab,
    search: String,
    import_path: String,
    import_name: String,
    message: Option<String>,
    show_portfolio: bool,
    show_terminal: bool,
    terminal_fraction: f32,
    text_scale: f32,
    dark_mode: bool,
}

impl Default for DesktopApp {
    fn default() -> Self {
        let mut app = Self {
            engine: None,
            projects: Vec::new(),
            selected_project: None,
            active_tab: Tab::Overview,
            search: String::new(),
            import_path: String::new(),
            import_name: String::new(),
            message: None,
            show_portfolio: true,
            show_terminal: true,
            terminal_fraction: 0.35,
            text_scale: 1.0,
            dark_mode: true,
        };
        let database =
            directories::ProjectDirs::from("dev", "Project Pipeline", "Project Pipeline")
                .map(|dirs| dirs.data_local_dir().join("portfolio.sqlite"));
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
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn portfolio(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("portfolio")
            .resizable(true)
            .default_size(285.0)
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
                            self.selected_project = Some(project.id.clone());
                        }
                    }
                });
            });
    }

    fn management(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, name) in [
                (Tab::Overview, "Overview"),
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
                ui.label("Brief and task planning are coming in the next milestones.");
            }
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
                if self.show_terminal {
                    ui.add(
                        egui::Slider::new(&mut self.terminal_fraction, 0.2..=0.65).text("Split"),
                    );
                }
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
        ctx.set_pixels_per_point(self.text_scale);
        if self.show_portfolio {
            self.portfolio(ui);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.show_terminal {
                let upper_height = ui.available_height() * (1.0 - self.terminal_fraction);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), upper_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_height(upper_height);
                        self.management(ui);
                    },
                );
                ui.separator();
                ui.heading("Terminal area");
                ui.label("Terminal integration is planned for P07.");
            } else {
                self.management(ui);
            }
        });
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
