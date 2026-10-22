use crate::{audio, config, focus_monitor, languages, sarvam};
use eframe::egui::{self, Color32, RichText, Stroke};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, Submenu},
    Icon, TrayIcon, TrayIconBuilder,
};

const ACCENT: Color32 = Color32::from_rgb(255, 133, 51);
const PANEL: Color32 = Color32::from_rgb(30, 33, 39);
const LANGUAGES: &[languages::Language] = languages::LANGUAGES;

#[derive(Debug)]
enum AppMessage {
    ToggleRecording(u32),
    Show,
    Quit,
    Transcription(Result<String, String>),
    ApiKeyTest(Result<(), String>),
}

pub struct VaaniApp {
    config: config::Config,
    api_key: String,
    key_entry: String,
    credential_warning: Option<String>,
    banner: String,
    banner_is_error: bool,
    draft: String,
    recorder: audio::Recorder,
    recording: bool,
    processing: bool,
    recording_started: Option<Instant>,
    text_field_focused: bool,
    microphone_names: Vec<String>,
    message_tx: Sender<AppMessage>,
    message_rx: Receiver<AppMessage>,
    focus_rx: Receiver<focus_monitor::FocusEvent>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey: Option<HotKey>,
    tray: Option<TrayIcon>,
    quitting: bool,
}

