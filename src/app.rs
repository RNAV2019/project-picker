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
}

/// Animation duration in seconds.
const ANIM_DURATION: f32 = 0.08;

pub struct App {
    pub mode: Mode,
    pub query: String,
    pub selected_idx: Option<usize>,
    pub recents: Vec<String>,
    pub pinned: Vec<String>,
    pub icon_resolver: IconResolver,
    pub icon_textures: HashMap<String, TextureHandle>,
    pub pending_actions: Vec<AppAction>,
    pub focus_search: bool,
    /// 0.0 = fully hidden, 1.0 = fully visible. Animated on show/hide.
    pub anim_progress: f32,
    /// True while animating in, false while animating out.
    pub anim_showing: bool,
    /// Set to true when close animation completes — daemon should actually hide the surface.
    pub anim_hide_pending: bool,
}

impl App {
    pub fn new() -> Self {
        let recents = crate::projects::load_recents();
        let pinned = crate::projects::load_pinned();
        App {
            mode: Mode::Search,
            query: String::new(),
            selected_idx: None,
            recents,
            pinned,
            icon_resolver: IconResolver::new(),
            icon_textures: HashMap::new(),
            pending_actions: Vec::new(),
            focus_search: false,
            anim_progress: 0.0,
            anim_showing: false,
            anim_hide_pending: false,
        }
    }

    pub fn on_show(&mut self) {
        self.query.clear();
        self.selected_idx = None;
        self.mode = Mode::Search;
        self.focus_search = true;
        self.anim_showing = true;
        self.anim_progress = 0.0;
        self.anim_hide_pending = false;
    }

    /// Begin close animation. Actual hide happens when animation completes.
    pub fn begin_hide(&mut self) {
        self.anim_showing = false;
    }

