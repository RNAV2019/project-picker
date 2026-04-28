use std::collections::HashMap;
use egui::TextureHandle;
use crate::icons::IconResolver;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode { Search, Add }

/// Actions produced during a frame that require daemon-level responses.
#[derive(Debug)]
pub enum AppAction {
    Hide,
    OpenTerminal(String),
    RemoveProject(String),
}

pub struct App {
    pub visible: bool,
    pub mode: Mode,
    pub query: String,
    pub selected_idx: Option<usize>,
    pub recents: Vec<String>,
    pub icon_resolver: IconResolver,
    pub icon_textures: HashMap<String, TextureHandle>,
    pub pending_actions: Vec<AppAction>,
    pub focus_search: bool,
}

impl App {
    pub fn new() -> Self {
        let recents = crate::projects::load_recents();
        App {
            visible: false,
            mode: Mode::Search,
            query: String::new(),
            selected_idx: None,
            recents,
            icon_resolver: IconResolver::new(),
            icon_textures: HashMap::new(),
            pending_actions: Vec::new(),
            focus_search: false,
        }
    }

    pub fn on_show(&mut self) {
        self.query.clear();
        self.selected_idx = None;
        self.mode = Mode::Search;
        self.visible = true;
        self.focus_search = true;
    }

    pub fn on_hide(&mut self) {
        self.visible = false;
    }

    pub fn drain_actions(&mut self) -> Vec<AppAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Poll icon resolver for newly loaded textures. Call once per frame.
    pub fn poll_icons(&mut self, ctx: &egui::Context) {
        while let Some(result) = self.icon_resolver.poll() {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.rgba,
            );
            let handle = ctx.load_texture(
                &result.project_path,
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.icon_textures.insert(result.project_path, handle);
        }
    }

    pub fn filtered_recents(&self) -> Vec<&str> {
        self.recents.iter()
            .filter(|p| crate::projects::fuzzy_match(&self.query, p))
            .map(String::as_str)
            .collect()
    }

    pub fn suggestions(&self) -> Vec<String> {
        crate::paths::get_suggestions(&self.query)
    }

    pub fn open_project(&mut self, path: &str) {
        let path = path.to_string();
        self.recents.retain(|p| p != &path);
        self.recents.insert(0, path.clone());
        crate::projects::save_recents(&self.recents);
        self.pending_actions.push(AppAction::OpenTerminal(path));
        self.pending_actions.push(AppAction::Hide);
    }