impl VaaniApp {
    pub fn new(
        mut config: config::Config,
        api_key: String,
        credential_warning: Option<String>,
        ctx: &egui::Context,
    ) -> Self {
        config.normalize();
        let (message_tx, message_rx) = mpsc::channel();
        let (focus_tx, focus_rx) = mpsc::channel();
        focus_monitor::spawn(focus_tx);

        let mut app = Self {
            config,
            key_entry: api_key.clone(),
            api_key,
            credential_warning,
            banner: "Ready — press the shortcut or Record to dictate".into(),
            banner_is_error: false,
            draft: String::new(),
            recorder: audio::Recorder::new(),
            recording: false,
            processing: false,
            recording_started: None,
            text_field_focused: false,
            microphone_names: audio::list_devices(),
            message_tx,
            message_rx,
            focus_rx,
            hotkey_manager: None,
            hotkey: None,
            tray: None,
            quitting: false,
        };
        app.install_hotkey(ctx);
        app.install_tray(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        app
    }

    fn install_hotkey(&mut self, ctx: &egui::Context) {
        match GlobalHotKeyManager::new() {
            Ok(manager) => {
                self.hotkey_manager = Some(manager);
                if let Err(error) = self.register_hotkey() {
                    self.set_error(format!("Global shortcut unavailable: {error}"));
                }
            }
            Err(error) => self.set_error(format!("Global shortcuts unavailable: {error}")),
        }

        let receiver = GlobalHotKeyEvent::receiver().clone();
        let sender = self.message_tx.clone();
        let repaint = ctx.clone();
        let _ = thread::Builder::new()
            .name("vaaniflow-hotkey-events".into())
            .spawn(move || loop {
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) if event.state() == HotKeyState::Pressed => {
                        let _ = sender.send(AppMessage::ToggleRecording(event.id()));
                        repaint.request_repaint();
                    }
                    Ok(_) | Err(_) => {}
                }
            });
    }

    fn register_hotkey(&mut self) -> Result<(), String> {
        let manager = self
            .hotkey_manager
            .as_ref()
            .ok_or_else(|| "the shortcut manager did not initialize".to_owned())?;
        let candidate: HotKey = self
            .config
            .hotkey
            .parse()
            .map_err(|error: global_hotkey::hotkey::HotKeyParseError| error.to_string())?;
        if let Some(previous) = self.hotkey {
            manager
                .unregister(previous)
                .map_err(|error| error.to_string())?;
        }
        match manager.register(candidate) {
            Ok(()) => {
                self.hotkey = Some(candidate);
                Ok(())
            }
            Err(error) => {
                if let Some(previous) = self.hotkey {
                    let _ = manager.register(previous);
                }
                Err(error.to_string())
            }
        }
    }

    fn install_tray(&mut self, ctx: &egui::Context) {
        let tray = match create_tray() {
            Ok(tray) => tray,
            Err(error) => {
                log::warn!("Could not create system tray icon: {error}");
                return;
            }
        };
        self.tray = Some(tray);

        let receiver = MenuEvent::receiver().clone();
        let sender = self.message_tx.clone();
        let repaint = ctx.clone();
        let _ = thread::Builder::new()
            .name("vaaniflow-tray-events".into())
            .spawn(move || loop {
                if let Ok(event) = receiver.recv_timeout(Duration::from_millis(250)) {
                    let action = match event.id().as_ref() {
                        "settings" => Some(AppMessage::Show),
                        "quit" => Some(AppMessage::Quit),
                        _ => None,
                    };
                    if let Some(action) = action {
                        let _ = sender.send(action);
                        repaint.request_repaint();
                    }
                }
            });
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.message_rx.try_recv() {
            match message {
                AppMessage::ToggleRecording(id)
                    if self.hotkey.is_some_and(|hotkey| hotkey.id() == id) =>
                {
                    self.toggle_recording();
                }
                AppMessage::ToggleRecording(_) => {}
                AppMessage::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                AppMessage::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                AppMessage::Transcription(result) => {
                    self.processing = false;
                    match result {
                        Ok(text) => {
                            self.draft = text;
                            self.banner = "Transcription ready".into();
                            self.banner_is_error = false;
                            if !self.config.review_before_paste {
                                self.copy_or_paste(ctx, true);
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                            }
                        }
                        Err(error) => self.set_error(error),
                    }
                }
                AppMessage::ApiKeyTest(result) => match result {
                    Ok(()) => {
                        self.banner = "Sarvam API key is valid and reachable".into();
                        self.banner_is_error = false;
                    }
                    Err(error) => self.set_error(error),
                },
            }
        }

        while let Ok(focus_monitor::FocusEvent::TextField(focused)) = self.focus_rx.try_recv() {
            self.text_field_focused = focused;
        }

        if self.recording && self.recorder.is_duration_limit_reached() {
            self.stop_recording();
            self.banner = "Maximum recording length reached; processing audio…".into();
            self.banner_is_error = false;
        }
    }

    fn toggle_recording(&mut self) {
        if self.processing {
            self.banner = "Please wait for the current transcription to finish".into();
            return;
        }
        if self.recording {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    fn start_recording(&mut self) {
        if self.api_key.trim().is_empty() {
            self.set_error("Add your Sarvam API key in Settings before recording".into());
            return;
        }
        match self.recorder.start(&self.config.microphone) {
            Ok(()) => {
                self.recording = true;
                self.recording_started = Some(Instant::now());
                self.banner = "Listening — press the shortcut again to finish".into();
                self.banner_is_error = false;
            }
            Err(error) => self.set_error(format!("Could not start microphone: {error}")),
        }
    }

    fn stop_recording(&mut self) {
        if !self.recording {
            return;
        }
        self.recording = false;
        self.recording_started = None;
        let (samples, sample_rate) = match self.recorder.stop() {
            Ok(audio) => audio,
            Err(error) => {
                self.set_error(format!("Could not stop microphone: {error}"));
                return;
            }
        };
        if samples.is_empty() {
            self.set_error("No audio was captured. Check the selected microphone.".into());
            return;
        }
        if samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max)
            < 1e-4
        {
            self.set_error("The microphone returned silence. Check microphone permissions.".into());
            return;
        }

        let config = self.config.clone();
        let api_key = self.api_key.clone();
        let sender = self.message_tx.clone();
        self.processing = true;
        self.banner = "Transcribing securely with Sarvam…".into();
        self.banner_is_error = false;
        let _ = thread::Builder::new()
            .name("vaaniflow-transcription".into())
            .spawn(move || {
                let result = audio::encode_wav(samples, sample_rate)
                    .map_err(|error| error.to_string())
                    .and_then(|wav| {
                        tokio::runtime::Runtime::new()
                            .map_err(|error| error.to_string())
                            .and_then(|runtime| {
                                runtime
                                    .block_on(sarvam::transcribe_and_translate(
                                        &api_key, wav, &config,
                                    ))
                                    .map_err(|error| error.to_string())
                            })
                    });
                let _ = sender.send(AppMessage::Transcription(result));
            });
    }

    fn test_api_key(&mut self) {
        let key = self.key_entry.trim().to_owned();
        if key.is_empty() {
            self.set_error("Enter an API key first".into());
            return;
        }
        let sender = self.message_tx.clone();
        self.banner = "Checking API key with Sarvam…".into();
        self.banner_is_error = false;
        let _ = thread::Builder::new()
            .name("vaaniflow-api-key-test".into())
            .spawn(move || {
                let result = tokio::runtime::Runtime::new()
                    .map_err(|error| error.to_string())
                    .and_then(|runtime| {
                        runtime
                            .block_on(sarvam::test_key(&key))
                            .map_err(|error| error.to_string())
                    });
                let _ = sender.send(AppMessage::ApiKeyTest(result));
            });
    }

    fn save_settings(&mut self) {
        self.config.normalize();
        let old_hotkey = self.hotkey;
        if let Err(error) = self.register_hotkey() {
            self.set_error(format!(
                "Settings were not saved: invalid shortcut: {error}"
            ));
            return;
        }

        let new_key = self.key_entry.trim().to_owned();
        match config::save_api_key(&new_key) {
            Ok(()) => {
                self.api_key = new_key;
                self.credential_warning = None;
            }
            Err(error) => {
                self.api_key = new_key;
                self.credential_warning = Some(format!(
                    "The key is only stored for this session; the OS keyring failed: {error}"
                ));
            }
        }

        if let Err(error) = config::save(&self.config) {
            if let (Some(manager), Some(current)) = (&self.hotkey_manager, self.hotkey) {
                let _ = manager.unregister(current);
            }
            if let (Some(manager), Some(previous)) = (&self.hotkey_manager, old_hotkey) {
                let _ = manager.register(previous);
                self.hotkey = Some(previous);
            }
            self.set_error(format!("Could not save settings: {error}"));
            return;
        }
        self.banner = "Settings saved".into();
        self.banner_is_error = false;
    }

    fn copy_or_paste(&mut self, ctx: &egui::Context, automatic: bool) {
        if self.draft.trim().is_empty() {
            self.set_error("There is no transcription to copy yet".into());
            return;
        }
        match copy_text(&self.draft, automatic) {
            Ok(pasted) => {
                self.banner = if pasted {
                    "Text copied and pasted into the previous app".into()
                } else {
                    "Text copied — paste it into your app with Ctrl+V / Cmd+V".into()
                };
                self.banner_is_error = false;
                if automatic && pasted {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
            }
            Err(error) => self.set_error(format!("Could not copy transcription: {error}")),
        }
    }

    fn set_error(&mut self, error: String) {
        self.banner = error;
        self.banner_is_error = true;
    }

    fn draw_language_combo(ui: &mut egui::Ui, id: &str, selected: &mut String, source: bool) {
        egui::ComboBox::from_id_salt(id)
            .selected_text(languages::language_name(selected))
            .width(210.0)
            .show_ui(ui, |ui| {
                if source {
                    ui.selectable_value(selected, "auto".into(), "Auto-detect");
                } else {
                    ui.selectable_value(selected, "same".into(), "Same as source");
                }
                for language in LANGUAGES {
                    ui.selectable_value(selected, language.code.to_owned(), language.name);
                }
            });
    }

    fn draw_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("VaaniFlow").size(28.0).strong().color(ACCENT));
            ui.add_space(8.0);
            ui.label(RichText::new("v0.3 · native Rust").weak());
        });
        ui.label("Speak naturally. Get text in the language you need.");
    }

    fn draw_recording_card(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(PANEL)
            .stroke(Stroke::new(1.0, Color32::from_gray(58)))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("DICTATION").size(12.0).strong().color(ACCENT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let duration = self
                            .recording_started
                            .map(|started| format_duration(started.elapsed()))
                            .unwrap_or_default();
                        if self.recording {
                            ui.label(RichText::new(duration).monospace().color(ACCENT));
                        } else if self.processing {
                            ui.spinner();
                        }
                    });
                });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(if self.recording {
                        "Listening…"
                    } else if self.processing {
                        "Transcribing your recording…"
                    } else if self.text_field_focused {
                        "Text field detected — ready when you are"
                    } else {
                        "Ready when you are"
                    })
                    .size(20.0)
                    .strong(),
                );
                ui.add_space(12.0);
                let label = if self.recording {
                    "Finish recording"
                } else if self.processing {
                    "Please wait…"
                } else {
                    "●   Start recording"
                };
                let button = egui::Button::new(RichText::new(label).strong())
                    .fill(if self.recording {
                        Color32::from_rgb(177, 54, 49)
                    } else {
                        ACCENT
                    })
                    .min_size(egui::vec2(ui.available_width(), 44.0));
                if ui.add_enabled(!self.processing, button).clicked() {
                    self.toggle_recording();
                }
                ui.add_space(7.0);
                ui.label(RichText::new(format!("Global shortcut: {}", self.config.hotkey)).weak());
            });
    }

    fn draw_result(&mut self, ui: &mut egui::Ui) {
        if self.draft.is_empty() {
            return;
        }
        ui.add_space(12.0);
        egui::Frame::group(ui.style())
            .fill(PANEL)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("REVIEW TRANSCRIPTION").size(12.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Clear").clicked() {
                            self.draft.clear();
                        }
                    });
                });
                ui.add_space(7.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.draft)
                        .desired_rows(4)
                        .hint_text("Your transcription appears here"),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy text").clicked() {
                        self.copy_or_paste(ui.ctx(), false);
                    }
                    if ui
                        .add_enabled(
                            cfg!(target_os = "windows"),
                            egui::Button::new("Paste to previous app"),
                        )
                        .clicked()
                    {
                        self.copy_or_paste(ui.ctx(), true);
                    }
                });
            });
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.label(RichText::new("SETTINGS").size(12.0).strong().color(ACCENT));
        ui.add_space(8.0);

        egui::Grid::new("language_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                ui.label("Speak in");
                Self::draw_language_combo(
                    ui,
                    "source_language",
                    &mut self.config.source_language,
                    true,
                );
                ui.end_row();
                ui.label("Output language");
                Self::draw_language_combo(
                    ui,
                    "target_language",
                    &mut self.config.target_language,
                    false,
                );
                ui.end_row();
                ui.label("Microphone");
                egui::ComboBox::from_id_salt("microphone")
                    .selected_text(&self.config.microphone)
                    .width(210.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.microphone,
                            "default".into(),
                            "System default",
                        );
                        for name in &self.microphone_names {
                            ui.selectable_value(&mut self.config.microphone, name.clone(), name);
                        }
                    });
                ui.end_row();
                ui.label("Speech model");
                egui::ComboBox::from_id_salt("speech_model")
                    .selected_text(&self.config.stt_model)
                    .width(210.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.stt_model,
                            "saaras:v3".into(),
                            "Saaras v3",
                        );
                        ui.selectable_value(
                            &mut self.config.stt_model,
                            "saaras:v4".into(),
                            "Saaras v4 (preview)",
                        );
                    });
                ui.end_row();
                ui.label("Global shortcut");
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.hotkey)
                        .hint_text("Ctrl+Shift+Space")
                        .desired_width(210.0),
                );
                ui.end_row();
                ui.label("Speech mode");
                egui::ComboBox::from_id_salt("speech_mode")
                    .selected_text(&self.config.stt_mode)
                    .width(210.0)
                    .show_ui(ui, |ui| {
                        for mode in ["transcribe", "translate", "verbatim", "translit", "codemix"] {
                            ui.selectable_value(&mut self.config.stt_mode, mode.into(), mode);
                        }
                    });
                ui.end_row();
            });

        if self.config.stt_model == "saaras:v4" {
            ui.add_space(8.0);
            ui.label("Terminology hints (comma-separated, up to 50)");
            ui.add(
                egui::TextEdit::singleline(&mut self.config.keyterms)
                    .hint_text("Names, products, or specialist words")
                    .desired_width(f32::INFINITY),
            );
        }
        if self.config.target_language != "same" && self.config.target_language != "en-IN" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Translation style");
                egui::ComboBox::from_id_salt("translation_mode")
                    .selected_text(&self.config.translation_mode)
                    .show_ui(ui, |ui| {
                        for mode in [
                            "formal",
                            "classic-colloquial",
                            "modern-colloquial",
                            "code-mixed",
                        ] {
                            ui.selectable_value(
                                &mut self.config.translation_mode,
                                mode.into(),
                                mode,
                            );
                        }
                    });
            });
        }

        ui.add_space(9.0);
        ui.label("Sarvam API key (saved in your operating-system keyring)");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.key_entry)
                    .password(true)
                    .hint_text("Paste your API key")
                    .desired_width(ui.available_width() - 92.0),
            );
            if ui.button("Test key").clicked() {
                self.test_api_key();
            }
        });

        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.config.review_before_paste,
                "Review before copying / pasting",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("Save settings").strong()).clicked() {
                    self.save_settings();
                }
            });
        });
    }

    fn draw_banner(&self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        let color = if self.banner_is_error {
            Color32::from_rgb(255, 145, 130)
        } else {
            Color32::from_rgb(190, 198, 207)
        };
        ui.label(RichText::new(&self.banner).color(color).size(13.0));
        if let Some(warning) = &self.credential_warning {
            ui.add_space(4.0);
            ui.label(RichText::new(warning).color(Color32::YELLOW).size(12.0));
        }
        ui.add_space(5.0);
        ui.label(RichText::new("Audio is sent directly to Sarvam for recognition; VaaniFlow does not log transcripts.").weak().size(11.0));
    }
}

