use super::*;
use TextTemplateFormat as Format;
use TextTemplateValueKind as Kind;

#[derive(Clone, Copy)]
pub(super) struct TextTemplateValue {
    pub(super) group: &'static str,
    pub(super) label: &'static str,
    pub(super) expression: &'static str,
    pub(super) kind: TextTemplateValueKind,
}

const fn v(
    group: &'static str,
    label: &'static str,
    expression: &'static str,
    kind: TextTemplateValueKind,
) -> TextTemplateValue {
    TextTemplateValue {
        group,
        label,
        expression,
        kind,
    }
}

#[rustfmt::skip]
pub(super) const TEXT_TEMPLATE_VALUES: &[TextTemplateValue] = &[
    v("Date and time", "Current date and time", "time.now.unix", Kind::Timestamp),
    v("Application", "App version", "app.version", Kind::Text),
    v("Application", "App version major", "app.version.major", Kind::Number),
    v("Application", "App version minor", "app.version.minor", Kind::Number),
    v("Application", "App version patch", "app.version.patch", Kind::Number),
    v("General", "Enabled provider count", "providers.count", Kind::Number),
    v("General", "Counting down", "display.countdown", Kind::Number),
    v("Active provider", "Session summary", "active.session", Kind::UsageSummary),
    v("Active provider", "Session used", "active.session.percentage", Kind::Percentage),
    v("Active provider", "Session remaining", "active.session.remaining", Kind::Percentage),
    v("Active provider", "Session shown", "active.session.display", Kind::DisplayPercentage),
    v("Active provider", "Session reset", "active.session.reset.seconds", Kind::Duration),
    v("Active provider", "Session reset date and time", "active.session.reset.unix", Kind::Timestamp),
    v("Active provider", "Weekly summary", "active.weekly", Kind::UsageSummary),
    v("Active provider", "Weekly used", "active.weekly.percentage", Kind::Percentage),
    v("Active provider", "Weekly remaining", "active.weekly.remaining", Kind::Percentage),
    v("Active provider", "Weekly shown", "active.weekly.display", Kind::DisplayPercentage),
    v("Active provider", "Weekly reset", "active.weekly.reset.seconds", Kind::Duration),
    v("Active provider", "Weekly reset date and time", "active.weekly.reset.unix", Kind::Timestamp),
    v("Claude Code", "Session summary", "claude.session", Kind::UsageSummary),
    v("Claude Code", "Session used", "claude.session.percentage", Kind::Percentage),
    v("Claude Code", "Session remaining", "claude.session.remaining", Kind::Percentage),
    v("Claude Code", "Session shown", "claude.session.display", Kind::DisplayPercentage),
    v("Claude Code", "Session reset", "claude.session.reset.seconds", Kind::Duration),
    v("Claude Code", "Weekly summary", "claude.weekly", Kind::UsageSummary),
    v("Claude Code", "Weekly used", "claude.weekly.percentage", Kind::Percentage),
    v("Claude Code", "Weekly remaining", "claude.weekly.remaining", Kind::Percentage),
    v("Claude Code", "Weekly shown", "claude.weekly.display", Kind::DisplayPercentage),
    v("Claude Code", "Weekly reset", "claude.weekly.reset.seconds", Kind::Duration),
    v("Claude Code", "Model cap name (Fable)", "claude.scoped.label", Kind::Text),
    v("Claude Code", "Model cap summary", "claude.scoped", Kind::UsageSummary),
    v("Claude Code", "Model cap used", "claude.scoped.percentage", Kind::Percentage),
    v("Claude Code", "Model cap shown", "claude.scoped.display", Kind::DisplayPercentage),
    v("Claude Code", "Model cap reset", "claude.scoped.reset.seconds", Kind::Duration),
    v("Claude Code", "Session context summary", "claude.context", Kind::UsageSummary),
    v("Claude Code", "Session context used", "claude.context.percentage", Kind::Percentage),
    v("Claude Code", "Session context tokens", "claude.context.tokens", Kind::Number),
    v("Claude Code", "Session context window", "claude.context.window", Kind::Number),
    v("Claude Code", "Session context project folder", "claude.context.project", Kind::Text),
    v("Codex", "Session summary", "codex.session", Kind::UsageSummary),
    v("Codex", "Session used", "codex.session.percentage", Kind::Percentage),
    v("Codex", "Session remaining", "codex.session.remaining", Kind::Percentage),
    v("Codex", "Session shown", "codex.session.display", Kind::DisplayPercentage),
    v("Codex", "Session reset", "codex.session.reset.seconds", Kind::Duration),
    v("Codex", "Five-hour summary (exact)", "codex.five_hour", Kind::UsageSummary),
    v("Codex", "Five-hour used (exact)", "codex.five_hour.percentage", Kind::Percentage),
    v("Codex", "Five-hour remaining (exact)", "codex.five_hour.remaining", Kind::Percentage),
    v("Codex", "Five-hour shown (exact)", "codex.five_hour.display", Kind::DisplayPercentage),
    v("Codex", "Five-hour reset (exact)", "codex.five_hour.reset.seconds", Kind::Duration),
    v("Codex", "Five-hour reset date and time (exact)", "codex.five_hour.reset.unix", Kind::Timestamp),
    v("Codex", "Weekly summary", "codex.weekly", Kind::UsageSummary),
    v("Codex", "Weekly used", "codex.weekly.percentage", Kind::Percentage),
    v("Codex", "Weekly remaining", "codex.weekly.remaining", Kind::Percentage),
    v("Codex", "Weekly shown", "codex.weekly.display", Kind::DisplayPercentage),
    v("Codex", "Weekly reset", "codex.weekly.reset.seconds", Kind::Duration),
    v("Codex", "Weekly reset date and time", "codex.weekly.reset.unix", Kind::Timestamp),
    v("Antigravity", "Session summary", "antigravity.session", Kind::UsageSummary),
    v("Antigravity", "Session used", "antigravity.session.percentage", Kind::Percentage),
    v("Antigravity", "Session remaining", "antigravity.session.remaining", Kind::Percentage),
    v("Antigravity", "Session shown", "antigravity.session.display", Kind::DisplayPercentage),
    v("Antigravity", "Session reset", "antigravity.session.reset.seconds", Kind::Duration),
    v("Antigravity", "Weekly summary", "antigravity.weekly", Kind::UsageSummary),
    v("Antigravity", "Weekly used", "antigravity.weekly.percentage", Kind::Percentage),
    v("Antigravity", "Weekly remaining", "antigravity.weekly.remaining", Kind::Percentage),
    v("Antigravity", "Weekly shown", "antigravity.weekly.display", Kind::DisplayPercentage),
    v("Antigravity", "Weekly reset", "antigravity.weekly.reset.seconds", Kind::Duration),
    v("OpenCode", "Session summary", "opencode.session", Kind::UsageSummary),
    v("OpenCode", "Session used", "opencode.session.percentage", Kind::Percentage),
    v("OpenCode", "Session remaining", "opencode.session.remaining", Kind::Percentage),
    v("OpenCode", "Session shown", "opencode.session.display", Kind::DisplayPercentage),
    v("OpenCode", "Session reset", "opencode.session.reset.seconds", Kind::Duration),
    v("OpenCode", "Long-window label", "opencode.weekly.label", Kind::Text),
    v("OpenCode", "Long-window summary", "opencode.weekly", Kind::UsageSummary),
    v("OpenCode", "Long-window used", "opencode.weekly.percentage", Kind::Percentage),
    v("OpenCode", "Long-window remaining", "opencode.weekly.remaining", Kind::Percentage),
    v("OpenCode", "Long-window shown", "opencode.weekly.display", Kind::DisplayPercentage),
    v("OpenCode", "Long-window reset", "opencode.weekly.reset.seconds", Kind::Duration),
    v("Cursor", "Auto summary", "cursor.session", Kind::UsageSummary),
    v("Cursor", "Auto used", "cursor.session.percentage", Kind::Percentage),
    v("Cursor", "Auto remaining", "cursor.session.remaining", Kind::Percentage),
    v("Cursor", "Auto shown", "cursor.session.display", Kind::DisplayPercentage),
    v("Cursor", "Auto reset", "cursor.session.reset.seconds", Kind::Duration),
    v("Cursor", "API summary", "cursor.weekly", Kind::UsageSummary),
    v("Cursor", "API used", "cursor.weekly.percentage", Kind::Percentage),
    v("Cursor", "API remaining", "cursor.weekly.remaining", Kind::Percentage),
    v("Cursor", "API shown", "cursor.weekly.display", Kind::DisplayPercentage),
    v("Cursor", "API reset", "cursor.weekly.reset.seconds", Kind::Duration),
    v("Labels", "Session window label", "i18n.session_window", Kind::Text),
    v("Labels", "Weekly window label", "i18n.weekly_window", Kind::Text),
    v("Labels", "Now label", "i18n.now", Kind::Text),
];

