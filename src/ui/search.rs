use egui::{Ui, TextEdit};
use crate::ui::theme;

pub const SEARCH_ID: &str = "project_picker_search";

/// Returns true if the query changed. If `request_focus` is true, grabs keyboard focus.
pub fn search_bar(ui: &mut Ui, query: &mut String, placeholder: &str, request_focus: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(egui::RichText::new("⌕").size(18.0).color(theme::TEXT_MUTED));
        ui.add_space(8.0);
        let response = ui.add(
            TextEdit::singleline(query)
                .id(egui::Id::new(SEARCH_ID))
                .hint_text(egui::RichText::new(placeholder).color(theme::TEXT_MUTED))
                .frame(false)
                .desired_width(f32::INFINITY)
                .font(egui::FontId::proportional(theme::FONT_TITLE))
                .text_color(theme::TEXT_PRIMARY),
        );
        if request_focus {
            response.request_focus();
        }
        changed = response.changed();
    });
    changed
}