impl eframe::App for VaaniApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);
        if self.recording {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_messages(&ctx);

        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            if self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                // Do not strand users with an invisible app when this desktop
                // session does not provide a working tray implementation.
                self.quitting = true;
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::from_rgb(21, 23, 28)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.draw_header(ui);
                    ui.add_space(18.0);
                    self.draw_recording_card(ui);
                    self.draw_result(ui);
                    self.draw_settings(ui);
                    self.draw_banner(ui);
                });
            });

        if self.recording {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }
}

fn create_tray() -> Result<TrayIcon, String> {
    let open = MenuItem::with_id("settings", "Open VaaniFlow", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    let submenu = Submenu::with_id_and_items("vaaniflow", "VaaniFlow", true, &[&open, &quit])
        .map_err(|error| error.to_string())?;
    let menu = Menu::with_items(&[&submenu]).map_err(|error| error.to_string())?;
    let decoded = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = decoded.dimensions();
    let icon =
        Icon::from_rgba(decoded.into_raw(), width, height).map_err(|error| error.to_string())?;

    TrayIconBuilder::new()
        .with_tooltip("VaaniFlow — voice typing")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|error| error.to_string())
}

fn copy_text(text: &str, try_paste: bool) -> Result<bool, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|error| error.to_string())?;
    drop(clipboard);

    if try_paste && focus_monitor::restore_previous_target() {
        thread::sleep(Duration::from_millis(120));
        if simulate_paste().is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "windows")]
fn simulate_paste() -> Result<(), String> {
    use rdev::{simulate, EventType, Key};
    simulate(&EventType::KeyPress(Key::ControlLeft)).map_err(|error| format!("{error:?}"))?;
    simulate(&EventType::KeyPress(Key::KeyV)).map_err(|error| format!("{error:?}"))?;
    simulate(&EventType::KeyRelease(Key::KeyV)).map_err(|error| format!("{error:?}"))?;
    simulate(&EventType::KeyRelease(Key::ControlLeft)).map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn simulate_paste() -> Result<(), String> {
    Err("automatic paste is currently available on Windows only".into())
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
