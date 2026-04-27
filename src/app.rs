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

    pub fn ui(&mut self, _ctx: &egui::Context) {
        // TODO: implemented in Task 18
    }
}