pub(super) fn text_template_value(expression: &str) -> Option<TextTemplateValue> {
    TEXT_TEMPLATE_VALUES
        .iter()
        .copied()
        .find(|value| value.expression == expression)
}

pub(super) fn text_template_formats(kind: TextTemplateValueKind) -> &'static [TextTemplateFormat] {
    match kind {
        TextTemplateValueKind::Number => &[
            Format::Automatic,
            Format::WholeNumber,
            Format::OneDecimal,
            Format::TwoDecimals,
        ],
        TextTemplateValueKind::Percentage => &[
            Format::Percentage,
            Format::WholeNumber,
            Format::OneDecimal,
            Format::TwoDecimals,
            Format::Automatic,
        ],
        TextTemplateValueKind::DisplayPercentage => &[
            Format::Percentage,
            Format::WholeNumber,
            Format::OneDecimal,
            Format::TwoDecimals,
            Format::Automatic,
            Format::UsageLine,
            Format::UsageBadge,
        ],
        TextTemplateValueKind::Duration => &[
            Format::ShortDuration,
            Format::DetailedDuration,
            Format::WholeNumber,
        ],
        TextTemplateValueKind::Timestamp => &[
            Format::WeekdayTwo,
            Format::WeekdayShort,
            Format::WeekdayLong,
            Format::Day,
            Format::DayTwo,
            Format::Month,
            Format::MonthTwo,
            Format::MonthShort,
            Format::MonthLong,
            Format::YearTwo,
            Format::Year,
            Format::DateShort,
            Format::DateLong,
            Format::TimeShort,
            Format::TimeSeconds,
            Format::Time24,
            Format::Time24Seconds,
            Format::Time12,
            Format::Time12Seconds,
            Format::DateTimeShort,
            Format::DateTimeLong,
            Format::IsoDate,
            Format::IsoTime,
            Format::IsoDateTime,
            Format::WholeNumber,
        ],
        TextTemplateValueKind::UsageSummary => &[Format::UsageLine, Format::UsageBadge],
        TextTemplateValueKind::Text => &[Format::PlainText],
    }
}

