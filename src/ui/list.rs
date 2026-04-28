use egui::{Ui, Response, Color32, Rect, Vec2, Pos2};
use crate::ui::theme;

pub fn section_header(ui: &mut Ui, label: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(label.to_uppercase())
                .size(theme::FONT_SECTION)
                .color(theme::SECTION_HEADER),
        );
    });
    ui.add_space(4.0);
}

/// Draws a clickable row background, returns response. `selected` highlights the row.
fn row_background(ui: &mut Ui, height: f32, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    let bg = if selected {
        theme::ROW_SELECTED
    } else if response.hovered() {
        theme::ROW_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    response
}

/// Returns true if clicked.
pub fn action_row(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let response = row_background(ui, theme::ROW_H_ACTION, selected);

    // Icon + label, left-aligned
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(response.rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(16.0);
    child.label(
        egui::RichText::new(egui_phosphor::regular::FOLDER_PLUS)
            .size(theme::ICON_SIZE)
            .color(theme::TEXT_MUTED),
    );
    child.add_space(12.0);
    child.label(egui::RichText::new(label).size(theme::FONT_TITLE).color(theme::TEXT_PRIMARY));

    // Caret, right-aligned with generous padding from edge
    let caret_width = 24.0;
    let caret_margin = 20.0;
    let caret_rect = Rect::from_min_size(
        Pos2::new(response.rect.right() - caret_width - caret_margin, response.rect.top()),
        Vec2::new(caret_width, theme::ROW_H_ACTION),
    );
    let mut right_child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(caret_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    right_child.label(
        egui::RichText::new(egui_phosphor::regular::CARET_RIGHT)
            .size(16.0)
            .color(theme::TEXT_MUTED),
    );

    response.clicked()
}

/// Returns true if clicked. `icon_texture` is optional resolved icon.
pub fn project_row(
    ui: &mut Ui,
    path: &str,
    icon: Option<&egui::TextureHandle>,
    selected: bool,
) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    let response = row_background(ui, theme::ROW_H_PROJECT, selected);

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(response.rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(16.0);

    if let Some(tex) = icon {
        child.image(egui::load::SizedTexture::new(tex.id(), [theme::ICON_SIZE, theme::ICON_SIZE]));
    } else {
        child.label(
            egui::RichText::new(egui_phosphor::regular::FOLDER)
                .size(theme::ICON_SIZE)
                .color(theme::TEXT_MUTED),
        );
    }
    child.add_space(12.0);

    // Two-line text block, vertically centered in the row
    child.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.style_mut().spacing.item_spacing.y = 1.0;
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(name).size(theme::FONT_TITLE).strong().color(theme::TEXT_PRIMARY),
        );
        ui.label(
            egui::RichText::new(path).size(theme::FONT_SUBTITLE).color(theme::TEXT_MUTED),
        );
    });

    response.clicked()
}

pub fn suggestion_row(ui: &mut Ui, path: &str, selected: bool) -> bool {
    let response = row_background(ui, theme::ROW_H_ACTION, selected);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(response.rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(16.0);
    child.label(
        egui::RichText::new(egui_phosphor::regular::FOLDER)
            .size(theme::ICON_SIZE)
            .color(theme::TEXT_MUTED),
    );
    child.add_space(12.0);
    child.label(egui::RichText::new(path).size(theme::FONT_TITLE).color(theme::TEXT_PRIMARY));
    response.clicked()
}
