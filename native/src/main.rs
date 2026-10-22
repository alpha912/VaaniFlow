#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod config;
mod focus_monitor;
mod languages;
mod sarvam;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let (config, api_key, warning) = match config::load() {
        Ok(loaded) => (loaded.config, loaded.api_key, loaded.credential_warning),
        Err(error) => (
            config::Config::default(),
            String::new(),
            Some(format!("Settings could not be loaded: {error}")),
        ),
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("VaaniFlow")
            .with_inner_size([520.0, 760.0])
            .with_min_inner_size([480.0, 620.0]),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "VaaniFlow",
        options,
        Box::new(move |creation| {
            creation.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
            Ok(Box::new(app::VaaniApp::new(
                config,
                api_key,
                warning,
                &creation.egui_ctx,
            )))
        }),
    )
}
