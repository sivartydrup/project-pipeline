use eframe::egui::{self, Color32, RichText};

const ACCENT: Color32 = Color32::from_rgb(25, 119, 112);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Home,
    Overview,
    Roadmap,
    Work,
    Design,
    Releases,
    History,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkState {
    Ready,
    Working,
    PauseRequested,
    Paused,
    Blocked,
    AwaitingReview,
    Offline,
}

impl WorkState {
    const ALL: [Self; 7] = [
        Self::Ready,
        Self::Working,
        Self::PauseRequested,
        Self::Paused,
        Self::Blocked,
        Self::AwaitingReview,
        Self::Offline,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Working => "Working",
            Self::PauseRequested => "Pause requested",
            Self::Paused => "Paused",
            Self::Blocked => "Blocked",
            Self::AwaitingReview => "Awaiting review",
            Self::Offline => "Offline",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Self::Ready => "Approved work is eligible after a fresh preflight. Nothing is running.",
            Self::Working => "An approved task is running within its illustrative limits.",
            Self::PauseRequested => "Current work continues; no new task will start afterward.",
            Self::Paused => {
                "Automatic continuation is stopped. Saved work and review remain available."
            }
            Self::Blocked => "An unresolved decision prevents this task from starting.",
            Self::AwaitingReview => {
                "Work was submitted. Only the owner can accept it after checking evidence."
            }
            Self::Offline => {
                "The configured provider is unavailable. Saved plans and reviews remain available."
            }
        }
    }
}

#[derive(Default)]
struct Conversation {
    draft: String,
    turns: Vec<String>,
}

struct Studio {
    project: Option<usize>,
    screen: Screen,
    state: [WorkState; 2],
    conversations: [Conversation; 3],
    assistant_open: bool,
    advanced_open: bool,
    dark: bool,
    scale: f32,
    focus_composer: bool,
    focus_return: bool,
    show_proposal: [bool; 2],
    capture_path: Option<String>,
    capture_frames: u8,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            project: None,
            screen: Screen::Home,
            state: [WorkState::AwaitingReview, WorkState::Ready],
            conversations: std::array::from_fn(|_| Conversation::default()),
            assistant_open: false,
            advanced_open: false,
            dark: false,
            scale: 1.0,
            focus_composer: false,
            focus_return: false,
            show_proposal: [false; 2],
            capture_path: None,
            capture_frames: 0,
        }
    }
}

impl Studio {
    fn context_index(&self) -> usize {
        self.project.map_or(0, |index| index + 1)
    }

