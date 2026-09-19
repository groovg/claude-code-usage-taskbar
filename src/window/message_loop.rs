use super::*;

/// Main window procedure
pub(super) unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
            refresh_theme_host_geometry();
            if msg == WM_SETTINGCHANGE {
                check_theme_change();
                check_language_change();
            }
            position_at_taskbar();
            render_layered();
            sync_tray_icon(hwnd);
            LRESULT(0)
        }
        WM_TIMER => {
            let timer_id = wparam.0;
            match timer_id {
                TIMER_POLL => {
                    let auth_watch = {
                        let state = lock_state();
                        state.as_ref().map(|s| {
                            (
                                s.auth_error_paused_polling,
                                s.auth_watch_mode,
                                s.auth_watch_snapshot.clone(),
                            )
                        })
                    };
                    match auth_watch {
                        Some((true, watch_mode, previous_snapshot)) => {
                            let current_snapshot = poller::credential_watch_snapshot(watch_mode);
                            if current_snapshot != previous_snapshot {
                                let mut state = lock_state();
                                if let Some(s) = state.as_mut() {
                                    if s.auth_error_paused_polling
                                        && s.auth_watch_mode == watch_mode
                                    {
                                        s.auth_watch_snapshot = current_snapshot;
                                    }
                                }
                                drop(state);
                                request_poll(hwnd);
                            }
                        }
                        Some((false, _, _)) => {
                            request_scheduled_poll(hwnd);
                        }
                        None => {}
                    }
                }
                TIMER_COUNTDOWN => {
                    render_layered();
                    sync_tray_icon(hwnd);
                    schedule_countdown_timer();
                }
                TIMER_CONTEXT if refresh_session_context() => {
                    render_layered();
                    sync_tray_icon(hwnd);
                }
                TIMER_CLOCK => {
                    render_layered();
                    let refresh_tray = lock_state()
                        .as_ref()
                        .is_some_and(|state| state.tray_theme_uses_current_time);
                    if refresh_tray {
                        sync_tray_icon(hwnd);
                    }
                    schedule_clock_timer();
                }
                TIMER_RESET_POLL => {
                    let should_poll = {
                        let state = lock_state();
                        state
                            .as_ref()
                            .map(|s| !s.auth_error_paused_polling)
                            .unwrap_or(false)
                    };
                    if should_poll {
                        request_scheduled_poll(hwnd);
                    }
                }
                TIMER_UPDATE_CHECK => {
                    begin_update_check(hwnd, false);
                }
                TIMER_WINDOW_STATE => {
                    sync_theme_window_visibility();
                }
                TIMER_MOUSE_CLICK => {
                    let _ = KillTimer(Some(hwnd), TIMER_MOUSE_CLICK);
                    let pending = lock_state()
                        .as_mut()
                        .and_then(|state| state.pending_mouse_click.take());
                    if let Some(pending) = pending {
                        let _ = dispatch_mouse_event(
                            pending.surface_index,
                            &pending.object_id,
                            MouseEventKind::Click,
                        );
                    }
                }
                TIMER_TRAY_HOVER => {
                    clear_tray_mouse_hover_if_left(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_APP_USAGE_UPDATED => {
            check_theme_change();
            check_language_change();
            render_layered();
            schedule_countdown_timer();
            schedule_clock_timer();
            sync_tray_icon(hwnd);
            LRESULT(0)
        }
        WM_APP_SETTINGS_UPDATED => {
            reload_external_settings(hwnd);
            LRESULT(0)
        }
        WM_APP_REFRESH_NOW => {
            if let Some(state) = lock_state().as_mut() {
                state.force_notify_auth_error = true;
            }
            request_poll(hwnd);
            LRESULT(0)
        }
        WM_APP_OPEN_DASHBOARD => {
            crate::dashboard::show(hwnd);
            LRESULT(0)
        }
        WM_APP_UPDATE_CHECK_COMPLETE => {
            schedule_auto_update_check(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 as u16;
            match id {
                IDM_DASHBOARD => {
                    crate::dashboard::show(hwnd);
                }
                1 => {
                    {
                        let mut state = lock_state();
                        if let Some(s) = state.as_mut() {
                            s.force_notify_auth_error = true;
                        }
                    }
                    render_layered();
                    request_poll(hwnd);
                }
                IDM_VERSION_ACTION => {
                    let (install_channel, release) = {
                        let state = lock_state();
                        match state.as_ref() {
                            Some(s) => (
                                s.install_channel,
                                match &s.update_status {
                                    UpdateStatus::Available(release) => Some(release.clone()),
                                    _ => None,
                                },
                            ),
                            None => (InstallChannel::Portable, None),
                        }
                    };

                    match install_channel {
                        InstallChannel::Winget => {
                            if release.is_some() {
                                begin_winget_update(hwnd);
                            } else {
                                begin_update_check(hwnd, true);
                            }
                        }
                        InstallChannel::Portable => {
                            if let Some(release) = release {
                                begin_update_apply(hwnd, release);
                            } else {
                                begin_update_check(hwnd, true);
                            }
                        }
                    }
                }
                2 => {
                    crate::dashboard::close_existing();
                    let _ = DestroyWindow(hwnd);
                }
                IDM_START_WITH_WINDOWS => {
                    set_startup_enabled(!is_startup_enabled());
                }
                IDM_FREQ_1MIN | IDM_FREQ_5MIN | IDM_FREQ_15MIN | IDM_FREQ_1HOUR => {
                    let new_interval = match id {
                        IDM_FREQ_1MIN => POLL_1_MIN,
                        IDM_FREQ_5MIN => POLL_5_MIN,
                        IDM_FREQ_15MIN => POLL_15_MIN,
                        IDM_FREQ_1HOUR => POLL_1_HOUR,
                        _ => POLL_15_MIN,
                    };
                    {
                        let mut state = lock_state();
                        if let Some(s) = state.as_mut() {
                            s.poll_interval_ms = new_interval;
                        }
                    }
                    save_state_settings();
                    // Reset the poll timer with the new interval
                    SetTimer(Some(hwnd), TIMER_POLL, new_interval, None);
                }
                id if ProviderId::from_native_menu_command_id(id).is_some() => {
                    {
                        let mut state = lock_state();
                        if let Some(s) = state.as_mut() {
                            let provider = ProviderId::from_native_menu_command_id(id)
                                .expect("provider menu command was matched above");
                            s.providers.toggle(provider);
                        }
                    }
                    save_state_settings();
                    position_at_taskbar();
                    render_layered();
                    sync_tray_icon(hwnd);
                    request_poll(hwnd);
                }
                id if id == IDM_LANG_SYSTEM || language_from_menu_command_id(id).is_some() => {
                    let language_override = if id == IDM_LANG_SYSTEM {
                        None
                    } else {
                        language_from_menu_command_id(id)
                    };
                    {
                        let mut state = lock_state();
                        if let Some(s) = state.as_mut() {
                            apply_language_to_state(s, language_override);
                        }
                    }
                    save_state_settings();
                    render_layered();
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ if msg == WM_APP_TRAY => {
            // Explorer can deliver this synchronously, including while a shell
            // call has re-entered our window procedure. Return before taking
            // STATE, opening windows, or calling back into Explorer.
            if let Err(error) = PostMessageW(
                Some(hwnd),
                native_interop::WM_APP_TRAY_DISPATCH,
                wparam,
                lparam,
            ) {
                diagnose::log_error("unable to queue tray callback", error);
            }
            LRESULT(0)
        }
        _ if msg == native_interop::WM_APP_TRAY_DISPATCH => {
            let tray_message = lparam.0 as u32;
            if let Some(surface_index) = tray_icon::themed_surface_index(wparam.0 as u32) {
                let root_id = lock_state().as_ref().and_then(|state| {
                    state
                        .active_theme
                        .surfaces
                        .get(surface_index)
                        .map(|surface| surface.id.clone())
                });
                if let Some(root_id) = root_id {
                    match tray_message {
                        WM_MOUSEMOVE => {
                            update_tray_mouse_hover(hwnd, surface_index, root_id);
                            return LRESULT(0);
                        }
                        WM_LBUTTONUP => {
                            if take_suppressed_left_up() {
                                return LRESULT(0);
                            }
                            if mouse_handler_exists(surface_index, &root_id, MouseEventKind::Click)
                            {
                                schedule_or_dispatch_click(surface_index, root_id);
                            } else if !mouse_handler_exists(
                                surface_index,
                                &root_id,
                                MouseEventKind::DoubleClick,
                            ) {
                                crate::dashboard::show(hwnd);
                            }
                            return LRESULT(0);
                        }
                        WM_LBUTTONDBLCLK => {
                            if mouse_handler_exists(
                                surface_index,
                                &root_id,
                                MouseEventKind::DoubleClick,
                            ) {
                                dispatch_double_click(surface_index, root_id);
                            } else {
                                crate::dashboard::show(hwnd);
                            }
                            return LRESULT(0);
                        }
                        WM_RBUTTONUP | WM_CONTEXTMENU => {
                            if !dispatch_mouse_event(
                                surface_index,
                                &root_id,
                                MouseEventKind::RightClick,
                            ) {
                                show_context_menu_document(hwnd, None, None);
                            }
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
            match tray_message {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => crate::dashboard::show(hwnd),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_context_menu_document(hwnd, None, None),
                _ => {}
            }
            LRESULT(0)
        }
        _ if msg == taskbar_created_message() => {
            refresh_theme_host_geometry();
            // Explorer discards notification icons when it restarts. Floating
            // and tray-icon-only themes keep their owner HWND, so restore the
            // registrations when the shell broadcasts its return.
            sync_tray_icon(hwnd);
            render_layered();
            LRESULT(0)
        }
        WM_DESTROY => {
            crate::dashboard::close_existing();
            crate::desktop_compositor::clear();
            let desktop_windows = lock_state()
                .as_mut()
                .map(|state| std::mem::take(&mut state.desktop_hwnds))
                .unwrap_or_default();
            for window in desktop_windows.into_iter().flatten() {
                let _ = DestroyWindow(window.to_hwnd());
            }
            tray_icon::remove_all(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        // Painting, hit testing and mouse input work as on the other surfaces.
        _ => mirror_wnd_proc(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_callbacks_return_while_state_is_locked_and_preserve_events() {
        // Model a shell call re-entering wnd_proc while the monitor owns STATE.
        // Keep the lock on this thread so a regression fails with a timeout
        // instead of permanently deadlocking the test process.
        let state = lock_state();
        let (completed, completion) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR::null(),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .expect("create isolated message-only test window");

            // Start with a themed hover: the old handler tries to acquire STATE
            // here. No dashboard or menu should ever be opened by this test.
            let events = [
                (1_000, WM_MOUSEMOVE),
                (1_042, WM_LBUTTONUP),
                (1_042, WM_LBUTTONDBLCLK),
                (1_042, WM_RBUTTONUP),
                (1, WM_LBUTTONUP),
                (1, WM_LBUTTONDBLCLK),
                (1, WM_RBUTTONUP),
                (1, WM_CONTEXTMENU),
            ];
            for (id, event) in events {
                assert_eq!(
                    wnd_proc(hwnd, WM_APP_TRAY, WPARAM(id), LPARAM(event as isize)).0,
                    0
                );
                let mut queued = MSG::default();
                let found = PeekMessageW(
                    &mut queued,
                    Some(hwnd),
                    native_interop::WM_APP_TRAY_DISPATCH,
                    native_interop::WM_APP_TRAY_DISPATCH,
                    PM_REMOVE,
                )
                .as_bool();
                if !found {
                    let _ = DestroyWindow(hwnd);
                    panic!("tray callback was processed inline instead of queued");
                }
                assert_eq!(queued.hwnd, hwnd);
                assert_eq!(queued.wParam.0, id);
                assert_eq!(queued.lParam.0, event as isize);
            }
            let _ = DestroyWindow(hwnd);
            completed.send(()).unwrap();
        });

        let result = completion.recv_timeout(Duration::from_secs(5));
        drop(state);
        worker.join().expect("tray callback test thread");
        result.expect("tray callbacks must return without waiting for STATE");
    }
}
