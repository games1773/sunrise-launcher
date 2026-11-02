//! The Sunrise Launcher window.
//!
//! Visual translation of the supplied mock: obsidian surfaces, sunrise amber
//! accents, a custom title bar, a hero banner, and a tabbed management panel.
//!
//! Split along its seams: `app` owns the state, `work` starts the jobs a control
//! asks for, `events` applies what those jobs report, `theme` paints the palette
//! and the art, `widgets` holds the painted controls, and `panels` is the layout.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod events;
mod logo;
mod panels;
mod theme;
mod widgets;
mod work;

use app::App;
use eframe::egui;
use sunrise::paths;

fn main() -> eframe::Result {
    let data = paths::data_dir();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sunrise")
            .with_inner_size([1120.0, 720.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_icon(std::sync::Arc::new(app_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "Sunrise",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, data)))),
    )
}

/// The window and taskbar icon: the same mark the .exe carries, rasterised
/// from the pixel grid at 16x so every cell stays crisp.
fn app_icon() -> egui::IconData {
    let size = 256u32;
    egui::IconData {
        rgba: logo::rgba(size),
        width: size,
        height: size,
    }
}