pub(super) fn default_text_template_format(kind: TextTemplateValueKind) -> TextTemplateFormat {
    text_template_formats(kind)[0]
}

/// Each format's English label and the token suffix it inserts.
#[rustfmt::skip]
const TEXT_TEMPLATE_FORMAT_INFO: &[(TextTemplateFormat, &str, Option<&str>)] = &[
    (Format::Automatic, "Automatic number", Some("0.##")),
    (Format::WholeNumber, "Whole number", Some("0")),
    (Format::OneDecimal, "One decimal", Some("0.0")),
    (Format::TwoDecimals, "Two decimals", Some("0.00")),
    (Format::Percentage, "Percentage", Some("percent")),
    (Format::ShortDuration, "Short duration", Some("duration_short")),
    (Format::DetailedDuration, "Detailed duration", Some("duration")),
    (Format::UsageLine, "Usage and reset", Some("usage_line")),
    (Format::UsageBadge, "Usage only", Some("usage_badge")),
    (Format::WeekdayTwo, "Weekday (2 letters)", Some("weekday_2")),
    (Format::WeekdayShort, "Weekday (short)", Some("weekday_short")),
    (Format::WeekdayLong, "Weekday (full)", Some("weekday_long")),
    (Format::Day, "Day", Some("day")),
    (Format::DayTwo, "Day (2 digits)", Some("day_2")),
    (Format::Month, "Month", Some("month")),
    (Format::MonthTwo, "Month (2 digits)", Some("month_2")),
    (Format::MonthShort, "Month (short)", Some("month_short")),
    (Format::MonthLong, "Month (full)", Some("month_long")),
    (Format::YearTwo, "Year (2 digits)", Some("year_2")),
    (Format::Year, "Year", Some("year")),
    (Format::DateShort, "Short date", Some("date_short")),
    (Format::DateLong, "Long date", Some("date_long")),
    (Format::TimeShort, "Time", Some("time_short")),
    (Format::TimeSeconds, "Time with seconds", Some("time_seconds")),
    (Format::Time24, "24-hour time", Some("time_24")),
    (Format::Time24Seconds, "24-hour time with seconds", Some("time_24_seconds")),
    (Format::Time12, "12-hour time", Some("time_12")),
    (Format::Time12Seconds, "12-hour time with seconds", Some("time_12_seconds")),
    (Format::DateTimeShort, "Short date and time", Some("datetime_short")),
    (Format::DateTimeLong, "Long date and time", Some("datetime_long")),
    (Format::IsoDate, "ISO date", Some("iso_date")),
    (Format::IsoTime, "ISO time", Some("iso_time")),
    (Format::IsoDateTime, "ISO date and time", Some("iso_datetime")),
    (Format::PlainText, "Plain text", None),
];

