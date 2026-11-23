use super::*;

#[test]
fn window_state_timer_is_only_needed_for_floating_surfaces() {
    let mut theme = ThemeDocument::starter();
    assert!(!theme_has_floating_surface(&theme));

    theme.surfaces[0].placement.nest = SurfaceNest::Floating;
    assert!(theme_has_floating_surface(&theme));
}

#[test]
fn center_points_keep_a_surface_centered_as_it_resizes() {
    assert_eq!(aligned_origin(0, 1920, 300, 0.5, 0.5, 0), 810);
    assert_eq!(aligned_origin(0, 1920, 500, 0.5, 0.5, 0), 710);
}

#[test]
fn reference_left_to_surface_right_places_widget_before_reference() {
    assert_eq!(aligned_origin(1600, 320, 300, 0.0, 1.0, 0), 1300);
}

#[test]
fn negative_offsets_inset_right_bottom_anchored_desktop_surfaces() {
    assert_eq!(aligned_origin(0, 3440, 198, 1.0, 1.0, -26), 3216);
    assert_eq!(aligned_origin(0, 1440, 144, 1.0, 1.0, -54), 1242);
}

#[test]
fn theme_dimensions_scale_from_logical_to_physical_pixels() {
    assert_eq!(scaled_theme_dimension(217, 1.0), 217);
    assert_eq!(scaled_theme_dimension(217, 1.25), 271);
    assert_eq!(scaled_theme_dimension(217, 1.5), 326);
    assert_eq!(scaled_theme_dimension(46, 2.0), 92);
}

#[test]
fn physical_host_dimensions_are_normalized_to_logical_pixels() {
    assert_eq!(logical_host_dimension(38, 1.25), 30);
    assert_eq!(logical_host_dimension(46, 1.0), 46);
    assert_eq!(logical_host_dimension(92, 2.0), 46);
    assert_eq!(logical_host_dimension(30, 0.0), 30);
}

#[test]
fn tray_widget_action_targets_a_custom_theme_root_without_a_main_id() {
    let mut theme = ThemeDocument::starter();
    theme.surfaces[0].id = "layer-62744-2".into();
    let (surface_index, root_id) = context_menu_widget_origin(&theme).unwrap();
    assert_eq!((surface_index, root_id.as_str()), (0, "layer-62744-2"));

    let runtime = ThemeRuntime::new(true, true, true);
    let mut overrides = HashMap::new();
    theme_engine::execute_mouse_actions(
        &theme,
        surface_index,
        &root_id,
        "toggle(self, render)",
        None,
        runtime,
        &mut overrides,
    )
    .unwrap();
    let hidden = theme_engine::apply_mouse_action_overrides(&theme, &overrides);
    assert!(!theme_engine::surface_should_render(
        &hidden,
        surface_index,
        None,
        runtime
    ));
}

#[test]
fn fullscreen_bounds_cover_the_monitor_but_maximized_work_area_does_not() {
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    assert!(rect_covers_monitor(monitor, monitor));
    assert!(rect_covers_monitor(
        RECT {
            left: -2,
            top: -2,
            right: 1922,
            bottom: 1082,
        },
        monitor,
    ));
    assert!(!rect_covers_monitor(
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        },
        monitor,
    ));
}

