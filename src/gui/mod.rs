pub mod app;

pub use app::PaaViewerApp;

use std::path::PathBuf;

pub fn run_gui(initial_file: Option<PathBuf>) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("PAA Texture Viewer & Converter"),
        ..Default::default()
    };

    eframe::run_native(
        "PAA Viewer & Converter",
        native_options,
        Box::new(|cc| Ok(Box::new(PaaViewerApp::new(cc, initial_file)))),
    )
}