    pub fn on_hide(&mut self) {
        self.anim_progress = 0.0;
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

    pub fn filtered_pinned(&self) -> Vec<&str> {
        self.pinned.iter()
            .filter(|p| crate::projects::fuzzy_match(&self.query, p))
            .map(String::as_str)
            .collect()
    }

    /// Recents matching the query, with pinned projects excluded (they appear above).
    pub fn filtered_recents_unpinned(&self) -> Vec<&str> {
        self.recents.iter()
            .filter(|p| {
                crate::projects::fuzzy_match(&self.query, p) && !self.pinned.contains(p)
            })
            .map(String::as_str)
            .collect()
    }

    pub fn toggle_pin(&mut self, path: &str) {
        if let Some(pos) = self.pinned.iter().position(|p| p == path) {
            self.pinned.remove(pos);
        } else {
            self.pinned.push(path.to_string());
        }
        crate::projects::save_pinned(&self.pinned);
    }

    pub fn suggestions(&self) -> Vec<String> {
        crate::paths::get_suggestions(&self.query)
    }

    pub fn open_project(&mut self, path: &str) {
        // Guard: if a terminal open is already queued (e.g. click + Enter in same frame), ignore.
        if self.pending_actions.iter().any(|a| matches!(a, AppAction::OpenTerminal(_))) {
            return;
        }
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

        for path in self.recents.iter().chain(self.pinned.iter()) {
            if !self.icon_textures.contains_key(path) {
                self.icon_resolver.request(path);
            }
        }

        // Drive animation
        let dt = ctx.input(|i| i.predicted_dt);
        if self.anim_showing {
            self.anim_progress = (self.anim_progress + dt / ANIM_DURATION).min(1.0);
        } else {
            self.anim_progress = (self.anim_progress - dt / ANIM_DURATION).max(0.0);
            if self.anim_progress <= 0.0 {
                self.anim_hide_pending = true;
            }
        }
        // Request repaint while animating in either direction
        let animating = if self.anim_showing {
            self.anim_progress < 1.0
        } else {
            self.anim_progress > 0.0
        };
        if animating {
            ctx.request_repaint();
        }

        // Ease-out curve: fast start, gentle settle
        let t = ease_out_cubic(self.anim_progress);

        // Pre-consume Escape before egui widgets eat it (TextEdit steals it to unfocus).
        // Skip input during close animation.
        if self.anim_showing {
            let escape_pressed = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if escape_pressed {
                if self.mode == Mode::Add {
                    self.mode = Mode::Search;
                    self.query.clear();
                    self.selected_idx = None;
                    self.focus_search = true;
                } else {
                    self.begin_hide();
                }
            }
        }

        // Single central panel — we paint everything ourselves so it all fades together
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                ui.set_opacity(t);

                // Rounded card fills the entire window. Pixels outside the rounded rect are
                // transparent (wgpu clears to a=0), so the compositor shows whatever is behind,
                // giving true rounded window corners without compositor-side decoration tricks.
                let full_rect = ui.max_rect();
                let rounding = 16.0;
                let card_rect = full_rect;
                if card_rect.width() > 0.0 && card_rect.height() > 0.0 {
                    ui.painter().rect_filled(card_rect, rounding, crate::ui::theme::CARD_BG);
                    ui.painter().rect_stroke(card_rect, rounding, egui::Stroke::new(1.0, crate::ui::theme::BORDER));

                    let builder = egui::UiBuilder::new()
                        .max_rect(card_rect.shrink(1.0))
                        .layout(egui::Layout::top_down(egui::Align::Min));
                    ui.allocate_new_ui(builder, |ui| {
                        ui.style_mut().spacing.item_spacing = egui::Vec2::new(0.0, 0.0);
                        ui.style_mut().visuals.selection.bg_fill = crate::ui::theme::ACCENT;
                        ui.style_mut().visuals.widgets.noninteractive.bg_stroke.color =
                            crate::ui::theme::SEPARATOR;

                        let placeholder = match self.mode {
                            Mode::Search => "Search projects...",
                            Mode::Add => "Type directory path...",
                        };
                        let should_focus = self.focus_search || self.selected_idx.is_none();
                        self.focus_search = false;
                        let search_changed = crate::ui::search::search_bar(ui, &mut self.query, placeholder, should_focus);
                        if search_changed { self.selected_idx = None; }

                        ui.add(egui::Separator::default().horizontal().spacing(0.0));

                        let hints_height = 40.0f32;
                        let scroll_height = (ui.available_height() - hints_height).max(0.0);

                        egui::ScrollArea::vertical()
                            .max_height(scroll_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.render_list(ui);
                            });

                        crate::ui::hints::hints_bar(ui);
                    });
                }
            });

        // Only process keyboard when fully interactive (not during close animation)
        if self.anim_showing {
            self.handle_keyboard(ctx);
        }
    }

    fn render_list(&mut self, ui: &mut egui::Ui) {
        match self.mode {
            Mode::Search => self.render_search_list(ui),
            Mode::Add    => self.render_add_list(ui),
        }
    }

    /// "Add project" shows when the query is empty or fuzzy-matches the action name.
    fn show_add_action(&self) -> bool {
        self.query.is_empty() || crate::projects::fuzzy_match(&self.query, "add project")
    }

    fn render_search_list(&mut self, ui: &mut egui::Ui) {
        let show_add = self.show_add_action();
        let offset_add = if show_add { 1 } else { 0 };
        let filtered_pinned = self.filtered_pinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let offset_recent = offset_add + filtered_pinned.len();
        let filtered_recent = self.filtered_recents_unpinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();

        if show_add {
            crate::ui::list::section_header(ui, "Actions");
            let add_selected = self.selected_idx == Some(0);
            if crate::ui::list::action_row(ui, "Add project", add_selected) {
                self.enter_add_mode();
            }
        }

        if !filtered_pinned.is_empty() {
            crate::ui::list::section_header(ui, "Pinned Projects");
            for (i, path) in filtered_pinned.iter().enumerate() {
                let selected = self.selected_idx == Some(i + offset_add);
                let icon = self.icon_textures.get(path.as_str());
                if crate::ui::list::project_row(ui, path, icon, selected) {
                    self.open_project(path);
                }
            }
        }

        if !filtered_recent.is_empty() {
            crate::ui::list::section_header(ui, "Recent Projects");
            for (i, path) in filtered_recent.iter().enumerate() {
                let selected = self.selected_idx == Some(i + offset_recent);
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
        self.focus_search = true;
    }

    fn add_and_open(&mut self, path: &str) {
        let abs = crate::paths::expand_tilde(path);
        if std::path::Path::new(&abs).is_dir() {
            self.open_project(&abs);
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let selectable_count = self.selectable_count();

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        match key {
                            // Escape handled via pre-consume in ui() before TextEdit eats it
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
                                        let offset_add = if self.show_add_action() { 1 } else { 0 };
                                        let filtered_pinned = self.filtered_pinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                        let offset_recent = offset_add + filtered_pinned.len();
                                        if idx >= offset_recent {
                                            let filtered_recent = self.filtered_recents_unpinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                            if let Some(path) = filtered_recent.get(idx - offset_recent) {
                                                let path = path.clone();
                                                self.remove_project(&path);
                                                self.selected_idx = None;
                                                self.focus_search = true;
                                            }
                                        }
                                    }
                                }
                            }
                            egui::Key::P if modifiers.alt => {
                                if let Some(idx) = self.selected_idx {
                                    if self.mode == Mode::Search {
                                        let offset_add = if self.show_add_action() { 1 } else { 0 };
                                        let filtered_pinned = self.filtered_pinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                        let offset_recent = offset_add + filtered_pinned.len();
                                        let path = if idx >= offset_recent {
                                            let filtered_recent = self.filtered_recents_unpinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                            filtered_recent.get(idx - offset_recent).cloned()
                                        } else if idx >= offset_add {
                                            filtered_pinned.get(idx - offset_add).cloned()
                                        } else {
                                            None
                                        };
                                        if let Some(path) = path {
                                            self.toggle_pin(&path);
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
                let add = if self.show_add_action() { 1 } else { 0 };
                add + self.filtered_pinned().len() + self.filtered_recents_unpinned().len()
            }
            Mode::Add => self.suggestions().len(),
        }
    }

    fn move_selection(&mut self, delta: i32, count: usize) {
        if count == 0 { return; }
        self.selected_idx = match self.selected_idx {
            None if delta > 0 => Some(0),
            None => None,
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 { None } else { Some((next as usize).min(count - 1)) }
            }
        };
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
                let show_add = self.show_add_action();
                let offset_add = if show_add { 1 } else { 0 };
                let filtered_pinned = self.filtered_pinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                let offset_recent = offset_add + filtered_pinned.len();

                if show_add && idx == 0 {
                    self.enter_add_mode();
                } else if idx < offset_recent {
                    let path = filtered_pinned[idx - offset_add].clone();
                    self.open_project(&path);
                } else {
                    let filtered_recent = self.filtered_recents_unpinned().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    if let Some(path) = filtered_recent.get(idx - offset_recent) {
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

/// Ease-out cubic: fast start, gentle settle.
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
