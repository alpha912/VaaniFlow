// src-tauri/src/focus_monitor.rs
//
// Monitors the system for text-field focus events across ALL apps.
// When a text field gains focus anywhere (Gmail, Slack, VS Code, etc.),
// emits "text-field-focused" to the bubble window so it glows and repositions.
//
// Platform support:
//   macOS   — AppleScript polling via osascript (requires Accessibility permission)
//   Windows — GetGUIThreadInfo + GetClassName polling via Win32 (no extra permission)
//   Linux   — stub; hotkey-only mode until AT-SPI2 is wired up

use tauri::{AppHandle, Emitter, Manager};

pub fn run(app: AppHandle) {
    #[cfg(target_os = "macos")]
    run_macos(app);

    #[cfg(target_os = "windows")]
    run_windows(app);

    #[cfg(target_os = "linux")]
    run_linux_stub(app);
}

// ─── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn run_macos(app: AppHandle) {
    use std::time::Duration;

    // AX roles that count as "text field"
    const TEXT_ROLES: &[&str] = &[
        "AXTextField",
        "AXTextArea",
        "AXSearchField",
        "AXComboBox",
    ];

    let mut last_was_text = false;

    loop {
        std::thread::sleep(Duration::from_millis(150));

        let is_text = macos_focused_is_text(TEXT_ROLES);

        if is_text && !last_was_text {
            if let Some(bubble) = app.get_webview_window("bubble") {
                if let Some((x, y)) = macos_focused_element_position() {
                    let bx = (x - 90.0).max(0.0) as i32;
                    let by = (y - 70.0).max(0.0) as i32;
                    let _ = bubble.set_position(tauri::PhysicalPosition::new(bx, by));
                }
                let _ = bubble.emit("text-field-focused", ());
            }
            last_was_text = true;
        } else if !is_text && last_was_text {
            if let Some(bubble) = app.get_webview_window("bubble") {
                let _ = bubble.emit("text-field-blurred", ());
            }
            last_was_text = false;
        }
    }
}

// Returns the AX role of the currently focused UI element.
// Uses osascript — no unsafe Objective-C bindings required.
#[cfg(target_os = "macos")]
fn macos_focused_is_text(text_roles: &[&str]) -> bool {
    let script = r#"
tell application "System Events"
    try
        set p to first application process whose frontmost is true
        set r to role of focused UI element of p
        return r
    on error
        return ""
    end try
end tell
"#;

    let Ok(out) = std::process::Command::new("osascript")
        .arg("-e").arg(script)
        .output()
    else { return false; };

    let role = String::from_utf8_lossy(&out.stdout);
    let role = role.trim();
    text_roles.iter().any(|&r| role == r)
}

// Returns the screen position (x, y) of the focused UI element.
#[cfg(target_os = "macos")]
fn macos_focused_element_position() -> Option<(f64, f64)> {
    let script = r#"
tell application "System Events"
    try
        set p to first application process whose frontmost is true
        set pos to position of focused UI element of p
        return ((item 1 of pos) as string) & "," & ((item 2 of pos) as string)
    on error
        return ""
    end try
end tell
"#;

    let out = std::process::Command::new("osascript")
        .arg("-e").arg(script)
        .output().ok()?;

    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim();
    let (xs, ys) = s.split_once(',')?;
    let x = xs.trim().parse::<f64>().ok()?;
    let y = ys.trim().parse::<f64>().ok()?;
    Some((x, y))
}

// ─── Windows ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_windows(app: AppHandle) {
    use std::time::Duration;

    // Window class names that indicate a text-input control.
    // Chrome/Electron renderer host is included so web text fields
    // (Gmail, Notion, Linear, etc.) also trigger the bubble.
    const TEXT_CLASSES: &[&str] = &[
        "Edit",               // Standard Win32 single-line input
        "RichEdit",           // RichEdit v1
        "RichEdit20A",        // RichEdit v2 ANSI
        "RichEdit20W",        // RichEdit v2 Unicode
        "RICHEDIT60W",        // RichEdit v6 (Office)
        "RichEditD2DPT",      // Chrome address bar
        "Chrome_RenderWidgetHostHWND", // Chrome / Electron content
    ];

    let mut last_was_text = false;

    loop {
        std::thread::sleep(Duration::from_millis(200));

        let is_text = windows_focused_is_text(TEXT_CLASSES);

        if is_text && !last_was_text {
            if let Some(bubble) = app.get_webview_window("bubble") {
                let _ = bubble.emit("text-field-focused", ());
            }
            last_was_text = true;
        } else if !is_text && last_was_text {
            if let Some(bubble) = app.get_webview_window("bubble") {
                let _ = bubble.emit("text-field-blurred", ());
            }
            last_was_text = false;
        }
    }
}

// GetForegroundWindow → get its thread → GetGUIThreadInfo (gives the focused
// HWND within that thread) → GetClassNameW.
//
// IMPORTANT: plain GetFocus() only works on the *calling* thread, which is
// why we go via GetGUIThreadInfo instead.
#[cfg(target_os = "windows")]
fn windows_focused_is_text(text_classes: &[&str]) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetClassNameW, GetWindowThreadProcessId,
        GUITHREADINFO,
    };

    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }

        // Get the thread that owns the foreground window
        let thread_id = GetWindowThreadProcessId(fg, None);
        if thread_id == 0 {
            return false;
        }

        // GetGUIThreadInfo gives us the hwndFocus of that thread — the
        // specific child control that actually has keyboard focus
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(thread_id, &mut info).is_err() {
            return false;
        }
        if info.hwndFocus.0.is_null() {
            return false;
        }

        // Read the window class name of the focused control
        let mut buf = [0u16; 256];
        let len = GetClassNameW(info.hwndFocus, &mut buf);
        if len == 0 {
            return false;
        }

        let class = String::from_utf16_lossy(&buf[..len as usize]);
        text_classes.iter().any(|&c| class.as_str().contains(c))
    }
}

// ─── Linux stub ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn run_linux_stub(_app: AppHandle) {
    // Full implementation requires AT-SPI2 (the Linux accessibility bus).
    // Contributions welcome: https://gitlab.gnome.org/GNOME/at-spi2-core
    //
    // In the meantime VaaniFlow on Linux works perfectly via the global
    // hotkey — the bubble just stays fixed in the corner rather than
    // jumping to the active text field.
    log::info!("VaaniFlow: Linux text-field detection not yet implemented. Use the hotkey to record.");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