fn text_template_format_info(format: TextTemplateFormat) -> (&'static str, Option<&'static str>) {
    TEXT_TEMPLATE_FORMAT_INFO
        .iter()
        .find(|(candidate, _, _)| *candidate == format)
        .map(|(_, label, code)| (*label, *code))
        .expect("every text template format is listed")
}

pub(super) fn text_template_format_label(
    language: LanguageId,
    format: TextTemplateFormat,
) -> &'static str {
    language.text(text_template_format_info(format).0)
}

pub(super) fn text_template_format_code(format: TextTemplateFormat) -> Option<&'static str> {
    text_template_format_info(format).1
}

pub(super) fn text_template_token(expression: &str, format: TextTemplateFormat) -> String {
    text_template_format_code(format).map_or_else(
        || format!("{{{expression}}}"),
        |format| format!("{{{expression}:{format}}}"),
    )
}

pub(super) fn set_text_template(content: &mut SceneContent, template: String) -> bool {
    match content {
        SceneContent::Text {
            template: current, ..
        } => {
            *current = template;
            true
        }
        _ => false,
    }
}

pub(super) fn format_number_for_ui(value: f64) -> String {
    if value.fract().abs() < 0.000_001 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub(super) fn append_expression_token(draft: &mut String, token: &str) {
    let needs_space = !draft.is_empty()
        && !draft.ends_with(char::is_whitespace)
        && !draft.ends_with('(')
        && !token.starts_with(')')
        && !token.starts_with(',');
    if needs_space {
        draft.push(' ');
    }
    draft.push_str(token);
}

pub(super) fn text_template_value_sample(
    value: TextTemplateValue,
    format: TextTemplateFormat,
    context: &DataContext,
) -> String {
    theme_engine::format_template(&text_template_token(value.expression, format), context)
}

pub(super) fn text_template_values_panel(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    context: &DataContext,
    filter: &mut String,
    selected_value: &mut &'static str,
    selected_format: &mut TextTemplateFormat,
    language: LanguageId,
) {
    expression_reference_card(ui, size.x, size.y, language.text("Provider values"), |ui| {
        ui.add(
            singleline_text_edit(filter)
                .desired_width(ui.available_width())
                .hint_text(language.text("Search values...")),
        );
        ui.add_space(4.0);
        let needle = filter.trim().to_ascii_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("text-template-values")
            .auto_shrink([false, false])
            .content_margin(egui::Margin {
                right: 12,
                ..egui::Margin::ZERO
            })
            .max_height((size.y - 72.0).max(80.0))
            .show(ui, |ui| {
                let mut last_group = "";
                for value in TEXT_TEMPLATE_VALUES.iter().copied().filter(|value| {
                    needle.is_empty()
                        || value.label.to_ascii_lowercase().contains(&needle)
                        || language.text(value.label).to_lowercase().contains(&needle)
                        || value.group.to_ascii_lowercase().contains(&needle)
                        || language.text(value.group).to_lowercase().contains(&needle)
                        || value.expression.to_ascii_lowercase().contains(&needle)
                }) {
                    if value.group != last_group {
                        if !last_group.is_empty() {
                            ui.add_space(6.0);
                        }
                        ui.label(
                            egui::RichText::new(language.text(value.group))
                                .small()
                                .strong()
                                .color(MUTED),
                        );
                        last_group = value.group;
                    }
                    ui.horizontal(|ui| {
                        let is_selected = *selected_value == value.expression;
                        if ui
                            .add(
                                egui::Button::selectable(is_selected, language.text(value.label))
                                    .frame(false),
                            )
                            .on_hover_text(value.expression)
                            .clicked()
                        {
                            *selected_value = value.expression;
                            *selected_format = default_text_template_format(value.kind);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let sample = text_template_value_sample(
                                value,
                                default_text_template_format(value.kind),
                                context,
                            );
                            ui.label(egui::RichText::new(sample).color(MUTED));
                        });
                    });
                }
            });
    });
}