    fn context_name(&self) -> &'static str {
        match self.project {
            None => "All projects",
            Some(0) => "Northstar",
            Some(_) => "Fieldnotes",
        }
    }

    fn choose_project(&mut self, project: usize) {
        self.project = Some(project);
        self.screen = Screen::Overview;
    }

    fn send(&mut self) {
        let index = self.context_index();
        let draft = self.conversations[index].draft.trim().to_owned();
        if draft.is_empty() {
            return;
        }
        self.conversations[index].draft.clear();
        self.conversations[index]
            .turns
            .push(format!("You: {draft}"));
        let response = if self.project.is_none() && draft.to_lowercase().contains("change") {
            "Illustrative guidance: choose a project before preparing a change. No data was changed."
        } else if draft.to_lowercase().contains("subscription") {
            if let Some(project) = self.project {
                self.show_proposal[project] = true;
            }
            "Illustrative proposal prepared below. It is a draft and requires review. No data was changed."
        } else {
            "Illustrative guidance based on this screen. No provider was contacted and no data was changed."
        };
        self.conversations[index].turns.push(response.to_owned());
        self.assistant_open = true;
    }

    fn close_assistant(&mut self) {
        if self.assistant_open {
            self.assistant_open = false;
            self.focus_return = true;
        }
    }

    fn toggle_assistant(&mut self) {
        if self.assistant_open {
            self.close_assistant();
        } else {
            self.assistant_open = true;
            self.focus_composer = true;
        }
    }

    fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
        let width = ui.available_width().min(900.0);
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(16))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.set_width((width - 32.0).max(120.0));
                content(ui);
            });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("studio_top").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(RichText::new("Project Pipeline").size(18.0).color(ACCENT));
                ui.separator();
                if ui.button("Home").clicked() {
                    self.project = None;
                    self.screen = Screen::Home;
                }
                if ui.button("Projects").clicked() {
                    self.project = None;
                    self.screen = Screen::Home;
                }
                if ui.button("Inbox · 1").clicked() {
                    self.choose_project(0);
                    self.screen = Screen::Review;
                }
                ui.separator();
                ui.label(self.context_name());
                if ui
                    .button(if self.dark { "Light" } else { "Dark" })
                    .clicked()
                {
                    self.dark = !self.dark;
                }
                let toggle = ui.button(if self.assistant_open {
                    "Close assistant"
                } else {
                    "Open assistant"
                });
                if self.focus_return {
                    toggle.request_focus();
                    self.focus_return = false;
                }
                if toggle.clicked() {
                    self.toggle_assistant();
                }
            });
        });
    }

    fn bottom_composer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("studio_composer")
            .min_size(72.0)
            .show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Ask about this work");
                    ui.label(format!("Context: {}", self.context_name()));
                    ui.weak("Illustrative · no provider connected");
                });
                ui.horizontal(|ui| {
                    let index = self.context_index();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.conversations[index].draft)
                            .id(egui::Id::new("compact_composer"))
                            .hint_text("Ask a question or describe a change…")
                            .desired_width(ui.available_width() - 72.0),
                    );
                    if self.focus_composer {
                        response.request_focus();
                        self.focus_composer = false;
                    }
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if ui.button("Send").clicked() || enter {
                        self.send();
                    }
                });
            });
    }

    fn assistant(&mut self, ui: &mut egui::Ui) {
        ui.heading("Assistant");
        ui.label(format!("{} · {:?}", self.context_name(), self.screen));
        ui.weak("Sample conversation. No provider or project data is connected.");
        ui.separator();
        let index = self.context_index();
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                if self.conversations[index].turns.is_empty() {
                    ui.label(
                        "Ask what changed, why this action is next, or propose a plan change.",
                    );
                }
                for turn in &self.conversations[index].turns {
                    Self::card(ui, |ui| {
                        ui.label(turn);
                    });
                    ui.add_space(6.0);
                }
            });
        if let Some(project) = self.project
            && self.show_proposal[project]
        {
            ui.separator();
            Self::card(ui, |ui| {
                ui.strong("Draft proposal · Add subscriptions");
                ui.label(format!(
                    "Bound to {} plan revision {}",
                    self.context_name(),
                    if project == 0 { 3 } else { 2 }
                ));
                ui.label("Before: no subscription milestone");
                ui.label("After: draft subscription discovery and build work");
                ui.label("Impact: scope expansion · owner approval required");
                ui.weak("Review is illustrative; this spike cannot apply the proposal.");
                if ui.button("Review in Roadmap").clicked() {
                    self.screen = Screen::Roadmap;
                }
            });
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Your projects");
        if ui.button("Northstar · Build · 40% verified").clicked() {
            self.choose_project(0);
        }
        if ui.button("Fieldnotes · Planning · 0% verified").clicked() {
            self.choose_project(1);
        }
        ui.separator();
        ui.label("Selected project");
        ui.strong(self.context_name());
        ui.add_space(12.0);
        ui.weak("Illustrative data only");
    }

    fn project_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (screen, label) in [
                (Screen::Overview, "Overview"),
                (Screen::Roadmap, "Roadmap"),
                (Screen::Work, "Work"),
                (Screen::Design, "Design"),
                (Screen::Releases, "Releases"),
                (Screen::History, "History"),
            ] {
                if ui.selectable_label(self.screen == screen, label).clicked() {
                    self.screen = screen;
                }
            }
        });
        ui.separator();
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Good morning. Here's what needs you.").size(27.0));
        ui.label("Two illustrative projects · one work item awaiting your review");
        ui.add_space(14.0);
        Self::card(ui, |ui| {
            ui.strong("YOUR FOCUS · NORTHSTAR");
            ui.heading("Review the navigation update");
            ui.label("The sample agent submitted a change and checks passed. Your acceptance is still required before verified progress changes.");
            if ui.button("Review work").clicked() {
                self.choose_project(0);
                self.screen = Screen::Review;
            }
        });
        ui.add_space(14.0);
        if ui.available_width() >= 650.0 {
            ui.columns(2, |columns| {
                self.project_card(&mut columns[0], 0);
                self.project_card(&mut columns[1], 1);
            });
        } else {
            self.project_card(ui, 0);
            ui.add_space(8.0);
            self.project_card(ui, 1);
        }
    }

    fn project_card(&mut self, ui: &mut egui::Ui, project: usize) {
        Self::card(ui, |ui| {
            ui.heading(if project == 0 {
                "Northstar"
            } else {
                "Fieldnotes"
            });
            ui.label(if project == 0 {
                "Build · 40% verified"
            } else {
                "Planning · 0% verified"
            });
            ui.label(if project == 0 {
                "Next: review navigation update"
            } else {
                "Next: approve research direction"
            });
            if ui
                .button(if project == 0 {
                    "Open Northstar"
                } else {
                    "Open Fieldnotes"
                })
                .clicked()
            {
                self.choose_project(project);
            }
        });
    }

    fn state_controls(&mut self, ui: &mut egui::Ui, project: usize) {
        egui::ComboBox::from_label("Preview sample state")
            .selected_text(self.state[project].label())
            .show_ui(ui, |ui| {
                for state in WorkState::ALL {
                    ui.selectable_value(&mut self.state[project], state, state.label());
                }
            });
        ui.strong(format!("Status: {}", self.state[project].label()));
        ui.label(self.state[project].explanation());
        match self.state[project] {
            WorkState::Working => {
                if ui.button("Pause after task").clicked() {
                    self.state[project] = WorkState::PauseRequested;
                }
                if ui.button("Pause now").clicked() {
                    self.state[project] = WorkState::Paused;
                }
            }
            WorkState::PauseRequested => {
                if ui.button("Cancel pause request").clicked() {
                    self.state[project] = WorkState::Working;
                }
                if ui.button("Finish current task").clicked() {
                    self.state[project] = WorkState::Paused;
                }
            }
            WorkState::Paused => {
                if ui
                    .button("Resume eligible work · fresh preflight")
                    .clicked()
                {
                    self.state[project] = WorkState::Ready;
                }
            }
            WorkState::AwaitingReview if ui.button("Open review").clicked() => {
                self.screen = Screen::Review;
            }
            _ => {}
        }
    }

    fn overview(&mut self, ui: &mut egui::Ui, project: usize) {
        let (goal, action, reason, stage, progress) = if project == 0 {
            (
                "A calmer way for teams to plan their next release",
                "Review the navigation update",
                "The submitted sample work has checks and a diff ready for owner review.",
                "Build",
                "40%",
            )
        } else {
            (
                "Turn field observations into shared product insight",
                "Approve research direction",
                "The proposed research scope needs an owner decision before work can start.",
                "Planning",
                "0%",
            )
        };
        ui.heading(RichText::new(self.context_name()).size(27.0));
        ui.label(goal);
        ui.add_space(12.0);
        Self::card(ui, |ui| {
            ui.strong("RECOMMENDED NEXT ACTION");
            ui.heading(action);
            ui.label(reason);
            ui.label(if project == 0 {
                "Prerequisite: inspect outcome and evidence"
            } else {
                "Prerequisite: review proposed brief revision"
            });
            ui.weak("Alternative: explore the Roadmap");
            if ui.button("Continue").clicked() {
                self.screen = if project == 0 {
                    Screen::Review
                } else {
                    Screen::Roadmap
                };
            }
        });
        ui.add_space(12.0);
        if ui.available_width() >= 1100.0 {
            ui.columns(2, |columns| {
                self.work_card(&mut columns[0], project);
                Self::progress_card(&mut columns[1], progress, stage);
            });
        } else {
            self.work_card(ui, project);
            ui.add_space(8.0);
            Self::progress_card(ui, progress, stage);
        }
    }

    fn work_card(&mut self, ui: &mut egui::Ui, project: usize) {
        Self::card(ui, |ui| {
            ui.strong("CURRENT WORK");
            self.state_controls(ui, project);
        });
    }

    fn progress_card(ui: &mut egui::Ui, progress: &str, stage: &str) {
        Self::card(ui, |ui| {
            ui.strong("VERIFIED PROGRESS");
            ui.heading(progress);
            ui.label(format!("{stage} stage · accepted work only"));
            ui.label("Idea / Plan / Build / Verify / Release");
        });
    }

    fn review(&mut self, ui: &mut egui::Ui) {
        ui.heading("Review submitted work");
        ui.label("Northstar · illustrative navigation update");
        Self::card(ui, |ui| {
            ui.strong("OUTCOME");
            ui.add(egui::Label::new("The sample navigation is easier to find and the menu remains usable by keyboard.").wrap());
            ui.add(egui::Label::new("Checks: 3 passed · 0 failed · sample evidence").wrap());
            ui.add(egui::Label::new("Risk: narrow layouts need further native validation.").wrap());
            ui.add(
                egui::Label::new(
                    "Status: awaiting owner review; verified completion has not changed.",
                )
                .wrap(),
            );
        });
        ui.add_space(12.0);
        Self::card(ui, |ui| {
            ui.strong("EVIDENCE SUMMARY");
            ui.label("Outcome check · sample screenshots attached");
            ui.label("Test result · sample keyboard check passed");
            ui.label("Diff · sample navigation files changed");
            ui.weak("This spike has no service connection. Accept and request-changes are intentionally unavailable.");
            ui.add_enabled(false, egui::Button::new("Accept work"));
            ui.add_enabled(false, egui::Button::new("Request changes"));
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui) {
        ui.heading("Advanced workspace");
        ui.label("In production this area retains terminal, raw diffs, exact evidence, run diagnostics and API/CLI access.");
        ui.weak("This isolated spike has no terminal, database or agent connection.");
    }

    fn main_content(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(1000.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            if self.project.is_none() {
                self.home(ui);
            } else {
                self.project_tabs(ui);
                let project = self.project.unwrap_or(0);
                match self.screen {
                    Screen::Overview => self.overview(ui, project),
                    Screen::Review => self.review(ui),
                    _ => {
                        ui.heading(format!("{} · {:?}", self.context_name(), self.screen));
                        ui.label("Representative navigation state for native shell feasibility.");
                        ui.label("Detailed workflow and service integration require separate UX implementation approval.");
                        if self.screen == Screen::Roadmap && self.show_proposal[project] {
                            ui.label("Draft subscription proposal waits for exact revision and owner approval.");
                        }
                    }
                }
            }
            ui.add_space(20.0);
            if ui.button(if self.advanced_open { "Hide Advanced" } else { "Open Advanced" }).clicked() {
                self.advanced_open = !self.advanced_open;
            }
            if self.advanced_open {
                self.advanced(ui);
            }
        });
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.override_text_color = Some(if self.dark {
            Color32::from_rgb(235, 237, 235)
        } else {
            Color32::from_rgb(42, 48, 47)
        });
        ctx.set_visuals(visuals);
        ctx.set_zoom_factor(self.scale);
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
            self.focus_composer = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.close_assistant();
        }
        self.top_bar(ui);
        self.bottom_composer(ui);
        let narrow = ui.available_width() < 1040.0;
        if !narrow && self.assistant_open {
            egui::Panel::right("assistant_side")
                .resizable(true)
                .default_size(330.0)
                .min_size(270.0)
                .max_size(390.0)
                .show(ui, |ui| self.assistant(ui));
        }
        if ui.available_width() >= 950.0 {
            egui::Panel::left("project_rail")
                .resizable(true)
                .default_size(215.0)
                .min_size(180.0)
                .max_size(260.0)
                .show(ui, |ui| self.sidebar(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if ui.available_width() < 560.0 {
                ui.weak("Compact layout · project switcher in top bar");
                ui.horizontal(|ui| {
                    if ui.button("Northstar").clicked() {
                        self.choose_project(0);
                    }
                    if ui.button("Fieldnotes").clicked() {
                        self.choose_project(1);
                    }
                });
            }
            self.main_content(ui);
        });
        if narrow && self.assistant_open {
            let mut open = self.assistant_open;
            egui::Window::new("Assistant drawer")
                .id(egui::Id::new("assistant_drawer"))
                .open(&mut open)
                .default_width(360.0)
                .max_width(440.0)
                .anchor(egui::Align2::RIGHT_TOP, [-12.0, 52.0])
                .show(&ctx, |ui| self.assistant(ui));
            if !open {
                self.close_assistant();
            }
        }
        if let Some(path) = self.capture_path.as_ref() {
            for event in ctx.input(|input| input.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    image::save_buffer(
                        path,
                        &pixels,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    )
                    .expect("save native screenshot");
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

fn parse_args() -> (Studio, [f32; 2]) {
    let mut app = Studio::default();
    let mut size = [1440.0, 900.0];
    let args: Vec<String> = std::env::args().collect();
    let mut index = 1;
    while index < args.len() {
        let value = args.get(index + 1).map(String::as_str);
        match args[index].as_str() {
            "--width" => size[0] = value.and_then(|v| v.parse().ok()).unwrap_or(size[0]),
            "--height" => size[1] = value.and_then(|v| v.parse().ok()).unwrap_or(size[1]),
            "--scale" => app.scale = value.and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--project" => {
                app.project = match value {
                    Some("northstar") => Some(0),
                    Some("fieldnotes") => Some(1),
                    _ => None,
                }
            }
            "--screen" => {
                app.screen = match value {
                    Some("review") => Screen::Review,
                    Some("roadmap") => Screen::Roadmap,
                    Some("work") => Screen::Work,
                    _ => Screen::Overview,
                }
            }
            "--state" => {
                if let Some(project) = app.project {
                    app.state[project] = match value {
                        Some("ready") => WorkState::Ready,
                        Some("working") => WorkState::Working,
                        Some("pause-requested") => WorkState::PauseRequested,
                        Some("paused") => WorkState::Paused,
                        Some("blocked") => WorkState::Blocked,
                        Some("awaiting-review") => WorkState::AwaitingReview,
                        Some("offline") => WorkState::Offline,
                        _ => app.state[project],
                    };
                }
            }
            "--capture" => app.capture_path = value.map(str::to_owned),
            "--dark" => {
                app.dark = true;
                index -= 1;
            }
            "--assistant" => {
                app.assistant_open = true;
                index -= 1;
            }
            _ => {
                index -= 1;
            }
        }
        index += 2;
    }
    (app, size)
}

fn main() -> eframe::Result {
    let (app, size) = parse_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size),
        ..Default::default()
    };
    eframe::run_native(
        "Modern Product Studio · native feasibility",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversations_and_drafts_remain_project_scoped() {
        let mut app = Studio::default();
        app.choose_project(0);
        app.conversations[1].draft = "northstar draft".into();
        app.choose_project(1);
        app.conversations[2].draft = "fieldnotes draft".into();
        app.send();
        app.choose_project(0);
        assert_eq!(app.conversations[1].draft, "northstar draft");
        assert!(app.conversations[1].turns.is_empty());
        assert_eq!(app.conversations[2].turns.len(), 2);
    }

    #[test]
    fn proposal_is_bound_to_its_project() {
        let mut app = Studio::default();
        app.choose_project(0);
        app.conversations[1].draft = "Add subscriptions".into();
        app.send();
        assert_eq!(app.show_proposal, [true, false]);
        app.choose_project(1);
        assert!(!app.show_proposal[1]);
    }

    #[test]
    fn close_requests_focus_return() {
        let mut app = Studio {
            assistant_open: true,
            ..Default::default()
        };
        app.close_assistant();
        assert!(!app.assistant_open);
        assert!(app.focus_return);
    }
}
