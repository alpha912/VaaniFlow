//! Best-effort active text-field detection. Windows foreground controls are
//! inspected locally; macOS and Linux remain hotkey-first where accessibility
//! APIs and compositor permissions vary between installations.

use std::{sync::mpsc::Sender, time::Duration};

#[derive(Debug, Clone, Copy)]
pub enum FocusEvent {
    TextField(bool),
}

pub fn spawn(sender: Sender<FocusEvent>) {
    let _ = std::thread::Builder::new()
        .name("vaaniflow-focus-monitor".into())
        .spawn(move || run(sender));
}

#[cfg(target_os = "windows")]
static LAST_EXTERNAL_WINDOW: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
fn run(sender: Sender<FocusEvent>) {
    use std::sync::atomic::Ordering;
    use windows::Win32::{
        System::Threading::GetCurrentProcessId,
        UI::WindowsAndMessaging::{
            GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
            GUITHREADINFO,
        },
    };

    // Native text controls and common browser/editor child windows. This is a
    // conservative heuristic rather than reading any field contents.
    const TEXT_CLASSES: &[&str] = &[
        "edit",
        "richedit",
        "chromerenderwidgethost",
        "mozillawindowclass",
        "scintilla",
        "_wwg",
        "excel7",
        "textboxview",
        "richtextbox",
        "cascadia_hosting_window_class",
        "consolewindowclass",
        "termcontrol",
        "inputsite",
        "windows.ui.core.corewindow",
    ];

    let current_pid = unsafe { GetCurrentProcessId() };
    let mut previous = false;
    loop {
        std::thread::sleep(Duration::from_millis(400));
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.0.is_null() {
            continue;
        }
        let mut foreground_pid = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(foreground, Some(&mut foreground_pid)) };
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focused = if thread_id != 0 && unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_ok()
        {
            if info.hwndFocus.0.is_null() {
                foreground
            } else {
                info.hwndFocus
            }
        } else {
            foreground
        };
        let mut buffer = [0u16; 256];
        let length = unsafe { GetClassNameW(focused, &mut buffer) };
        let class_name = String::from_utf16_lossy(&buffer[..length.max(0) as usize]).to_lowercase();
        let is_text = foreground_pid != current_pid
            && TEXT_CLASSES.iter().any(|class| class_name.contains(class));

        if is_text {
            LAST_EXTERNAL_WINDOW.store(foreground.0 as isize, Ordering::Relaxed);
        }
        if is_text != previous {
            let _ = sender.send(FocusEvent::TextField(is_text));
            previous = is_text;
        }
    }
}

/// Return focus to the last external window which contained a likely text
/// control. Windows may refuse activation due to foreground-lock policy; the
/// clipboard still contains the completed text in that case.
#[cfg(target_os = "windows")]
pub fn restore_previous_target() -> bool {
    use std::sync::atomic::Ordering;
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE},
    };

    let raw = LAST_EXTERNAL_WINDOW.load(Ordering::Relaxed);
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut std::ffi::c_void);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd).as_bool()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn restore_previous_target() -> bool {
    false
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run(sender: Sender<FocusEvent>) {
    let platform = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };
    log::info!("{platform} focus detection is hotkey-first; no accessibility data is polled");
    let _ = sender.send(FocusEvent::TextField(false));
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