pub(super) fn text_template_formats_panel(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    context: &DataContext,
    selected_value: &str,
    selected_format: &mut TextTemplateFormat,
    draft: &mut String,
    language: LanguageId,
) {
    expression_reference_card(ui, size.x, size.y, language.text("Format"), |ui| {
        let value = text_template_value(selected_value).unwrap_or(TEXT_TEMPLATE_VALUES[0]);
        ui.label(egui::RichText::new(language.text(value.label)).strong());
        ui.label(
            egui::RichText::new(value.expression)
                .small()
                .family(egui::FontFamily::Monospace)
                .color(MUTED),
        );
        ui.add_space(8.0);
        for format in text_template_formats(value.kind) {
            let sample = text_template_value_sample(value, *format, context);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::selectable(
                            *selected_format == *format,
                            text_template_format_label(language, *format),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    *selected_format = *format;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(sample).color(MUTED));
                });
            });
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        let token = text_template_token(value.expression, *selected_format);
        ui.label(egui::RichText::new(&token).small().color(MUTED).monospace());
        ui.add_space(6.0);
        if ui
            .add_sized(
                [ui.available_width(), CONTROL_HEIGHT],
                lucide_labeled_button(LucideIcon::Code, language.text("Insert value")),
            )
            .clicked()
        {
            draft.push_str(&token);
        }
    });
}

pub(super) fn text_template_guide_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    language: LanguageId,
) {
    expression_reference_card(ui, width, height, language.text("Guide"), |ui| {
        ui.label(language.text("Type ordinary words directly in the editor."));
        ui.add_space(8.0);
        ui.label(language.text("Select a provider value, choose its format, then insert it."));
        ui.add_space(8.0);
        ui.label(language.text("Values are inserted at the end of the current text and can be moved or edited afterwards."));
        ui.add_space(8.0);
        ui.label(language.text("To show a literal opening brace, type:"));
        ui.label(egui::RichText::new("{{").monospace().color(MUTED));
        ui.add_space(8.0);
        ui.label(language.text("Advanced expressions are supported inside a value token."));
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn action_reference_panels(
    ui: &mut egui::Ui,
    height: f32,
    targets: &[(String, String)],
    self_id: &str,
    target: &mut String,
    property: &mut MouseActionProperty,
    value: &mut String,
    url: &mut String,
    context_menus: &[context_menu::ContextMenuDescriptor],
    context_menu_reference: &mut String,
    draft: &mut String,
    language: LanguageId,
) {
    let gap = ui.spacing().item_spacing.x;
    let panel_width = ((ui.available_width() - gap * 2.0) / 3.0).max(1.0);
    ui.horizontal(|ui| {
        expression_reference_card(ui, panel_width, height, language.text("Actions"), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("action-helper-actions")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if ui.button(language.text("Show dashboard")).clicked() {
                        append_action(draft, "show_dashboard()");
                    }
                    if ui.button(language.text("Toggle dashboard")).clicked() {
                        append_action(draft, "toggle_dashboard()");
                    }
                    ui.label(
                        egui::RichText::new(language.text("URL"))
                            .small()
                            .color(MUTED),
                    );
                    ui.add(
                        singleline_text_edit(url)
                            .desired_width(ui.available_width())
                            .hint_text("https://example.com/usage"),
                    );
                    if ui
                        .add_enabled(
                            context_menu::supported_url(url),
                            egui::Button::new(language.text("Open URL")),
                        )
                        .on_disabled_hover_text(
                            language.text("Only http and https links are allowed."),
                        )
                        .clicked()
                    {
                        let url = url.replace('\\', "\\\\").replace('"', "\\\"");
                        append_action(draft, &format!("open_url(\"{url}\")"));
                    }
                    ui.label(
                        egui::RichText::new(language.text("Context menu"))
                            .small()
                            .color(MUTED),
                    );
                    Dropdown::from_id_salt("action-helper-context-menu")
                        .width(ui.available_width())
                        .selected_text(
                            context_menus
                                .iter()
                                .find(|menu| menu.id == *context_menu_reference)
                                .map(|menu| menu.name.as_str())
                                .unwrap_or(context_menu_reference.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for menu in context_menus {
                                dropdown_selectable_value(
                                    ui,
                                    context_menu_reference,
                                    menu.id.clone(),
                                    &menu.name,
                                );
                            }
                        });
                    if ui.button(language.text("Show context menu")).clicked() {
                        let reference = context_menu_reference
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"");
                        append_action(draft, &format!("show_context_menu(\"{reference}\")"));
                    }
                    ui.separator();
                    if let Some(action) =
                        property_action_buttons(ui, target, *property, value, true, language)
                    {
                        append_action(draft, &action);
                    }
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(
                            language.text("Actions run from top to bottom in one update."),
                        )
                        .small()
                        .color(MUTED),
                    );
                });
        });
        expression_reference_card(ui, panel_width, height, language.text("Layers"), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("action-helper-layers")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (id, name) in targets {
                        let token = if id.eq_ignore_ascii_case(self_id) {
                            "self".to_string()
                        } else {
                            format!("\"{}\"", id.replace('\\', "\\\\").replace('"', "\\\""))
                        };
                        let label = if token == "self" {
                            format!("{} ({})", language.text("Self"), name)
                        } else {
                            format!("{name}  ·  {id}")
                        };
                        if ui.selectable_label(*target == token, label).clicked() {
                            *target = token;
                        }
                    }
                });
        });
        expression_reference_card(ui, panel_width, height, language.text("Properties"), |ui| {
            property_selector(ui, property, language);
            ui.separator();
            ui.label(
                egui::RichText::new(language.text("Value expression"))
                    .small()
                    .color(MUTED),
            );
            ui.add(
                singleline_text_edit(value)
                    .desired_width(ui.available_width())
                    .hint_text(language.text("e.g. false, 120, parent.width / 2")),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    language.text(
                        "Reset removes the runtime override and restores the saved expression.",
                    ),
                )
                .small()
                .color(MUTED),
            );
        });
    });
}