    pub fn remove_project(&mut self, path: &str) {
        self.recents.retain(|p| p != path);
        crate::projects::save_recents(&self.recents);
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.poll_icons(ctx);

        for path in &self.recents {
            if !self.icon_textures.contains_key(path) {
                self.icon_resolver.request(path);
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(crate::ui::theme::BG))
            .show(ctx, |ui| {
                ui.style_mut().spacing.item_spacing = egui::Vec2::new(0.0, 0.0);
                ui.style_mut().visuals.selection.bg_fill = crate::ui::theme::ACCENT;

                let placeholder = match self.mode {
                    Mode::Search => "Search projects...",
                    Mode::Add    => "Type directory path...",
                };
                let grab = self.focus_search;
                self.focus_search = false;
                crate::ui::search::search_bar(ui, &mut self.query, placeholder, grab);

                ui.add(egui::Separator::default().horizontal().spacing(0.0));

                egui::ScrollArea::vertical()
                    .max_height(crate::ui::theme::WINDOW_MAX_H - 80.0)
                    .show(ui, |ui| {
                        self.render_list(ui);
                    });

                crate::ui::hints::hints_bar(ui);
            });

        self.handle_keyboard(ctx);
    }

    fn render_list(&mut self, ui: &mut egui::Ui) {
        match self.mode {
            Mode::Search => self.render_search_list(ui),
            Mode::Add    => self.render_add_list(ui),
        }
    }

    fn render_search_list(&mut self, ui: &mut egui::Ui) {
        let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();

        if self.query.is_empty() {
            crate::ui::list::section_header(ui, "Actions");
            let add_selected = self.selected_idx == Some(0);
            if crate::ui::list::action_row(ui, "⊕", "Add project", add_selected) {
                self.enter_add_mode();
            }
        }

        if !filtered.is_empty() {
            let offset = if self.query.is_empty() { 1 } else { 0 };
            crate::ui::list::section_header(ui, "Recent Projects");
            for (i, path) in filtered.iter().enumerate() {
                let selected = self.selected_idx == Some(i + offset);
                let icon = self.icon_textures.get(path.as_str());
                if crate::ui::list::project_row(ui, path, icon, selected) {
                    self.open_project(path);
                }
            }
        }
    }

    fn render_add_list(&mut self, ui: &mut egui::Ui) {
        let suggestions = self.suggestions();
        for (i, path) in suggestions.iter().enumerate() {
            let selected = self.selected_idx == Some(i);
            if crate::ui::list::suggestion_row(ui, path, selected) {
                self.add_and_open(path);
            }
        }
    }

    fn enter_add_mode(&mut self) {
        self.mode = Mode::Add;
        self.query.clear();
        self.selected_idx = None;
    }

    fn add_and_open(&mut self, path: &str) {
        let abs = crate::paths::expand_tilde(path);
        if std::path::Path::new(&abs).is_dir() {
            self.open_project(path);
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let selectable_count = self.selectable_count();

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        match key {
                            egui::Key::Escape => {
                                if self.mode == Mode::Add {
                                    self.mode = Mode::Search;
                                    self.query.clear();
                                    self.selected_idx = None;
                                } else {
                                    self.pending_actions.push(AppAction::Hide);
                                }
                            }
                            egui::Key::Enter => {
                                self.activate_selected();
                            }
                            egui::Key::ArrowDown | egui::Key::Tab => {
                                if modifiers.shift && *key == egui::Key::Tab {
                                    self.move_selection(-1, selectable_count);
                                } else {
                                    self.move_selection(1, selectable_count);
                                }
                            }
                            egui::Key::ArrowUp => {
                                self.move_selection(-1, selectable_count);
                            }
                            egui::Key::Backspace if modifiers.alt => {
                                if let Some(idx) = self.selected_idx {
                                    if self.mode == Mode::Search {
                                        let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                        let offset = if self.query.is_empty() { 1 } else { 0 };
                                        if idx >= offset {
                                            if let Some(path) = filtered.get(idx - offset) {
                                                let path = path.clone();
                                                self.remove_project(&path);
                                                self.selected_idx = None;
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn selectable_count(&self) -> usize {
        match self.mode {
            Mode::Search => {
                let n = self.filtered_recents().len();
                if self.query.is_empty() { n + 1 } else { n }
            }
            Mode::Add => self.suggestions().len(),
        }
    }

    fn move_selection(&mut self, delta: i32, count: usize) {
        if count == 0 { return; }
        let current = self.selected_idx.unwrap_or(usize::MAX) as i32;
        let next = current + delta;
        if next < 0 {
            self.selected_idx = None;
        } else {
            self.selected_idx = Some((next as usize).min(count - 1));
        }
    }

    fn activate_selected(&mut self) {
        let selectable = self.selectable_count();
        let idx = match self.selected_idx {
            Some(i) => i,
            None => {
                if selectable > 0 { 0 } else { return; }
            }
        };

        match self.mode {
            Mode::Search => {
                let offset = if self.query.is_empty() { 1 } else { 0 };
                if idx == 0 && self.query.is_empty() {
                    self.enter_add_mode();
                } else {
                    let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    if let Some(path) = filtered.get(idx.saturating_sub(offset)) {
                        let path = path.clone();
                        self.open_project(&path);
                    }
                }
            }
            Mode::Add => {
                let suggestions = self.suggestions();
                if let Some(path) = suggestions.get(idx) {
                    let path = path.clone();
                    self.add_and_open(&path);
                }
            }
        }
    }
}
