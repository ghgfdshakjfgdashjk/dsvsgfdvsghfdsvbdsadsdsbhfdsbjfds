use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";

/// How far a corner sits from the screen edge.
const MARGIN: i32 = 6;

/// Games put their own bar across the top, so the top corners start below it
/// rather than fighting for the same strip.
const TOP_MARGIN: i32 = 78;

const WIDTH: f64 = 132.0;
const HEIGHT: f64 = 44.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlaySettings {
    pub enabled: bool,
    /// One of the four corners, or "custom" to use the coordinates below.
    pub position: String,
    pub x: i32,
    pub y: i32,
    /// Only show while one of the named windows is in front.
    pub only_in_windows: bool,
    /// Names to look for in the front window's title. Case does not matter,
    /// and a partial match counts.
    pub windows: Vec<String>,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        OverlaySettings {
            enabled: false,
            position: "top-right".into(),
            x: 40,
            y: 40,
            only_in_windows: false,
            windows: vec!["Roblox".into()],
        }
    }
}

impl OverlaySettings {
    pub fn sanitised(mut self) -> Self {
        const CORNERS: [&str; 5] = [
            "top-left",
            "top-right",
            "bottom-left",
            "bottom-right",
            "custom",
        ];
        if !CORNERS.contains(&self.position.as_str()) {
            self.position = "top-right".into();
        }
        self.x = self.x.clamp(-32_000, 32_000);
        self.y = self.y.clamp(-32_000, 32_000);

        self.windows = self
            .windows
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .take(20)
            .collect();

        self
    }
}

/// What the overlay is currently allowed to appear over, and whether it is
/// showing. Kept here so the bind watcher can consult it every so often
/// without having to reach into the app's settings.
static ALLOWED: Mutex<Option<Vec<String>>> = Mutex::new(None);
static SHOWING: AtomicBool = AtomicBool::new(true);

fn remember(settings: &OverlaySettings) {
    let names = if settings.only_in_windows && !settings.windows.is_empty() {
        Some(
            settings
                .windows
                .iter()
                .map(|name| name.to_lowercase())
                .collect(),
        )
    } else {
        None
    };

    if let Ok(mut held) = ALLOWED.lock() {
        *held = names;
    }
}

/// Does this window title pass the filter? With no filter, everything does.
pub fn allowed(title: &str) -> bool {
    let Ok(held) = ALLOWED.lock() else {
        return true;
    };

    match held.as_ref() {
        None => true,
        Some(names) => {
            let title = title.to_lowercase();
            names.iter().any(|name| title.contains(name))
        }
    }
}

/// Show or hide to match whatever is in front, doing nothing when it is
/// already in the right state.
pub fn follow_foreground(app: &AppHandle, title: &str) {
    let want = allowed(title);
    if SHOWING.swap(want, Ordering::Relaxed) == want {
        return;
    }

    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window(LABEL) {
            let _ = if want { window.show() } else { window.hide() };
        }
    });
}

/// Put the overlay where the settings say, in real pixels.
///
/// Corners are worked out from the monitor the overlay is on rather than
/// assumed, so a second screen or any scaling still lands it in the corner.
pub fn place(window: &WebviewWindow, settings: &OverlaySettings) {
    let size = match window.outer_size() {
        Ok(size) => size,
        Err(_) => return,
    };

    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    let (origin, screen) = match monitor {
        Some(monitor) => (*monitor.position(), *monitor.size()),
        None => return,
    };

    let far_x = origin.x + (screen.width.saturating_sub(size.width)) as i32 - MARGIN;
    let far_y = origin.y + (screen.height.saturating_sub(size.height)) as i32 - MARGIN;
    let near_x = origin.x + MARGIN;
    let near_y = origin.y + TOP_MARGIN;

    let (x, y) = match settings.position.as_str() {
        "top-left" => (near_x, near_y),
        "bottom-left" => (near_x, far_y),
        "bottom-right" => (far_x, far_y),
        "custom" => (settings.x, settings.y),
        _ => (far_x, near_y),
    };

    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Bring the overlay up, move it, or take it away, to match the settings.
///
/// Windows belong to the event loop. Building one straight from a command
/// thread can wedge the whole app: the call waits on the event loop, which is
/// itself waiting on the webview that made the call. Hand the work over and
/// return immediately instead.
pub fn apply(app: &AppHandle, settings: &OverlaySettings) {
    let handle = app.clone();
    let wanted = settings.clone();
    let _ = app.run_on_main_thread(move || apply_now(&handle, &wanted));
}

fn apply_now(app: &AppHandle, settings: &OverlaySettings) {
    remember(settings);
    SHOWING.store(true, Ordering::Relaxed);

    if let Some(window) = app.get_webview_window(LABEL) {
        if settings.enabled {
            place(&window, settings);
            let _ = window.show();
        } else {
            let _ = window.close();
        }
        return;
    }

    if !settings.enabled {
        return;
    }

    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Syntax overlay")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build();

    if let Ok(window) = built {
        // clicks go straight through to whatever is underneath
        let _ = window.set_ignore_cursor_events(true);
        place(&window, settings);
        let _ = window.show();
    }
}

pub fn is_up(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}
