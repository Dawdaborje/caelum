// SPDX-License-Identifier: GPL-3.0-or-later
mod app;
mod bar;
mod clock;
mod services;
mod theme_bridge;
mod tray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let path = caelum_config::config_path();
    let cfg = caelum_config::load(&path).unwrap_or_else(|e| {
        tracing::warn!("{e}; using defaults");
        caelum_config::Config::default()
    });
    app::init(cfg);
    caelum_wayland::init(app::on_output)?;
    services::start()?;
    clock::tick();
    // Kept alive for the whole run; reloads hop onto the UI thread.
    let _watch = caelum_config::watch(path, |cfg| {
        if let Err(e) = slint::invoke_from_event_loop(move || app::on_config(cfg)) {
            tracing::warn!("config reload: {e}");
        }
    })
    .inspect_err(|e| tracing::warn!("{e}; config will not reload live"));
    slint::run_event_loop()?;
    Ok(())
}
