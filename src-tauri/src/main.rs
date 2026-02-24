// src-tauri/src/main.rs
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod focus_monitor;
mod sarvam;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

// ─── Shared state ────────────────────────────────────────────────────────────

pub struct RecordingState(pub Arc<Mutex<audio::Recorder>>);
pub struct ConfigState(pub Arc<Mutex<config::Config>>);

// ─── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn load_config(state: State<'_, ConfigState>) -> Result<config::Config, String> {
    let cfg = state.0.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
async fn save_config(
    config: config::Config,
    state: State<'_, ConfigState>,
    app: AppHandle,
) -> Result<(), String> {
    // Persist to disk
    config::save(&config).map_err(|e| e.to_string())?;

    // Update in-memory state
    let mut cfg = state.0.lock().map_err(|e| e.to_string())?;
    *cfg = config.clone();

    // Re-register global hotkey with new value
    register_hotkey(&app, &config.hotkey)?;

    Ok(())
}

#[tauri::command]
async fn test_api_key(api_key: String) -> Result<String, String> {
    sarvam::test_key(&api_key)
        .await
        .map(|_| "✓ API key is valid and reachable!".to_string())
        .map_err(|e| format!("Error: {e}"))
}

#[tauri::command]
async fn list_microphones() -> Result<Vec<String>, String> {
    Ok(audio::list_devices())
}

#[tauri::command]
async fn start_recording(
    state: State<'_, RecordingState>,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let mic = {
        let cfg = config_state.0.lock().map_err(|e| e.to_string())?;
        cfg.microphone.clone()
    };
    let mut recorder = state.0.lock().map_err(|e| e.to_string())?;
    recorder.start(&mic).map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_recording(
    app: AppHandle,
    recording_state: State<'_, RecordingState>,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    // Capture audio data + actual sample rate
    let (audio_data, sample_rate) = {
        let mut recorder = recording_state.0.lock().map_err(|e| e.to_string())?;
        recorder.stop().map_err(|e| e.to_string())?
    };

    if audio_data.is_empty() {
        return Err("No audio recorded".into());
    }

    // Detect silent audio (microphone permissions likely blocked)
    let max_amp = audio_data.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    if max_amp < 1e-4 {
        // Try to open Windows microphone privacy settings
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "ms-settings:privacy-microphone"])
                .spawn();
        }
        return Err(
            "Microphone returned silence. Please enable microphone access and try again.".into(),
        );
    }

    // Get current config
    let config = {
        let cfg = config_state.0.lock().map_err(|e| e.to_string())?;
        cfg.clone()
    };

    if config.api_key.is_empty() {
        return Err("No API key configured. Open Settings to add your Sarvam key.".into());
    }

    // Spawn async transcription so we don't block the command
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(bubble) = app_clone.get_webview_window("bubble") else {
            log::error!("Bubble window not found during transcription callback");
            return;
        };

        match transcribe_and_paste(&app_clone, audio_data, sample_rate, &config).await {
            Ok(text) => {
                if let Err(e) = paste_text(&app_clone, &text) {
                    log::error!("Paste failed: {e}");
                    let _ = bubble.emit("transcription-error", format!("Paste failed: {e}"));
                } else {
                    let _ = bubble.emit("transcription-done", text);
                }
            }
            Err(e) => {
                log::error!("Transcription failed: {e}");
                let _ = bubble.emit("transcription-error", e.to_string());
            }
        }
    });

    Ok(())
}