thread_local! {
    static FRAME_CHANGES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

unsafe extern "system" fn count_frame_changes(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_WINDOWPOSCHANGING {
        let position = &*(lparam.0 as *const WINDOWPOS);
        if position.flags.contains(SWP_FRAMECHANGED) {
            FRAME_CHANGES.with(|changes| changes.set(changes.get() + 1));
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn take_frame_changes() -> usize {
    FRAME_CHANGES.with(|changes| changes.replace(0))
}

/// A private hidden window; these tests never touch Explorer.
struct TestWindow(HWND);

impl TestWindow {
    fn new(ex_style: WINDOW_EX_STYLE) -> Self {
        unsafe {
            let class = native_interop::wide_str("CCUTHostedSurfaceRegression");
            let instance = GetModuleHandleW(PCWSTR::null()).unwrap();
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(count_frame_changes),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            });
            Self(
                CreateWindowExW(
                    ex_style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    PCWSTR(class.as_ptr()),
                    PCWSTR::null(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .unwrap(),
            )
        }
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

#[test]
fn re_embedding_a_hosted_surface_does_not_change_its_frame() {
    let parent = TestWindow::new(WINDOW_EX_STYLE::default());
    let window = TestWindow::new(WS_EX_LAYERED);
    native_interop::embed_as_child(window.0, parent.0);
    assert!(take_frame_changes() > 0);

    for _ in 0..3 {
        native_interop::embed_as_child(window.0, parent.0);
    }
    assert_eq!(take_frame_changes(), 0);

    // A surface that left the hosted form is still converted back.
    native_interop::make_popup(window.0, true);
    take_frame_changes();
    native_interop::embed_as_child(window.0, parent.0);
    assert!(take_frame_changes() > 0);
    assert_eq!(unsafe { GetParent(window.0) }.unwrap(), parent.0);
}

#[test]
fn re_embedding_repairs_styles_even_when_the_parent_matches() {
    let parent = TestWindow::new(WINDOW_EX_STYLE::default());
    let window = TestWindow::new(WS_EX_LAYERED);
    native_interop::embed_as_child(window.0, parent.0);
    unsafe {
        let style = GetWindowLongW(window.0, GWL_STYLE);
        SetWindowLongW(window.0, GWL_STYLE, style & !(WS_CLIPSIBLINGS.0 as i32));
    }
    take_frame_changes();
    native_interop::embed_as_child(window.0, parent.0);
    assert!(take_frame_changes() > 0);
    assert_ne!(
        unsafe { GetWindowLongW(window.0, GWL_STYLE) } as u32 & WS_CLIPSIBLINGS.0,
        0
    );
}

#[test]
fn hosted_placement_detects_real_moves_resizes_and_z_order_changes() {
    let parent = TestWindow::new(WINDOW_EX_STYLE::default());
    let window = TestWindow::new(WS_EX_LAYERED);
    let sibling = TestWindow::new(WS_EX_LAYERED);
    let origin = POINT { x: 23, y: 17 };
    let placed = |origin: POINT, width, height| unsafe {
        positioning::hosted_child_is_placed(window.0, parent.0, origin, width, height)
    };
    unsafe {
        // A displaced host checks the screen-to-client conversion.
        SetWindowPos(
            parent.0,
            None,
            -200,
            100,
            200,
            100,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .unwrap();
        native_interop::embed_as_child(window.0, parent.0);
        SetWindowPos(
            window.0,
            Some(HWND_TOP),
            origin.x,
            origin.y,
            30,
            20,
            SWP_NOACTIVATE,
        )
        .unwrap();
    }
    assert!(placed(origin, 30, 20));
    assert!(!placed(POINT { x: 24, y: 17 }, 30, 20));
    assert!(!placed(origin, 31, 20));
    assert!(!placed(origin, 30, 21));
    native_interop::embed_as_child(sibling.0, parent.0);
    assert!(!placed(origin, 30, 20));
    unsafe {
        SetWindowPos(
            window.0,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .unwrap();
    }
    assert!(placed(origin, 30, 20));
}

#[test]
fn re_making_a_popup_does_not_change_its_frame() {
    let window = TestWindow::new(WS_EX_LAYERED);
    native_interop::make_popup(window.0, true);
    take_frame_changes();
    for _ in 0..3 {
        native_interop::make_popup(window.0, true);
    }
    assert_eq!(take_frame_changes(), 0);

    // Leaving the topmost band or a host still goes through.
    native_interop::make_popup(window.0, false);
    assert!(take_frame_changes() > 0);
    assert_eq!(
        unsafe { GetWindowLongW(window.0, GWL_EXSTYLE) } & WS_EX_TOPMOST.0 as i32,
        0
    );
    let parent = TestWindow::new(WINDOW_EX_STYLE::default());
    native_interop::embed_as_child(window.0, parent.0);
    take_frame_changes();
    native_interop::make_popup(window.0, false);
    assert!(take_frame_changes() > 0);
    assert!(unsafe { GetParent(window.0) }.is_err());
}

#[test]
fn desktop_windows_past_the_new_surface_count_are_destroyed() {
    let window = TestWindow::new(WINDOW_EX_STYLE::default());
    let kept = SendHwnd::from_hwnd(window.0);
    // Never a live window; only its slot position matters here.
    let removed = SendHwnd(2);
    let mut windows = vec![Some(kept), Some(removed), None];
    let stale = take_stale_desktop_windows(&mut windows, 1, &[true]);
    assert!(windows == vec![Some(kept)]);
    assert!(stale == vec![removed]);
}
