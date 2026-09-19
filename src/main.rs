#![windows_subsystem = "windows"]

mod accounts;
mod app_settings;
mod context_menu;
mod dashboard;
mod desktop_compositor;
mod diagnose;
mod font_catalog;
mod localization;
mod models;
mod native_interop;
mod poller;
mod providers;
mod studio_app;
mod theme;
mod theme_engine;
mod theme_package;
mod tray_icon;
mod ui;
mod updater;
mod window;
mod winsqlite;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Without `--diagnose` the log stays closed and `diagnose::log` is a no-op.
    if args.iter().any(|arg| arg == "--diagnose") {
        let append = args.iter().any(|arg| arg == "--diagnose-append");
        if let Ok(path) = diagnose::init(append) {
            diagnose::log(format!("startup args={args:?} log_path={}", path.display()));
        }
    }

    if studio_app::handle_cli_mode(&args) {
        return;
    }

    if let Some(exit_code) = updater::handle_cli_mode(&args) {
        diagnose::log(format!("cli mode exited with code {exit_code}"));
        std::process::exit(exit_code);
    }

    diagnose::log("entering window::run");
    window::run();
}