/// The Set/Toggle/Reset/Increase/Decrease buttons shared by both action
/// helpers. Returns the clicked action. `disabled_hints` adds the tooltips
/// the layer action helper shows on disabled buttons.
pub(super) fn property_action_buttons(
    ui: &mut egui::Ui,
    target: &str,
    property: MouseActionProperty,
    value: &str,
    disabled_hints: bool,
    language: LanguageId,
) -> Option<String> {
    let hint = |response: egui::Response, text: &'static str| {
        if disabled_hints {
            response.on_disabled_hover_text(language.text(text))
        } else {
            response
        }
    };
    let mut action = None;
    if ui.button(language.text("Set property")).clicked() {
        action = Some(format!(
            "set({target}, {}, {})",
            property.name(),
            value.trim()
        ));
    }
    let toggle = ui.add_enabled(
        property == MouseActionProperty::Render,
        egui::Button::new(language.text("Toggle property")),
    );
    if hint(toggle, "Toggle currently supports Render only").clicked() {
        action = Some(format!("toggle({target}, {})", property.name()));
    }
    if ui.button(language.text("Reset property")).clicked() {
        action = Some(format!("reset({target}, {})", property.name()));
    }
    let numeric_property = property != MouseActionProperty::Render;
    let increase = ui.add_enabled(
        numeric_property,
        egui::Button::new(language.text("Increase value")),
    );
    if hint(increase, "Choose a numeric property").clicked() {
        action = Some(format!(
            "increase({target}, {}, {})",
            property.name(),
            value.trim()
        ));
    }
    let decrease = ui.add_enabled(
        numeric_property,
        egui::Button::new(language.text("Decrease value")),
    );
    if hint(decrease, "Choose a numeric property").clicked() {
        action = Some(format!(
            "decrease({target}, {}, {})",
            property.name(),
            value.trim()
        ));
    }
    action
}

pub(super) fn property_selector(
    ui: &mut egui::Ui,
    property: &mut MouseActionProperty,
    language: LanguageId,
) {
    for candidate in MouseActionProperty::ALL {
        let label = match candidate {
            MouseActionProperty::Render => language.text("Render"),
            MouseActionProperty::Visibility => language.text("Visibility"),
            MouseActionProperty::X => language.text("X"),
            MouseActionProperty::Y => language.text("Y"),
            MouseActionProperty::Width => language.text("Width"),
            MouseActionProperty::Height => language.text("Height"),
            MouseActionProperty::Rotation => language.text("Rotation"),
        };
        if ui.selectable_label(*property == candidate, label).clicked() {
            *property = candidate;
        }
    }
}

