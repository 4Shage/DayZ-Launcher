// ============================================================
//  main.rs — punkt wejścia aplikacji
// ============================================================

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod launcher;
mod profile;
mod server;
mod updater;

use app::DayZLauncher;
use eframe::egui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DayZ Launcher")
            .with_inner_size([1200.0, 680.0])
            .with_min_inner_size([900.0, 580.0])
            .with_resizable(true),

        // depth_buffer/stencil_buffer = 0 reduces GPU memory allocation,
        // which is the root cause of the glutin swap_buffers [3003] OOM
        // on Mesa and some integrated GPU drivers.
        depth_buffer: 0,
        stencil_buffer: 0,

        ..Default::default()
    };

    eframe::run_native(
        "DayZ Launcher",
        native_options,
        Box::new(|cc| Box::new(DayZLauncher::new(cc)) as Box<dyn eframe::App>),
    )
    .map_err(|e| anyhow::anyhow!("Błąd uruchamiania okna: {e}"))?;

    Ok(())
}
