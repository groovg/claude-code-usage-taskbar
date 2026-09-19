//! The full-page text, expression and action helpers: one shared shell with a
//! title, Discard/Close and Apply buttons, a multiline editor, a status row
//! and the helper-specific reference panels.

use eframe::egui;
use lucide_icons::Icon as LucideIcon;

use crate::localization::LanguageId;
use crate::ui::components::card::card;
use crate::ui::components::icon::{icon_only_button, icon_text, labeled_icon_button};
use crate::ui::theme::{DANGER, HELPER_BORDER, HELPER_SURFACE, MUTED, SUCCESS};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextTemplateFormat {
    Automatic,
    WholeNumber,
    OneDecimal,
    TwoDecimals,
    Percentage,
    ShortDuration,
    DetailedDuration,
    UsageLine,
    UsageBadge,
    WeekdayTwo,
    WeekdayShort,
    WeekdayLong,
    Day,
    DayTwo,
    Month,
    MonthTwo,
    MonthShort,
    MonthLong,
    YearTwo,
    Year,
    DateShort,
    DateLong,
    TimeShort,
    TimeSeconds,
    Time24,
    Time24Seconds,
    Time12,
    Time12Seconds,
    DateTimeShort,
    DateTimeLong,
    IsoDate,
    IsoTime,
    IsoDateTime,
    PlainText,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextTemplateValueKind {
    Number,
    Percentage,
    DisplayPercentage,
    Duration,
    Timestamp,
    UsageSummary,
    Text,
}

/// The draft being edited in a helper plus that helper's own panel state.
pub(crate) struct HelperState<P> {
    pub(crate) draft: String,
    original_draft: String,
    pub(crate) panels: P,
}

impl<P: Default> HelperState<P> {
    pub(crate) fn new(draft: String) -> Self {
        Self {
            original_draft: draft.clone(),
            draft,
            panels: P::default(),
        }
    }
}

pub(crate) struct TextHelperPanels {
    pub(crate) value_filter: String,
    pub(crate) selected_value: &'static str,
    pub(crate) selected_format: TextTemplateFormat,
}

impl Default for TextHelperPanels {
    fn default() -> Self {
        Self {
            value_filter: String::new(),
            selected_value: "active.session.percentage",
            selected_format: TextTemplateFormat::Percentage,
        }
    }
}

#[derive(Default)]
pub(crate) struct ExpressionHelperPanels {
    pub(crate) variable_filter: String,
    pub(crate) function_filter: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelperAction {
    Continue,
    Close,
    Apply,
}

/// What differs between the helpers' shells. Strings are English locale keys.
struct HelperLayout {
    title: &'static str,
    detail: &'static str,
    discard_hint: &'static str,
    close_hint: &'static str,
    editor_hint: &'static str,
    editor_height: f32,
    code_editor: bool,
    min_panel_height: f32,
}

fn helper_shell<P>(
    ui: &mut egui::Ui,
    state: &mut HelperState<P>,
    language: LanguageId,
    layout: HelperLayout,
    can_apply: bool,
    status: impl FnOnce(&mut egui::Ui, &str),
    render_reference_panels: impl FnOnce(&mut egui::Ui, &mut HelperState<P>, f32),
) -> HelperAction {
    let mut action = HelperAction::Continue;
    let width = ui.available_width();
    let height = ui.available_height();

    egui::Frame::new()
        .fill(HELPER_SURFACE)
        .stroke(egui::Stroke::new(1.0, HELPER_BORDER))
        .corner_radius(egui::CornerRadius::same(7))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width((width - 28.0).max(1.0));
            ui.set_min_height((height - 28.0).max(1.0));
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(language.text(layout.title))
                            .size(20.0)
                            .strong(),
                    );
                    ui.label(egui::RichText::new(language.text(layout.detail)).color(MUTED));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let close_response = if state.draft != state.original_draft {
                        ui.add(egui::Button::new((
                            language.text("Discard"),
                            icon_text(LucideIcon::X, 16.0),
                        )))
                        .on_hover_text(language.text(layout.discard_hint))
                    } else {
                        ui.add(icon_only_button(LucideIcon::X))
                            .on_hover_text(language.text(layout.close_hint))
                    };
                    if close_response.clicked() {
                        action = HelperAction::Close;
                    }
                    if ui
                        .add_enabled(
                            can_apply,
                            labeled_icon_button(LucideIcon::Save, language.text("Apply")),
                        )
                        .clicked()
                    {
                        action = HelperAction::Apply;
                    }
                });
            });
            ui.add_space(10.0);
            let mut editor = egui::TextEdit::multiline(&mut state.draft);
            if layout.code_editor {
                editor = editor.code_editor();
            }
            ui.add_sized(
                [ui.available_width(), layout.editor_height],
                editor
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::same(10))
                    .hint_text(language.text(layout.editor_hint)),
            );
            status(ui, &state.draft);
            ui.add_space(10.0);

            let panel_height = ui.available_height().max(layout.min_panel_height);
            render_reference_panels(ui, state, panel_height);
        });

    action
}