/// Calls Sarvam Saaras v3 API, returns final text.
///
/// Routing (all via /speech-to-text with Saaras v3):
///   target = "same"  → mode=transcribe (original language)
///   target = "en-IN" → mode=translate  (speech → English, single call)
///   target = other   → mode=transcribe then text /translate (two calls)
async fn transcribe_and_paste(
    _app: &AppHandle,
    audio: Vec<f32>,
    sample_rate: u32,
    config: &config::Config,
) -> Result<String, String> {
    let wav_bytes = audio::encode_wav(audio, sample_rate).map_err(|e| e.to_string())?;
    log::info!(
        "WAV encoded: {} bytes (resampled to 16kHz)",
        wav_bytes.len()
    );

    if config.target_language == "en-IN" {
        // English output → Saaras v3 mode=translate (single call)
        log::info!("Route: Saaras v3 translate → English");
        let result = sarvam::speech_to_text(
            &config.api_key,
            wav_bytes,
            &config.source_language,
            sarvam::SttMode::Translate,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(result.transcript)
    } else {
        // Transcribe first (original language)
        log::info!("Route: Saaras v3 transcribe");
        let stt_result = sarvam::speech_to_text(
            &config.api_key,
            wav_bytes,
            &config.source_language,
            sarvam::SttMode::Transcribe,
        )
        .await
        .map_err(|e| e.to_string())?;

        let detected_lang = stt_result.language_code.as_deref().unwrap_or("auto");

        // No translation needed if target is "same" or matches detected language
        if config.target_language == "same" || config.target_language == detected_lang {
            Ok(stt_result.transcript)
        } else {
            // Translate to target Indian language via text translate (Mayura v1)
            let translated = sarvam::translate(
                &config.api_key,
                &stt_result.transcript,
                detected_lang,
                &config.target_language,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(translated)
        }
    }
}

/// Writes text to clipboard then simulates Cmd+V (macOS) or Ctrl+V (Windows/Linux)
fn paste_text(app: &AppHandle, text: &str) -> Result<(), String> {
    // Write to clipboard
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;

    // Brief pause to let the target app regain focus before simulating Ctrl+V
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Simulate paste keystroke
    simulate_paste();
    Ok(())
}

/// Platform-specific paste simulation
fn simulate_paste() {
    use rdev::{simulate, EventType, Key};

    let paste_key = Key::KeyV;

    let hold_key = {
        #[cfg(target_os = "macos")]
        {
            Key::MetaLeft
        } // Cmd on macOS
        #[cfg(not(target_os = "macos"))]
        {
            Key::ControlLeft
        } // Ctrl on Windows/Linux
    };

    let _ = simulate(&EventType::KeyPress(hold_key));
    let _ = simulate(&EventType::KeyPress(paste_key));
    let _ = simulate(&EventType::KeyRelease(paste_key));
    let _ = simulate(&EventType::KeyRelease(hold_key));
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    // If the settings window was closed (destroyed), recreate it
    let window = match app.get_webview_window("settings") {
        Some(w) => w,
        None => {
            let url = tauri::WebviewUrl::App("index.html".into());
            tauri::WebviewWindowBuilder::new(&app, "settings", url)
                .title("VaaniFlow — Settings")
                .inner_size(560.0, 640.0)
                .resizable(false)
                .decorations(true)
                .center()
                .build()
                .map_err(|e| e.to_string())?
        }
    };
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ─── Hotkey registration ─────────────────────────────────────────────────────

fn register_hotkey(app: &AppHandle, hotkey_str: &str) -> Result<(), String> {
    let shortcut: Shortcut = hotkey_str
        .parse()
        .map_err(|_| format!("Invalid hotkey: {hotkey_str}"))?;

    // Unregister all existing shortcuts first
    let _ = app.global_shortcut().unregister_all();

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, _event| {
            // Toggle recording via hotkey
            let app = app_clone.clone();
            tauri::async_runtime::spawn(async move {
                let Some(bubble) = app.get_webview_window("bubble") else {
                    log::error!("Bubble window not found during hotkey handler");
                    return;
                };
                // We emit to the bubble and it handles the toggle logic
                let _ = bubble.emit("hotkey-triggered", ());
            });
        })
        .map_err(|e| e.to_string())
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn init_logging() {
    use std::io::Write;

    // In debug builds, write logs to a file that persists across process restarts
    #[cfg(debug_assertions)]
    {
        let log_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("vaani-debug.log");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            let file = std::sync::Mutex::new(file);
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .format(move |buf, record| {
                    let msg = format!(
                        "[{} {}] {}\n",
                        record.level(),
                        record.target(),
                        record.args()
                    );
                    let _ = buf.write_all(msg.as_bytes());
                    if let Ok(mut f) = file.lock() {
                        let _ = f.write_all(msg.as_bytes());
                        let _ = f.flush();
                    }
                    Ok(())
                })
                .init();
            return;
        }
    }

    // Fallback: normal env_logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
}

fn main() {
    init_logging();

    // Load config from disk (or defaults)
    let config = config::load().unwrap_or_default();
    let config_state = ConfigState(Arc::new(Mutex::new(config.clone())));
    let recording_state = RecordingState(Arc::new(Mutex::new(audio::Recorder::new())));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(config_state)
        .manage(recording_state)
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            test_api_key,
            list_microphones,
            start_recording,
            stop_recording,
            open_settings,
            quit_app,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Register hotkey from saved config
            let hotkey = {
                let cfg = app.state::<ConfigState>();
                let lock_result = cfg.0.lock();
                match lock_result {
                    Ok(guard) => guard.hotkey.clone(),
                    Err(poisoned) => {
                        log::warn!("Config mutex was poisoned, using poisoned value");
                        poisoned.into_inner().hotkey.clone()
                    }
                }
            };
            if let Err(e) = register_hotkey(&app_handle, &hotkey) {
                log::warn!("Could not register hotkey '{hotkey}': {e}");
            }

            // Position the bubble at the bottom-right of the primary monitor
            if let Some(bubble) = app_handle.get_webview_window("bubble") {
                if let Ok(Some(monitor)) = bubble.primary_monitor() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let phys_w = screen.width as f64;
                    let phys_h = screen.height as f64;
                    let x = (phys_w - 220.0 * scale).max(0.0) as i32;
                    let y = (phys_h - 160.0 * scale).max(0.0) as i32;
                    let _ = bubble.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            // Start background thread to monitor text-field focus across apps
            let bubble_app = app_handle.clone();
            std::thread::spawn(move || {
                focus_monitor::run(bubble_app);
            });

            // On macOS, ensure the app appears in the dock and accessibility prompt shows
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                app.set_activation_policy(ActivationPolicy::Accessory);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running Vaani")
        .run(|_app_handle, event| {
            // Keep running even when all windows are closed (lives in system tray)
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
