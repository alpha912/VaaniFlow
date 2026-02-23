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
async fn start_recording(state: State<'_, RecordingState>) -> Result<(), String> {
    let mut recorder = state.0.lock().map_err(|e| e.to_string())?;
    recorder.start().map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_recording(
    app: AppHandle,
    recording_state: State<'_, RecordingState>,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    // Capture audio data
    let audio_data = {
        let mut recorder = recording_state.0.lock().map_err(|e| e.to_string())?;
        recorder.stop().map_err(|e| e.to_string())?
    };

    // Get current config
    let config = {
        let cfg = config_state.0.lock().map_err(|e| e.to_string())?;
        cfg.clone()
    };

    // Spawn async transcription so we don't block the command
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let bubble = app_clone.get_webview_window("bubble").unwrap();

        match transcribe_and_paste(&app_clone, audio_data, &config).await {
            Ok(text) => {
                // Paste the text into the previously focused field
                if let Err(e) = paste_text(&app_clone, &text) {
                    let _ = bubble.emit("transcription-error", format!("Paste failed: {e}"));
                } else {
                    let _ = bubble.emit("transcription-done", text);
                }
            }
            Err(e) => {
                let _ = bubble.emit("transcription-error", e.to_string());
            }
        }
    });

    Ok(())
}

/// Calls Sarvam STT, then optionally Sarvam Translate, returns final text
async fn transcribe_and_paste(
    _app: &AppHandle,
    audio: Vec<f32>,
    config: &config::Config,
) -> Result<String, String> {
    // Convert to WAV bytes
    let wav_bytes = audio::encode_wav(audio, 16000).map_err(|e| e.to_string())?;

    // STT call
    let stt_result = sarvam::transcribe(&config.api_key, wav_bytes, &config.source_language)
        .await
        .map_err(|e| e.to_string())?;

    // Translation (if target language is set and different from source)
    if config.target_language != "same" && config.target_language != stt_result.language_code {
        let translated = sarvam::translate(
            &config.api_key,
            &stt_result.transcript,
            &stt_result.language_code,
            &config.target_language,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(translated)
    } else {
        Ok(stt_result.transcript)
    }
}

/// Writes text to clipboard then simulates Cmd+V (macOS) or Ctrl+V (Windows/Linux)
fn paste_text(app: &AppHandle, text: &str) -> Result<(), String> {
    // Write to clipboard
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;

    // Simulate paste keystroke
    simulate_paste();
    Ok(())
}

/// Platform-specific paste simulation
fn simulate_paste() {
    use rdev::{simulate, EventType, Key};

    #[cfg(target_os = "macos")]
    let paste_key = Key::KeyV;
    #[cfg(not(target_os = "macos"))]
    let paste_key = Key::KeyV;

    let hold_key = {
        #[cfg(target_os = "macos")]
        { Key::MetaLeft } // Cmd on macOS
        #[cfg(not(target_os = "macos"))]
        { Key::ControlLeft } // Ctrl on Windows/Linux
    };

    let _ = simulate(&EventType::KeyPress(hold_key));
    let _ = simulate(&EventType::KeyPress(paste_key));
    let _ = simulate(&EventType::KeyRelease(paste_key));
    let _ = simulate(&EventType::KeyRelease(hold_key));
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or("Settings window not found")?;
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
    let shortcut: Shortcut = hotkey_str.parse().map_err(|_| format!("Invalid hotkey: {hotkey_str}"))?;

    // Unregister all existing shortcuts first
    let _ = app.global_shortcut().unregister_all();

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, _event| {
            // Toggle recording via hotkey
            let app = app_clone.clone();
            tauri::async_runtime::spawn(async move {
                let bubble = app.get_webview_window("bubble").unwrap();
                // We emit to the bubble and it handles the toggle logic
                let _ = bubble.emit("hotkey-triggered", ());
            });
        })
        .map_err(|e| e.to_string())
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn main() {
    env_logger::init();

    // Load config from disk (or defaults)
    let config = config::load().unwrap_or_default();
    let config_state = ConfigState(Arc::new(Mutex::new(config.clone())));
    let recording_state = RecordingState(Arc::new(Mutex::new(audio::Recorder::new())));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .manage(config_state)
        .manage(recording_state)
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            test_api_key,
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
                let hotkey = cfg.0.lock().unwrap().hotkey.clone();
                hotkey
            };
            if let Err(e) = register_hotkey(&app_handle, &hotkey) {
                log::warn!("Could not register hotkey '{hotkey}': {e}");
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