pub(super) fn append_action(draft: &mut String, action: &str) {
    if !draft.trim().is_empty() && !draft.ends_with('\n') {
        draft.push('\n');
    }
    draft.push_str(action);
}

pub(super) fn expression_variables_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    context: &DataContext,
    filter: &mut String,
    draft: &mut String,
    language: LanguageId,
) {
    expression_reference_card(ui, width, height, language.text("Variables"), |ui| {
        ui.add(
            singleline_text_edit(filter)
                .desired_width(ui.available_width())
                .hint_text(language.text("Search variables...")),
        );
        ui.add_space(4.0);
        let needle = filter.trim().to_ascii_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("expression-variables")
            .auto_shrink([false, false])
            .content_margin(egui::Margin {
                right: 12,
                ..egui::Margin::ZERO
            })
            .max_height((height - 72.0).max(80.0))
            .show(ui, |ui| {
                let owned = |names: &[&str]| -> Vec<String> {
                    names.iter().map(|name| name.to_string()).collect()
                };
                let mut groups: Vec<(&'static str, Vec<String>)> =
                    vec![
                        ("Constants", owned(&["true", "false", "pi", "e"])),
                        (
                            "Layout",
                            owned(&[
                                "canvas.width",
                                "canvas.height",
                                "parent.width",
                                "parent.height",
                                "host.width",
                                "host.height",
                            ]),
                        ),
                        (
                            "Date and time",
                            owned(&[
                                "time.now.unix",
                                "time.now.milliseconds",
                                "time.local.year",
                                "time.local.month",
                                "time.local.day",
                                "time.local.weekday",
                                "time.local.hour",
                                "time.local.minute",
                                "time.local.second",
                                "time.utc.year",
                                "time.utc.month",
                                "time.utc.day",
                                "time.utc.weekday",
                                "time.utc.hour",
                                "time.utc.minute",
                                "time.utc.second",
                            ]),
                        ),
                        (
                            "Providers",
                            std::iter::once("providers.count".to_string())
                                .chain(PROVIDER_DESCRIPTORS.iter().map(|descriptor| {
                                    format!("providers.{}.enabled", descriptor.key)
                                }))
                                .collect(),
                        ),
                        ("Display", owned(&["display.countdown"])),
                        (
                            "Application",
                            owned(&[
                                "app.version.major",
                                "app.version.minor",
                                "app.version.patch",
                            ]),
                        ),
                    ];
                for (title, provider) in std::iter::once(("Active provider", "active")).chain(
                    PROVIDER_DESCRIPTORS
                        .iter()
                        .map(|descriptor| (descriptor.display_name, descriptor.key)),
                ) {
                    let mut names = vec![format!("{provider}.available")];
                    let windows = match provider {
                        "active" => &["session", "five_hour", "weekly", "monthly", "scoped"][..],
                        "codex" => &["session", "five_hour", "weekly", "monthly"][..],
                        "claude" => &["session", "weekly", "monthly", "scoped"][..],
                        _ => &["session", "weekly", "monthly"][..],
                    };
                    for window in windows {
                        for metric in ["available", "percentage", "remaining", "display"] {
                            names.push(format!("{provider}.{window}.{metric}"));
                        }
                        for unit in ["unix", "seconds", "minutes", "hours", "days"] {
                            names.push(format!("{provider}.{window}.reset.{unit}"));
                        }
                    }
                    // Model caps and the session context have no reset window.
                    if matches!(provider, "active" | "claude") {
                        names.push(format!("{provider}.scoped.active"));
                        names.push(format!("{provider}.scoped.count"));
                        for metric in [
                            "available",
                            "percentage",
                            "remaining",
                            "display",
                            "tokens",
                            "window",
                        ] {
                            names.push(format!("{provider}.context.{metric}"));
                        }
                    }
                    groups.push((title, names));
                }
                for (title, names) in &groups {
                    expression_variable_group(
                        ui,
                        language.text(title),
                        names,
                        &needle,
                        context,
                        draft,
                        language,
                    );
                }
            });
    });
}

pub(super) fn expression_variable_group(
    ui: &mut egui::Ui,
    title: &str,
    names: &[String],
    needle: &str,
    context: &DataContext,
    draft: &mut String,
    language: LanguageId,
) {
    let matches: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| needle.is_empty() || name.to_ascii_lowercase().contains(needle))
        .collect();
    if matches.is_empty() {
        return;
    }
    ui.label(egui::RichText::new(title).small().strong().color(MUTED));
    for name in matches {
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::selectable(false, name).frame(false))
                .on_hover_text(language.text("Insert variable"))
                .clicked()
            {
                append_expression_token(draft, name);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let value = context
                    .get(name)
                    .map(format_number_for_ui)
                    .unwrap_or_else(|| "—".into());
                ui.label(egui::RichText::new(value).color(MUTED));
            });
        });
    }
    ui.add_space(6.0);
}

