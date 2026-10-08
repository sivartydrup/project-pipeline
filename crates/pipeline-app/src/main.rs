use eframe::egui;

const PROJECT_COUNT: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Plan,
    Decisions,
    Runs,
    About,
}

struct DesktopSpike {
    selected_project: usize,
    active_tab: Tab,
    search: String,
    show_portfolio: bool,
    show_terminal: bool,
    terminal_fraction: f32,
    text_scale: f32,
    dark_mode: bool,
}

impl Default for DesktopSpike {
    fn default() -> Self {
        Self {
            selected_project: 0,
            active_tab: Tab::Overview,
            search: String::new(),
            show_portfolio: true,
            show_terminal: true,
            terminal_fraction: 0.35,
            text_scale: 1.0,
            dark_mode: true,
        }
    }
}

impl DesktopSpike {
    fn project_name(index: usize) -> String {
        if index == 0 {
            "Project Pipeline".to_owned()
        } else {
            format!("Project {:05}", index + 1)
        }
    }

    fn portfolio(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("portfolio")
            .resizable(true)
            .default_size(260.0)
            .min_size(180.0)
            .show(ui, |ui| {
                ui.heading("Projects");
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.search);
                });
                ui.separator();

                if self.search.is_empty() {
                    egui::ScrollArea::vertical().show_rows(ui, 53.0, PROJECT_COUNT, |ui, range| {
                        for index in range {
                            self.project_row(ui, index);
                        }
                    });
                } else {
                    let needle = self.search.to_lowercase();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for index in 0..PROJECT_COUNT {
                            if Self::project_name(index).to_lowercase().contains(&needle) {
                                self.project_row(ui, index);
                            }
                        }
                    });
                }
            });
    }

    fn project_row(&mut self, ui: &mut egui::Ui, index: usize) {
        let name = Self::project_name(index);
        let stage = if index == 0 { "Planning" } else { "Build" };
        let completion = if index == 0 { 0 } else { (index * 7) % 100 };
        let label = format!("{name}\n{stage} · {completion}% verified");
        if ui
            .add_sized(
                [ui.available_width(), 46.0],
                egui::Button::new(label).selected(self.selected_project == index),
            )
            .clicked()
        {
            self.selected_project = index;
        }
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
        ui.heading(Self::project_name(self.selected_project));
        match self.active_tab {
            Tab::Overview => {
                ui.label("Stage: Planning");
                ui.label("Health: Needs input");
                ui.label("Verified completion: 0%");
                ui.separator();
                ui.label("Next owner action: approve the first executable task plan.");
            }
            Tab::Plan => {
                ui.heading("Milestones");
                ui.label("Discovery → Design → Build → Verify → Release → Operate");
            }
            Tab::Decisions => {
                ui.heading("Decision inbox");
                ui.label("Each decision will show alternatives, rationale, evidence, and approval status.");
            }
            Tab::Runs => {
                ui.heading("Agent runs");
                ui.label("OpenCode and Pi sessions will appear here with live progress.");
            }
            Tab::About => {
                ui.heading("Project Pipeline");
                ui.label(format!(
                    "Version {} · foundation build",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.label("This build uses sample project data while persistence is implemented.");
                ui.label("Windows and Apple Silicon macOS are the primary targets.");
            }
        }
    }
}

impl eframe::App for DesktopSpike {
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
                ui.strong(format!(
                    "Project Pipeline {} · prototype data",
                    env!("CARGO_PKG_VERSION")
                ));
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
                ui.label("PTY integration is the next validation spike.");
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
        Box::new(|_cc| Ok(Box::<DesktopSpike>::default())),
    )
}
