// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use statik_app::{AppState, StatikApp, icon};
use std::io::Write;

/// Release builds abort on panic, so write a short crash note first (best effort):
/// `%LOCALAPPDATA%\Statik\crash.txt` (or the temp directory).
fn install_crash_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("Statik"))
            .unwrap_or_else(std::env::temp_dir);
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("crash.txt"))
        {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(
                f,
                "--- Statik {} crashed (unix time {secs})\n{info}\n{}\n",
                env!("CARGO_PKG_VERSION"),
                std::backtrace::Backtrace::force_capture()
            );
        }
        default(info);
    }));
}

fn main() -> eframe::Result {
    install_crash_hook();
    let state = AppState::new();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(state.title())
            .with_inner_size([1024.0, 700.0])
            .with_min_inner_size([480.0, 320.0])
            .with_icon(icon::icon_data()),
        ..Default::default()
    };
    eframe::run_native(
        "Statik",
        options,
        Box::new(|cc| {
            // Light, like the classic statistics package look.
            cc.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(StatikApp::new(state)))
        }),
    )
}