pub(super) fn expression_functions_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    filter: &mut String,
    draft: &mut String,
    language: LanguageId,
) {
    expression_reference_card(ui, width, height, language.text("Functions"), |ui| {
        ui.add(
            singleline_text_edit(filter)
                .desired_width(ui.available_width())
                .hint_text(language.text("Search functions...")),
        );
        ui.add_space(4.0);
        let needle = filter.trim().to_ascii_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("expression-functions")
            .auto_shrink([false, false])
            .max_height((height - 72.0).max(80.0))
            .show(ui, |ui| {
                for (name, signature, insertion, detail) in [
                    (
                        "get",
                        "get(this, property)",
                        "get(this, gap)",
                        "Value expression",
                    ),
                    ("min", "min(a, b)", "min(0, 0)", "Smaller value"),
                    ("max", "max(a, b)", "max(0, 0)", "Larger value"),
                    (
                        "clamp",
                        "clamp(value, min, max)",
                        "clamp(0, 0, 100)",
                        "Constrain a value",
                    ),
                    ("round", "round(value)", "round(0)", "Nearest integer"),
                    ("floor", "floor(value)", "floor(0)", "Round down"),
                    ("ceil", "ceil(value)", "ceil(0)", "Round up"),
                    ("abs", "abs(value)", "abs(0)", "Absolute value"),
                    ("sqrt", "sqrt(value)", "sqrt(0)", "Square root"),
                    ("pow", "pow(base, power)", "pow(0, 2)", "Exponent"),
                    (
                        "if",
                        "if(condition, yes, no)",
                        "if(true, 1, 0)",
                        "Conditional value",
                    ),
                    (
                        "lerp",
                        "lerp(start, end, amount)",
                        "lerp(0, 100, 0.5)",
                        "Linear interpolation",
                    ),
                ] {
                    if !needle.is_empty()
                        && !name.contains(&needle)
                        && !detail.to_ascii_lowercase().contains(&needle)
                    {
                        continue;
                    }
                    if ui
                        .add(
                            egui::Button::selectable(false, signature)
                                .frame(false)
                                .wrap(),
                        )
                        .on_hover_text(language.text(detail))
                        .clicked()
                    {
                        append_expression_token(draft, insertion);
                    }
                }
            });
    });
}

pub(super) fn expression_operators_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    draft: &mut String,
    language: LanguageId,
) {
    expression_reference_card(ui, width, height, language.text("Operators"), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("expression-operators")
            .auto_shrink([false, false])
            .max_height((height - 38.0).max(80.0))
            .show(ui, |ui| {
                for (operator, insertion, detail) in [
                    ("&&", "&&", "And"),
                    ("||", "||", "Or"),
                    ("!", "!", "Not"),
                    ("==", "==", "Equal"),
                    ("!=", "!=", "Not equal"),
                    (">", ">", "Greater than"),
                    ("<", "<", "Less than"),
                    (">=", ">=", "Greater or equal"),
                    ("<=", "<=", "Less or equal"),
                    ("+", "+", "Add"),
                    ("-", "-", "Subtract"),
                    ("*", "*", "Multiply"),
                    ("/", "/", "Divide"),
                    ("%", "%", "Remainder"),
                    ("( )", "()", "Grouping"),
                ] {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [44.0, CONTROL_HEIGHT],
                                egui::Button::new(
                                    egui::RichText::new(operator)
                                        .family(egui::FontFamily::Monospace),
                                ),
                            )
                            .on_hover_text(language.text("Insert operator"))
                            .clicked()
                        {
                            append_expression_token(draft, insertion);
                        }
                        ui.label(
                            egui::RichText::new(language.text(detail))
                                .small()
                                .color(MUTED),
                        );
                    });
                }
            });
    });
}