fn valid_row(ui: &mut egui::Ui, message: &str) {
    ui.label(icon_text(LucideIcon::CheckCircle, 15.0).color(SUCCESS));
    ui.colored_label(SUCCESS, message);
}

fn error_row(ui: &mut egui::Ui, error: impl Into<egui::RichText>) {
    ui.label(icon_text(LucideIcon::AlertCircle, 15.0).color(DANGER));
    ui.colored_label(DANGER, error);
}

pub(crate) fn show_text_helper(
    ui: &mut egui::Ui,
    state: &mut HelperState<TextHelperPanels>,
    language: LanguageId,
    validate: impl Fn(&str) -> Vec<String>,
    build_preview: impl Fn(&str) -> String,
    render_reference_panels: impl FnOnce(&mut egui::Ui, &mut HelperState<TextHelperPanels>, f32),
) -> HelperAction {
    let can_apply = validate(&state.draft).is_empty();
    let layout = HelperLayout {
        title: "Text helper",
        detail: "Build text from regular words and correctly formatted provider values.",
        discard_hint: "Discard text changes",
        close_hint: "Close text helper",
        editor_hint: "Type text here, then insert provider values below...",
        editor_height: 116.0,
        code_editor: false,
        min_panel_height: 150.0,
    };
    helper_shell(
        ui,
        state,
        language,
        layout,
        can_apply,
        |ui, draft| {
            let validation = validate(draft);
            let preview = build_preview(draft);
            ui.add_space(8.0);
            card(
                ui,
                ui.available_width(),
                76.0,
                language.text("Live preview"),
                egui::Margin::symmetric(11, 9),
                |ui| {
                    if preview.is_empty() {
                        ui.label(
                            egui::RichText::new(language.text("Preview is empty"))
                                .italics()
                                .color(MUTED),
                        );
                    } else {
                        ui.label(egui::RichText::new(&preview).size(18.0).strong());
                    }
                },
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if validation.is_empty() {
                    valid_row(ui, language.text("Template is valid"));
                } else {
                    error_row(ui, validation.join(" · "));
                }
            });
        },
        render_reference_panels,
    )
}

pub(crate) fn show_expression_helper(
    ui: &mut egui::Ui,
    state: &mut HelperState<ExpressionHelperPanels>,
    language: LanguageId,
    evaluate: impl Fn(&str) -> Result<String, String>,
    render_reference_panels: impl FnOnce(&mut egui::Ui, &mut HelperState<ExpressionHelperPanels>, f32),
) -> HelperAction {
    let can_apply = evaluate(&state.draft).is_ok();
    let layout = HelperLayout {
        title: "Expression helper",
        detail: "Build and validate an expression using the values supported by the theme engine.",
        discard_hint: "Discard expression changes",
        close_hint: "Close expression helper",
        editor_hint: "Enter an expression...",
        editor_height: 132.0,
        code_editor: true,
        min_panel_height: 180.0,
    };
    helper_shell(
        ui,
        state,
        language,
        layout,
        can_apply,
        |ui, draft| {
            let validation = evaluate(draft);
            ui.add_space(6.0);
            ui.horizontal(|ui| match &validation {
                Ok(result) => {
                    valid_row(ui, language.text("Valid expression"));
                    ui.separator();
                    ui.label(egui::RichText::new(language.text("Current result")).color(MUTED));
                    ui.label(egui::RichText::new(result).strong());
                }
                Err(error) => error_row(ui, error),
            });
        },
        render_reference_panels,
    )
}

pub(crate) fn show_action_helper(
    ui: &mut egui::Ui,
    state: &mut HelperState<()>,
    language: LanguageId,
    detail: &'static str,
    validate: impl Fn(&str) -> Result<String, String>,
    render_reference_panels: impl FnOnce(&mut egui::Ui, &mut HelperState<()>, f32),
) -> HelperAction {
    let can_apply = validate(&state.draft).is_ok();
    let layout = HelperLayout {
        title: "Action helper",
        detail,
        discard_hint: "Discard action changes",
        close_hint: "Close action helper",
        editor_hint: "Enter actions...",
        editor_height: 132.0,
        code_editor: true,
        min_panel_height: 180.0,
    };
    helper_shell(
        ui,
        state,
        language,
        layout,
        can_apply,
        |ui, draft| {
            let validation = validate(draft);
            ui.add_space(6.0);
            ui.horizontal(|ui| match validation {
                Ok(summary) => {
                    valid_row(ui, language.text("Valid actions"));
                    if !summary.is_empty() {
                        ui.separator();
                        ui.label(summary);
                    }
                }
                Err(error) => error_row(ui, error),
            });
        },
        render_reference_panels,
    )
}
