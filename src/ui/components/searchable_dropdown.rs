use eframe::egui;

use crate::ui::components::dropdown::{dropdown_scroll_height, dropdown_selectable_label};
use crate::ui::components::text_field::singleline;
use crate::ui::theme::MUTED;
use crate::ui::tokens::CONTROL_HEIGHT;

/// A searchable text field backed by a caller-provided list of options.
pub(crate) fn searchable_dropdown<T: AsRef<str>>(
    ui: &mut egui::Ui,
    id: egui::Id,
    selected_value: &mut String,
    options: &[T],
    width: f32,
    hint_text: &str,
    no_results_text: &str,
) {
    let response = ui.add_sized(
        [width, CONTROL_HEIGHT],
        singleline(selected_value)
            .desired_width(f32::INFINITY)
            .id(id)
            .hint_text(hint_text),
    );
    let filter = selected_value.trim().to_lowercase();
    let mut selected = None;
    let popup_width = response.rect.width();
    let popup_frame = egui::Frame::popup(ui.style());
    let popup_content_width = (popup_width - popup_frame.total_margin().sum().x).max(1.0);
    egui::Popup::from_response(&response)
        .id(id.with("suggestions"))
        .width(popup_width)
        .frame(popup_frame)
        // A click on an option takes focus from the field in the same frame, so
        // keep the list for that frame or the click never reaches the option.
        .open(response.has_focus() || response.lost_focus())
        .show(|ui| {
            ui.set_min_width(popup_content_width);
            egui::ScrollArea::vertical()
                .max_height(dropdown_scroll_height(ui))
                .show(ui, |ui| {
                    let mut found = false;
                    for option in options
                        .iter()
                        .map(AsRef::as_ref)
                        .filter(|option| option_matches(option, &filter))
                    {
                        found = true;
                        if dropdown_selectable_label(
                            ui,
                            option.eq_ignore_ascii_case(selected_value),
                            option,
                        )
                        .clicked()
                        {
                            selected = Some(option.to_owned());
                        }
                    }
                    if !found {
                        ui.label(egui::RichText::new(no_results_text).color(MUTED));
                    }
                });
        });
    if let Some(selected) = selected {
        *selected_value = selected;
        ui.memory_mut(|memory| memory.surrender_focus(response.id));
    }
}

pub(crate) fn option_matches(option: &str, lowercase_filter: &str) -> bool {
    lowercase_filter.is_empty() || option.to_lowercase().contains(lowercase_filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_a_suggestion_selects_it() {
        let context = egui::Context::default();
        crate::ui::theme::configure_style(&context, crate::localization::LanguageId::English);
        let id = egui::Id::new("font-search-test");
        let options = ["Arial", "Arial Black", "Consolas"];
        let mut value = "Ar".to_owned();
        let frame = |events: Vec<egui::Event>, value: &mut String| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            let mut output = context.run_ui(input, |ui| {
                searchable_dropdown(ui, id, value, &options, 240.0, "", "");
            });
            output.textures_delta.clear();
        };

        frame(Vec::new(), &mut value);
        context.memory_mut(|memory| memory.request_focus(id));
        // A new popup spends its first frame measuring itself.
        frame(Vec::new(), &mut value);
        frame(Vec::new(), &mut value);
        let popup = context
            .memory(|memory| memory.area_rect(id.with("suggestions")))
            .expect("the suggestions should open while the field has focus");
        let margin = egui::Frame::popup(&context.style_of(egui::Theme::Dark))
            .total_margin()
            .top;
        let option = egui::pos2(
            popup.center().x,
            popup.top() + margin + CONTROL_HEIGHT / 2.0,
        );
        let button = |pressed| egui::Event::PointerButton {
            pos: option,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            vec![egui::Event::PointerMoved(option), button(true)],
            &mut value,
        );
        frame(vec![button(false)], &mut value);

        assert_eq!(value, "Arial");
    }
}
