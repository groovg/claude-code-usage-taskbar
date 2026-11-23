use super::*;

impl StudioApp {
    pub(super) fn context_menus_page(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let descriptors = self.context_menus();
        let read_only = self.context_menu_read_only();
        let mut requested_path = None;
        let mut create_new = false;
        let mut duplicate = false;
        let mut save = false;
        let mut discard = false;
        let mut delete = false;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(language.text("Context menu")).color(MUTED));
            Dropdown::from_id_salt("context-menu-library-v2")
                .width(260.0)
                .selected_text(if read_only {
                    format!("{} ({})", self.context_menu.name, language.text("built-in"))
                } else {
                    self.context_menu.name.clone()
                })
                .show_ui(ui, |ui| {
                    for descriptor in &descriptors {
                        let selected =
                            self.context_menu_path.as_deref() == Some(descriptor.path.as_path());
                        let label = if descriptor.built_in {
                            format!("{} ({})", descriptor.name, language.text("built-in"))
                        } else {
                            descriptor.name.clone()
                        };
                        if ui
                            .add_enabled(
                                !self.context_menu_dirty,
                                egui::Button::selectable(selected, label),
                            )
                            .on_disabled_hover_text(
                                language.text("Save or discard the current menu before switching"),
                            )
                            .clicked()
                        {
                            requested_path = Some(descriptor.path.clone());
                        }
                    }
                });
            create_new = ui
                .add_enabled(
                    !self.context_menu_dirty,
                    lucide_icon_button(LucideIcon::FilePlus),
                )
                .on_hover_text(language.text("New"))
                .clicked();
            duplicate = ui
                .add(lucide_icon_button(LucideIcon::Copy))
                .on_hover_text(language.text("Duplicate..."))
                .clicked();
            save = ui
                .add_enabled(
                    !read_only && self.context_menu_dirty,
                    lucide_icon_button(LucideIcon::Save),
                )
                .on_hover_text(language.text("Save"))
                .clicked();
            discard = ui
                .add_enabled(self.context_menu_dirty, lucide_icon_button(LucideIcon::X))
                .on_hover_text(language.text("Discard changes"))
                .clicked();
            delete = ui
                .add_enabled(
                    !read_only && !self.context_menu_dirty && self.context_menu_path.is_some(),
                    lucide_icon_button(LucideIcon::Trash),
                )
                .on_hover_text(language.text("Delete..."))
                .clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !read_only,
                        singleline_text_edit(&mut self.context_menu.name).desired_width(170.0),
                    )
                    .changed()
                {
                    self.context_menu_dirty = true;
                }
                ui.label(language.text("Name"));
                if ui
                    .add_enabled(
                        !read_only,
                        singleline_text_edit(&mut self.context_menu.id).desired_width(145.0),
                    )
                    .changed()
                {
                    self.context_menu_dirty = true;
                }
                ui.label("ID");
                if self.context_menu_dirty {
                    ui.colored_label(
                        egui::Color32::from_rgb(232, 188, 95),
                        language.text("Unsaved changes"),
                    );
                }
            });
        });

        if let Some(path) = requested_path {
            match context_menu::load_context_menu(&path) {
                Ok(document) => {
                    self.context_menu = document;
                    self.context_menu_path = Some(path);
                    self.context_menu_dirty = false;
                    self.reset_context_menu_editor();
                }
                Err(error) => self.theme_error = Some(error),
            }
        }
        if create_new {
            self.context_menu = ContextMenuDocument::blank("New Context Menu");
            self.start_unsaved_context_menu();
            self.reset_context_menu_editor();
            self.context_menu_selection = Some(vec![0]);
        }
        if duplicate {
            let name = format!("{} ({})", self.context_menu.name, language.text("copy"));
            self.context_menu.id = context_menu::unique_document_id(&name);
            self.context_menu.name = name;
            self.start_unsaved_context_menu();
        }
        if discard {
            self.discard_context_menu_changes();
        }
        if delete {
            if let Some(path) = self.context_menu_path.clone() {
                self.delete_context_menu_confirmation =
                    Some((path, self.context_menu.name.clone()));
            }
        }
        self.delete_context_menu_dialog(ui.ctx());

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        if let Some(helper) = self.context_menu_text_template_helper.take() {
            self.context_menu_text_template_helper = self.text_template_helper_ui(ui, helper);
        } else if let Some(helper) = self.context_menu_expression_helper.take() {
            self.context_menu_expression_helper = self.expression_helper_ui(ui, helper);
        } else if self.context_menu_action_helper.is_some() {
            self.context_menu_action_helper_ui(ui);
        } else {
            let editor_identity = self.context_menu.id.clone();
            let read_only = self.context_menu_read_only();
            ui.push_id(("context-menu-document", editor_identity), |ui| {
                self.context_menu_workspace(ui, read_only);
            });
        }

        // Persist only after all editor controls have processed this frame.
        // Clicking Save moves focus, which can commit a final text edit and mark
        // the document dirty; saving earlier would immediately re-lock the menu
        // selector even though the user had just saved.
        if save {
            self.save_context_menu_document();
        }
    }

    /// Marks the open menu as a new document that has no file yet. Discard
    /// returns to the file it came from.
    pub(super) fn start_unsaved_context_menu(&mut self) {
        if let Some(path) = self.context_menu_path.take() {
            self.context_menu_discard_path = Some(path);
        }
        self.context_menu_dirty = true;
    }

    /// Reloads the open menu from its file, or the file a new copy came from.
    pub(super) fn discard_context_menu_changes(&mut self) {
        let path = self
            .context_menu_path
            .clone()
            .or_else(|| self.context_menu_discard_path.clone());
        match path
            .as_deref()
            .and_then(|path| context_menu::load_context_menu(path).ok())
        {
            Some(document) => {
                self.context_menu = document;
                self.context_menu_path = path;
            }
            None => self.context_menu = context_menu::classic_context_menu(),
        }
        self.context_menu_dirty = false;
        self.reset_context_menu_editor();
    }

    /// Saves the open menu and removes the file it replaced. Returns whether it saved.
    pub(super) fn save_context_menu_document(&mut self) -> bool {
        let language = self.language();
        let descriptors = context_menu::list_context_menus().unwrap_or_default();
        let duplicate_id = descriptors.iter().any(|descriptor| {
            descriptor
                .id
                .eq_ignore_ascii_case(self.context_menu.id.trim())
                && self.context_menu_path.as_deref() != Some(descriptor.path.as_path())
        });
        if duplicate_id {
            self.theme_error = Some(format!(
                "{} '{}'",
                language.text("Another context menu already uses the ID"),
                self.context_menu.id
            ));
            return false;
        }
        let previous_path = self.context_menu_path.clone();
        match context_menu::save_context_menu(&self.context_menu) {
            Ok(path) => {
                if let Some(previous) = previous_path.as_deref() {
                    if let Err(error) = remove_replaced_context_menu(previous, &path) {
                        self.theme_error = Some(format!(
                            "{}: {error}",
                            language
                                .text("The menu was saved, but its old file could not be removed")
                        ));
                    }
                }
                self.context_menu_path = Some(path);
                self.context_menu_dirty = false;
                self.refresh_library();
                self.notify_owner();
                true
            }
            Err(error) => {
                self.theme_error = Some(error);
                false
            }
        }
    }

    pub(super) fn context_menu_workspace(&mut self, ui: &mut egui::Ui, read_only: bool) {
        self.three_pane_workspace(
            ui,
            [
                "context-menu-layers-pane",
                "context-menu-left-splitter",
                "context-menu-preview-pane",
                "context-menu-right-splitter",
                "context-menu-inspector-pane",
            ],
            |app, ui, pane| match pane {
                0 => app.context_menu_layers(ui, read_only),
                1 => app.context_menu_preview(ui),
                _ => app.context_menu_inspector(ui, read_only),
            },
        );
    }

    pub(super) fn context_menu_layers(&mut self, ui: &mut egui::Ui, read_only: bool) {
        let language = self.language();
        let mut add_kind = None;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(language.text("Layers"))
                    .size(16.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!read_only, |ui| {
                    ui.menu_button(
                        (
                            crate::ui::components::icon::icon_text(LucideIcon::Plus, 16.0),
                            language.text("Add layer"),
                        ),
                        |ui| {
                            for (kind, icon, label) in [
                                ("action", LucideIcon::MousePointerClick, "Action"),
                                ("text", LucideIcon::Type, "Text"),
                                ("submenu", LucideIcon::Layers, "Group"),
                                ("separator", LucideIcon::Minus, "Separator"),
                            ] {
                                if ui
                                    .button((
                                        crate::ui::components::icon::icon_text(icon, 16.0),
                                        language.text(label),
                                    ))
                                    .clicked()
                                {
                                    add_kind = Some(kind);
                                    ui.close();
                                }
                            }
                        },
                    );
                });
            });
        });
        if let Some(kind) = add_kind {
            let item = match kind {
                "text" => ContextMenuItem::text(
                    &next_context_menu_item_id(&self.context_menu.items, "text"),
                    "New text",
                ),
                "submenu" => ContextMenuItem::submenu(
                    &next_context_menu_item_id(&self.context_menu.items, "group"),
                    "New group",
                    vec![],
                ),
                "separator" => ContextMenuItem::separator(&next_context_menu_item_id(
                    &self.context_menu.items,
                    "separator",
                )),
                _ => ContextMenuItem::action(
                    &next_context_menu_item_id(&self.context_menu.items, "action"),
                    "New action",
                    ContextMenuAction::OpenDashboard,
                ),
            };
            let path = add_context_menu_item(
                &mut self.context_menu.items,
                self.context_menu_selection.as_deref(),
                item,
            );
            self.context_menu_selection = Some(path);
            self.context_menu_dirty = true;
        }
        ui.add_space(6.0);
        let footer_height = CONTROL_HEIGHT + ui.spacing().item_spacing.y * 3.0 + 1.0;
        let tree_height = (ui.available_height() - footer_height).max(80.0);
        let mut pending_drop = None;
        egui::ScrollArea::vertical()
            .id_salt("context-menu-layers")
            .auto_shrink([false, false])
            .content_margin(egui::Margin {
                right: 12,
                ..egui::Margin::ZERO
            })
            .max_height(tree_height)
            .show(ui, |ui| {
                for index in 0..self.context_menu.items.len() {
                    let path = vec![index];
                    if pending_drop.is_none() {
                        pending_drop = self.context_menu_tree_row(ui, path, 0, read_only);
                    } else {
                        self.context_menu_tree_row(ui, path, 0, read_only);
                    }
                    ui.add_space(4.0);
                }
            });
        if !read_only {
            if let Some((source, target)) = pending_drop {
                if move_context_menu_item_to(
                    &mut self.context_menu.items,
                    &source,
                    &target,
                    &mut self.context_menu_selection,
                ) {
                    self.context_menu_dirty = true;
                }
            }
        }
        ui.separator();
        let footer_width = (ui.clip_rect().right() - ui.cursor().min.x).max(1.0);
        let button_width = ((footer_width - ui.spacing().item_spacing.x) / 2.0).max(1.0);
        let selected = self.context_menu_selection.clone();
        ui.horizontal(|ui| {
            if ui
                .add_enabled_ui(!read_only && selected.is_some(), |ui| {
                    ui.add_sized(
                        [button_width, CONTROL_HEIGHT],
                        lucide_labeled_button(LucideIcon::Copy, language.text("Duplicate")),
                    )
                })
                .inner
                .clicked()
            {
                if let Some(path) = selected.as_deref() {
                    if let Some(new_path) =
                        duplicate_context_menu_item(&mut self.context_menu.items, path)
                    {
                        self.context_menu_selection = Some(new_path);
                        self.context_menu_dirty = true;
                    }
                }
            }
            if ui
                .add_enabled_ui(!read_only && selected.is_some(), |ui| {
                    ui.add_sized(
                        [button_width, CONTROL_HEIGHT],
                        lucide_labeled_button(LucideIcon::Trash, language.text("Delete")),
                    )
                })
                .inner
                .clicked()
            {
                if let Some(path) = selected.as_deref() {
                    remove_context_menu_item(&mut self.context_menu.items, path);
                    self.context_menu_selection = None;
                    self.context_menu_dirty = true;
                }
            }
        });
    }

    pub(super) fn context_menu_tree_row(
        &mut self,
        ui: &mut egui::Ui,
        path: Vec<usize>,
        depth: usize,
        read_only: bool,
    ) -> Option<(Vec<usize>, ContextMenuDropTarget)> {
        let language = self.language();
        let mut item = context_menu_item(&self.context_menu.items, &path)?.clone();
        let child_count = match &item.kind {
            ContextMenuItemKind::Submenu { items } => items.len(),
            _ => 0,
        };
        let node_id = matches!(&item.kind, ContextMenuItemKind::Submenu { .. })
            .then(|| ui.make_persistent_id(("context-menu-layer", item.id.clone())));
        let is_open = node_id.map(|id| {
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                depth == 0,
            )
            .is_open()
        });
        let selected = self.context_menu_selection.as_ref() == Some(&path);
        let editable_name = !read_only && !matches!(&item.kind, ContextMenuItemKind::Separator);
        paint_scene_row_background(ui, selected);
        let responses = scene_row_style(ui, selected, |ui| {
            context_menu_tree_row_contents(
                ui,
                &mut item,
                &path,
                is_open,
                editable_name,
                !read_only,
                language,
            )
        });
        if responses.name_changed {
            if let Some(target) = context_menu_item_mut(&mut self.context_menu.items, &path) {
                target.label = item.label;
                self.context_menu_dirty = true;
            }
        }
        if responses.item.clicked() || responses.drag_handle.drag_started() {
            self.context_menu_selection = Some(path.clone());
        }
        let mut pending_drop = if read_only {
            None
        } else {
            context_menu_drop_from_response(
                ui,
                &responses.item,
                &path,
                matches!(&item.kind, ContextMenuItemKind::Submenu { .. }),
            )
        };
        if let Some(id) = node_id {
            if responses.expand_button.clicked() {
                toggle_scene_node(ui, id, depth == 0);
            }
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                depth == 0,
            );
            if state.is_open() && child_count > 0 {
                state.show_body_indented(&responses.expand_button, ui, |ui| {
                    for index in 0..child_count {
                        let mut child_path = path.clone();
                        child_path.push(index);
                        if pending_drop.is_none() {
                            pending_drop =
                                self.context_menu_tree_row(ui, child_path, depth + 1, read_only);
                        } else {
                            self.context_menu_tree_row(ui, child_path, depth + 1, read_only);
                        }
                    }
                });
            }
        }
        pending_drop
    }

    pub(super) fn context_menu_preview(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(language.text("Preview"))
                    .size(16.0)
                    .strong(),
            );
        });
        ui.add_space(10.0);
        let runtime = self.selected_theme_runtime();
        let context =
            DataContext::from_usage_with_runtime(self.usage.as_ref(), &Canvas::default(), runtime);
        let effective_theme =
            theme_engine::apply_mouse_action_overrides(&self.theme, &self.preview_mouse_overrides);
        let items = self.context_menu.items.clone();
        let appearance = NativeContextMenuAppearance::detect(ui.ctx().pixels_per_point());
        let available = ui.available_size();
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(13, 14, 17));
        let menu_width = native_context_menu_width(ui, &items, language, &context, &appearance)
            .min((rect.width() - 40.0).max(1.0));
        let menu_rect = egui::Rect::from_min_size(
            egui::pos2(rect.center().x - menu_width / 2.0, rect.top() + 30.0),
            egui::vec2(menu_width, rect.height() - 60.0),
        );
        let mut menu_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("context-menu-live-preview")
                .max_rect(menu_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        apply_native_context_menu_style(&mut menu_ui, &appearance);
        let preview_bounds = std::cell::Cell::new(egui::Rect::NOTHING);
        let open_submenu_state_ids = std::cell::RefCell::new(Vec::new());
        let mut preview_state = ContextMenuPreviewState {
            language,
            context: &context,
            settings: &self.settings,
            startup_enabled: self.startup_enabled,
            theme: &effective_theme,
            usage: self.usage.as_ref(),
            runtime,
            appearance: &appearance,
            preview_bounds: &preview_bounds,
            open_submenu_state_ids: &open_submenu_state_ids,
        };
        let root_menu = egui::Frame::menu(menu_ui.style()).show(&mut menu_ui, |ui| {
            ui.set_min_width((menu_width - appearance.frame_inset * 2.0).max(1.0));
            preview_context_menu_items(ui, &items, &mut preview_state);
        });
        preview_bounds.set(preview_bounds.get().union(root_menu.response.rect));
        collapse_context_menu_preview_on_outside_click(
            ui.ctx(),
            preview_bounds.get(),
            &open_submenu_state_ids.borrow(),
        );
    }

    pub(super) fn context_menu_inspector(&mut self, ui: &mut egui::Ui, read_only: bool) {
        egui::ScrollArea::vertical()
            .id_salt("context-menu-inspector")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.context_menu_item_inspector(ui, read_only) && !read_only {
                    self.context_menu_dirty = true;
                }
            });
    }

    pub(super) fn delete_context_menu_dialog(&mut self, context: &egui::Context) {
        let Some((path, name)) = self.delete_context_menu_confirmation.take() else {
            return;
        };
        let decision = confirm_delete(
            context,
            self.language(),
            "Delete context menu?",
            "delete-context-menu-dialog",
            "Are you sure you want to delete {name}?",
            &name,
            "Delete context menu",
        );
        match decision {
            Some(true) => match context_menu::delete_context_menu(&path) {
                Ok(()) => {
                    self.context_menu = context_menu::classic_context_menu();
                    self.context_menu_path = context_menu::ensure_builtin_context_menus().ok();
                    self.context_menu_dirty = false;
                    self.reset_context_menu_editor();
                    self.refresh_library();
                }
                Err(error) => self.theme_error = Some(error),
            },
            Some(false) => {}
            None => self.delete_context_menu_confirmation = Some((path, name)),
        }
    }

    pub(super) fn context_menu_item_inspector(
        &mut self,
        ui: &mut egui::Ui,
        read_only: bool,
    ) -> bool {
        let language = self.language();
        let Some(path) = self.context_menu_selection.clone() else {
            ui.label(language.text("Select a menu item to edit it."));
            return false;
        };
        let Some(mut item) = context_menu_item(&self.context_menu.items, &path).cloned() else {
            self.context_menu_selection = None;
            return false;
        };
        let before = serde_json::to_string(&item).unwrap_or_default();
        let mut changed = false;
        let mut open_label_helper = false;
        let mut open_action_helper = false;
        let mut open_expression_helper = false;
        let label_context = DataContext::from_usage_with_runtime(
            self.usage.as_ref(),
            &Canvas::default(),
            self.selected_theme_runtime(),
        );

        // Key widget state by the item's id, not its position, so a backup or a
        // half-typed id does not move to the item that takes this position.
        let item_identity = item.id.clone();
        ui.push_id(("context-menu-item", item_identity), |ui| {
            if read_only {
                inspector_heading(ui, &format!("ID: {}", item.id));
            } else {
                inspector_name_editor(
                    ui,
                    &mut item.id,
                    ui.make_persistent_id("context-menu-item-id"),
                    "ID: ",
                    language,
                );
            }
            ui.add_space(6.0);
            egui::CollapsingHeader::new(language.text("Item"))
                .id_salt(ui.make_persistent_id("context-menu-item-section"))
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_enabled_ui(!read_only, |ui| {
                        open_expression_helper = expression_control(
                            ui,
                            ui.make_persistent_id("context-menu-render"),
                            language.text("Render"),
                            &mut item.render,
                            ExpressionControlKind::Boolean,
                            &label_context,
                            language,
                        );
                    });
                    if !matches!(&item.kind, ContextMenuItemKind::Separator) {
                        labeled(ui, language.text("Label"), |ui| {
                            ui.add_enabled_ui(!read_only, |ui| {
                                open_label_helper = text_template_editor_control(
                                    ui,
                                    ui.make_persistent_id("context-menu-label"),
                                    &mut item.label,
                                    &label_context,
                                    inspector_control_width(ui),
                                    language,
                                );
                            });
                        });
                        if matches!(&item.kind, ContextMenuItemKind::Text) {
                            ui.label(
                                egui::RichText::new(
                                    language.text(
                                        "Text items are informational and cannot be clicked.",
                                    ),
                                )
                                .small()
                                .color(MUTED),
                            );
                        }
                    }
                    if let ContextMenuItemKind::Action { action } = &item.kind {
                        let preview = context_menu_action_script(action);
                        labeled(ui, language.text("Action"), |ui| {
                            ui.add_enabled_ui(!read_only, |ui| {
                                let helper_action = helper_preview_field(
                                    ui,
                                    ui.make_persistent_id("context-menu-action"),
                                    &preview,
                                    inspector_control_width(ui),
                                    true,
                                    language.text("Open action helper"),
                                    egui::Align::Min,
                                    language,
                                );
                                open_action_helper = helper_action.open;
                            });
                        });
                    }
                });
        });

        changed |= before != serde_json::to_string(&item).unwrap_or_default();
        if changed {
            if let Some(target) = context_menu_item_mut(&mut self.context_menu.items, &path) {
                *target = item;
            }
        }
        if open_expression_helper {
            if let Some(item) = context_menu_item(&self.context_menu.items, &path) {
                self.context_menu_expression_helper = Some(
                    ExpressionHelperState::for_context_menu(path.clone(), item.render.0.clone()),
                );
            }
        }
        if open_label_helper {
            if let Some(item) = context_menu_item(&self.context_menu.items, &path) {
                self.context_menu_text_template_helper = Some(
                    TextTemplateHelperState::for_context_menu(path.clone(), item.label.clone()),
                );
            }
        }
        if open_action_helper {
            if let Some(ContextMenuItem {
                kind: ContextMenuItemKind::Action { action },
                ..
            }) = context_menu_item(&self.context_menu.items, &path)
            {
                self.context_menu_action_helper =
                    Some(ContextMenuActionHelperState::new(path.clone(), action));
            }
        }
        changed
    }
}

/// Deletes a menu's old file after a save wrote `saved`, unless both paths
/// name the same file: saving `Work.json` writes `work.json`, the same file on
/// Windows, and deleting it would lose the menu.
pub(super) fn remove_replaced_context_menu(previous: &Path, saved: &Path) -> Result<(), String> {
    let same_file = match (
        std::fs::canonicalize(previous),
        std::fs::canonicalize(saved),
    ) {
        (Ok(previous), Ok(saved)) => previous == saved,
        _ => previous == saved,
    };
    if same_file {
        Ok(())
    } else {
        context_menu::delete_context_menu(previous)
    }
}
