pub struct App {
    pub visible: bool,
}

impl App {
    pub fn new() -> Self {
        Self { visible: false }
    }
    pub fn on_show(&mut self) { self.visible = true; }
    pub fn on_hide(&mut self) { self.visible = false; }
    pub fn drain_actions(&mut self) -> Vec<AppAction> { vec![] }
    pub fn ui(&mut self, _ctx: &egui::Context) {}
}

#[derive(Debug)]
pub enum AppAction {
    Hide,
    OpenTerminal(String),
    RemoveProject(String),
}
