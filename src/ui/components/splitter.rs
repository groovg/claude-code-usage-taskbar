use eframe::egui;

use crate::ui::theme::{ACCENT, SPLITTER_HOVER_SURFACE, SPLITTER_IDLE};

pub(crate) fn vertical_splitter(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> egui::Response {
    let response = ui.interact(rect, ui.make_persistent_id(id), egui::Sense::drag());
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let active = response.hovered() || response.dragged();
    if active {
        ui.painter().rect_filled(rect, 2.0, SPLITTER_HOVER_SURFACE);
    }
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top() + 8.0),
            egui::pos2(rect.center().x, rect.bottom() - 8.0),
        ],
        egui::Stroke::new(
            if active { 2.0 } else { 1.0 },
            if active { ACCENT } else { SPLITTER_IDLE },
        ),
    );
    response
}
