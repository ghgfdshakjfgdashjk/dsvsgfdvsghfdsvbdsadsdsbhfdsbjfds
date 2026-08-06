mod automation;
mod clickers;
mod engine;
mod fisher;
mod gumdrop;
mod hotkeys;
mod optimize;
mod recorder;
mod sequence;
mod crossbow;
mod overlay;
mod shake;
mod share;
mod davey;
mod skywars;
mod settings;
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WebviewWindow};

use automation::{AutomationSettings, AutomationStatus, Automator};
use clickers::{Clickers, Shared};
use engine::TargetInfo;
use hotkeys::{AppStatus, Capture};
use sequence::Step;
use settings::Settings;

const FRAME_TINT_FLAT_DARK: (u8, u8, u8) = (20, 21, 25);
const FRAME_TINT_FLAT_LIGHT: (u8, u8, u8) = (245, 246, 249);

fn frame_tint(settings: &Settings, light: bool) -> (u8, u8, u8) {
    // every gradient theme tints the frame; only the flat ones do not
    if !settings.theme.starts_with("gradient") {
        return if light {
            FRAME_TINT_FLAT_LIGHT
        } else {
            FRAME_TINT_FLAT_DARK
        };
    }

    let hue = settings.accent_hue.rem_euclid(360.0);
    if light {
        hsl_to_rgb(hue, 0.78, 0.90)
    } else {

        hsl_to_rgb(hue, 0.74, 0.14)
    }
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = lightness - c / 2.0;

    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let byte = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (byte(r), byte(g), byte(b))
}

pub struct AppState {
    clickers: Arc<Clickers>,
    shared: Arc<Shared>,
    automator: Arc<Automator>,
    capture: Arc<Capture>,
    recorder: Arc<recorder::Recorder>,
    fisher: Arc<fisher::Fisher>,
    gumdrop: Arc<gumdrop::Gumdrop>,
    skywars: Arc<skywars::Skywars>,
    davey: Arc<davey::Davey>,
    crossbow: Arc<crossbow::Crossbow>,

    settings: Mutex<Settings>,

    light_theme: AtomicBool,
    launched: Instant,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn apply_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<Settings, String> {

    let mut applied = settings.sanitised();

    applied.automation = state.automator.settings();

    state.clickers.sync(&applied.profiles);
    state.shared.update(&applied);
    *state.settings.lock().unwrap() = applied.clone();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(applied.always_on_top);
        decorate_window(&window, &applied, state.light_theme.load(Ordering::Relaxed));
    }

    let _ = settings::save(&app, &applied);
    Ok(applied)
}

#[tauri::command]
fn get_status(state: State<AppState>) -> AppStatus {
    hotkeys::build_status(&state.clickers, state.capture.is_armed(), state.launched)
}

#[tauri::command]
fn set_active(state: State<AppState>, index: usize, active: bool) -> bool {
    match state.clickers.get(index) {
        Some(engine) => {
            engine.set_active(active && engine.settings().enabled);
            engine.is_active()
        }
        None => false,
    }
}

#[tauri::command]
fn toggle_active(state: State<AppState>, index: usize) -> bool {
    match state.clickers.get(index) {

        Some(engine) if engine.settings().enabled => engine.toggle(),
        _ => false,
    }
}

#[tauri::command]
fn stop_all(state: State<AppState>) {
    state.clickers.stop_all();
}

#[tauri::command]
fn reset_clicks(state: State<AppState>, index: usize) {
    if let Some(engine) = state.clickers.get(index) {
        engine.reset_clicks();
    }
}

#[tauri::command]
fn reset_stats(state: State<AppState>) {
    for engine in state.clickers.snapshot() {
        engine.reset_stats();
    }
}

#[tauri::command]
fn get_automation(state: State<AppState>) -> AutomationSettings {
    state.automator.settings()
}

#[tauri::command]
fn get_fisher(state: State<AppState>) -> fisher::FisherSettings {
    state.fisher.settings()
}

#[tauri::command]
fn apply_fisher(
    app: AppHandle,
    state: State<AppState>,
    config: fisher::FisherSettings,
) -> fisher::FisherSettings {
    state.fisher.apply(config);
    let applied = state.fisher.settings();

    let mut settings = state.settings.lock().unwrap();
    settings.fisher = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_fisher_status(state: State<AppState>) -> fisher::FisherStatus {
    state.fisher.status()
}

#[tauri::command]
fn toggle_fisher(state: State<AppState>) -> bool {
    state.fisher.toggle()
}

#[tauri::command]
fn stop_fisher(state: State<AppState>) {
    state.fisher.set_running(false);
}

#[tauri::command]
fn reset_fisher_counts(state: State<AppState>) {
    state.fisher.reset_counts();
}

#[tauri::command]
fn default_profile() -> settings::Profile {
    settings::Profile::default()
}

#[tauri::command]
fn get_gumdrop(state: State<AppState>) -> gumdrop::GumdropSettings {
    state.gumdrop.settings()
}

#[tauri::command]
fn apply_gumdrop(
    app: AppHandle,
    state: State<AppState>,
    config: gumdrop::GumdropSettings,
) -> gumdrop::GumdropSettings {
    state.gumdrop.apply(config);
    let applied = state.gumdrop.settings();

    let mut settings = state.settings.lock().unwrap();
    settings.gumdrop = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_gumdrop_status(state: State<AppState>) -> gumdrop::GumdropStatus {
    state.gumdrop.status()
}

#[tauri::command]
fn fire_gumdrop(state: State<AppState>) {
    state.gumdrop.fire();
}

#[tauri::command]
fn get_skywars(state: State<AppState>) -> skywars::SkywarsSettings {
    state.skywars.settings()
}

#[tauri::command]
fn apply_skywars(
    app: AppHandle,
    state: State<AppState>,
    config: skywars::SkywarsSettings,
) -> skywars::SkywarsSettings {
    state.skywars.apply(config);
    let applied = state.skywars.settings();

    let mut settings = state.settings.lock().unwrap();
    settings.skywars = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_skywars_status(state: State<AppState>) -> skywars::SkywarsStatus {
    state.skywars.status()
}

#[tauri::command]
fn fire_skywars(state: State<AppState>) {
    state.skywars.fire();
}

#[tauri::command]
fn get_davey(state: State<AppState>) -> davey::DaveySettings {
    state.davey.settings()
}

#[tauri::command]
fn apply_davey(
    app: AppHandle,
    state: State<AppState>,
    config: davey::DaveySettings,
) -> davey::DaveySettings {
    state.davey.apply(config);
    let applied = state.davey.settings();

    let mut settings = settings::load(&app);
    settings.davey = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_davey_status(state: State<AppState>) -> davey::DaveyStatus {
    state.davey.status()
}

#[tauri::command]
fn fire_davey(state: State<AppState>) {
    state.davey.fire();
}

#[tauri::command]
fn get_crossbow(state: State<AppState>) -> crossbow::CrossbowSettings {
    state.crossbow.settings()
}

#[tauri::command]
fn apply_crossbow(
    app: AppHandle,
    state: State<AppState>,
    config: crossbow::CrossbowSettings,
) -> crossbow::CrossbowSettings {
    state.crossbow.apply(config);
    let applied = state.crossbow.settings();

    let mut settings = settings::load(&app);
    settings.crossbow = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_crossbow_status(state: State<AppState>) -> crossbow::CrossbowStatus {
    state.crossbow.status()
}

/// Factory settings for one macro, as plain JSON.
///
/// One command for all of them, rather than five that each return their own
/// type, so that a default is written down once -- in the `Default` impl next
/// to the code that uses it -- and never copied into the interface where the
/// two could drift apart.
///
/// The hotkey is deliberately not included. It is the one setting that is not
/// a tuning knob, and clearing it would leave the macro silently unreachable,
/// which reads as a broken reset rather than a thorough one. The caller keeps
/// whatever bind is already set.
#[tauri::command]
fn macro_defaults(which: String) -> Result<serde_json::Value, String> {
    let value = match which.as_str() {
        "fisher" => serde_json::to_value(fisher::FisherSettings::default()),
        "gumdrop" => serde_json::to_value(gumdrop::GumdropSettings::default()),
        "skywars" => serde_json::to_value(skywars::SkywarsSettings::default()),
        "davey" => serde_json::to_value(davey::DaveySettings::default()),
        "crossbow" => serde_json::to_value(crossbow::CrossbowSettings::default()),
        "overlay" => serde_json::to_value(overlay::OverlaySettings::default()),
        other => return Err(format!("no macro called {other}")),
    };

    value.map_err(|err| err.to_string())
}

#[tauri::command]
fn fire_crossbow(state: State<AppState>) {
    state.crossbow.fire();
}

#[tauri::command]
fn export_code(app: AppHandle, scope: String) -> String {
    share::export(&settings::load(&app), &scope)
}

#[tauri::command]
fn describe_code(code: String) -> Result<String, String> {
    share::describe(&code)
}

/// Read a code in and make it the live configuration.
#[tauri::command]
fn import_code(app: AppHandle, code: String) -> Result<settings::Settings, String> {
    let current = settings::load(&app);
    let next = share::import(&code, &current)?;

    settings::save(&app, &next)?;
    overlay::apply(&app, &next.overlay);

    Ok(next)
}

#[tauri::command]
fn save_preset(app: AppHandle, name: String) -> Vec<settings::Preset> {
    let mut current = settings::load(&app);
    let code = share::export(&current, "all");

    let name = {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            format!("Preset {}", current.presets.len() + 1)
        } else {
            trimmed.to_string()
        }
    };

    // saving under a name you already used replaces it rather than piling up
    match current.presets.iter_mut().find(|p| p.name == name) {
        Some(existing) => existing.code = code,
        None => current.presets.push(settings::Preset { name, code }),
    }

    let cleaned = current.sanitised();
    let _ = settings::save(&app, &cleaned);
    cleaned.presets
}

#[tauri::command]
fn delete_preset(app: AppHandle, index: usize) -> Vec<settings::Preset> {
    let mut current = settings::load(&app);
    if index < current.presets.len() {
        current.presets.remove(index);
    }
    let _ = settings::save(&app, &current);
    current.presets
}

#[tauri::command]
fn get_overlay(app: AppHandle) -> overlay::OverlaySettings {
    settings::load(&app).overlay
}

#[tauri::command]
fn apply_overlay(app: AppHandle, config: overlay::OverlaySettings) -> overlay::OverlaySettings {
    let applied = config.sanitised();

    let mut settings = settings::load(&app);
    settings.overlay = applied.clone();
    let _ = settings::save(&app, &settings);

    overlay::apply(&app, &applied);
    applied
}

#[tauri::command]
fn apply_automation(
    app: AppHandle,
    state: State<AppState>,
    automation: AutomationSettings,
) -> AutomationSettings {
    let applied = state.automator.update_settings(automation);

    let mut settings = state.settings.lock().unwrap();
    settings.automation = applied.clone();
    let _ = settings::save(&app, &settings);

    applied
}

#[tauri::command]
fn get_automation_status(state: State<AppState>) -> AutomationStatus {
    state.automator.status()
}

#[tauri::command]
fn toggle_automation(state: State<AppState>) -> bool {
    state.automator.toggle()
}

#[tauri::command]
fn stop_automation(state: State<AppState>) {
    state.automator.set_running(false);
}

#[tauri::command]
fn get_optimizations() -> optimize::Optimizations {
    optimize::snapshot()
}

#[tauri::command]
fn set_optimization(id: String, optimised: bool) -> Result<optimize::Optimizations, String> {
    optimize::set_tweak(&id, optimised)?;
    Ok(optimize::snapshot())
}

#[tauri::command]
fn set_power_plan(plan: String) -> Result<optimize::Optimizations, String> {
    optimize::set_power_plan(&plan)?;
    Ok(optimize::snapshot())
}

#[tauri::command]
fn set_admin_tweak(id: String, optimised: bool) -> Result<(), String> {
    optimize::set_admin_tweak(&id, optimised)
}

#[tauri::command]
fn run_cleanup(id: String) -> Result<(), String> {
    optimize::run_cleanup(&id)
}

#[tauri::command]
fn launch_tool(target: String) -> Result<(), String> {
    optimize::launch(&target)
}

#[tauri::command]
fn open_repo_url(url: String) -> Result<(), String> {
    const REPO: &str = "https://github.com/Boots3453/Syntax";

    if !url.starts_with(REPO) {
        return Err("that link doesn't point at the project's repository".into());
    }
    optimize::open_external(&url)
}

#[tauri::command]
fn begin_position_capture(state: State<AppState>) {
    state.automator.set_running(false);
    state.capture.arm_position();
}

const DEFAULT_WINDOW: (f64, f64) = (980.0, 600.0);

#[tauri::command]
fn reset_window_size(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let (width, height) = DEFAULT_WINDOW;

    {
        let mut settings = state.settings.lock().unwrap();
        settings.window_width = 0.0;
        settings.window_height = 0.0;
        let _ = settings::save(&app, &settings);
    }

    let window = app
        .get_webview_window("main")
        .ok_or("the main window has gone")?;

    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    let _ = window.center();
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub recording: bool,
    pub events: usize,
    pub seconds: f64,
}

#[tauri::command]
fn start_recording(state: State<AppState>, with_moves: bool) -> RecordingStatus {

    state.automator.set_running(false);
    state.recorder.start(with_moves);
    recording_status(state)
}

#[tauri::command]
fn stop_recording(state: State<AppState>) -> Vec<automation::Step> {
    state.recorder.stop()
}

#[tauri::command]
fn recording_status(state: State<AppState>) -> RecordingStatus {
    RecordingStatus {
        recording: state.recorder.is_recording(),
        events: state.recorder.count(),
        seconds: state.recorder.seconds(),
    }
}

#[tauri::command]
fn sample_pixel(x: i32, y: i32) -> Option<u32> {
    win32::screen_pixel(x, y)
        .map(|(r, g, b)| ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

#[tauri::command]
fn cursor_position() -> (i32, i32) {
    win32::cursor_position()
}

#[tauri::command]
fn set_native_cursor(width: i32, height: i32, hotspot_x: u32, hotspot_y: u32, rgba: Vec<u8>) -> bool {
    win32::set_cursor_from_rgba(width, height, &rgba, hotspot_x, hotspot_y)
}

#[tauri::command]
fn clear_native_cursor() {
    win32::clear_custom_cursor();
}

#[tauri::command]
fn set_theme_tint(app: AppHandle, state: State<AppState>, light: bool) {
    state.light_theme.store(light, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window("main") {
        let settings = state.settings.lock().unwrap().clone();
        decorate_window(&window, &settings, light);
    }
}

#[tauri::command]
fn peek_target() -> TargetInfo {
    engine::peek_target()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowEntry {
    title: String,
    process: String,

    raw_input: bool,
}

#[tauri::command]
fn list_windows() -> Vec<WindowEntry> {
    win32::list_windows()
        .into_iter()
        .map(|(hwnd, title, process)| WindowEntry {
            title,
            process,
            raw_input: win32::ignores_posted_input(hwnd),
        })
        .collect()
}

#[tauri::command]
fn to_client_point(title: String, process: String, x: i32, y: i32) -> Option<(i32, i32)> {
    let hwnd = win32::find_window(&title, &process)?;
    Some(win32::screen_to_client(hwnd, x, y))
}

#[tauri::command]
fn describe_sequence(text: String) -> Vec<String> {
    sequence::parse(&text).iter().map(Step::label).collect()
}

#[tauri::command]
fn begin_capture(state: State<AppState>) {
    state.clickers.stop_all();
    state.capture.arm();
}

#[tauri::command]
fn cancel_capture(state: State<AppState>) {
    state.capture.cancel();
}

#[tauri::command]
fn window_minimize(window: WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_close(window: WebviewWindow) {
    let _ = window.hide();
}

fn restore_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Syntax", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .tooltip("Syntax")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => restore_window(app),
            "quit" => {

                if let Some(state) = app.try_state::<AppState>() {
                    state.clickers.shutdown();
                    state.automator.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                restore_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[cfg(windows)]
fn decorate_window(window: &WebviewWindow, settings: &Settings, light: bool) {
    let Ok(handle) = window.hwnd() else {
        return;
    };
    let hwnd = win32::root_window(handle.0 as isize);

    win32::remember_own_window(hwnd);
    win32::install_cursor(hwnd);

    win32::suppress_non_client_area(hwnd);
    win32::strip_caption(hwnd);
    win32::disable_nc_rendering(hwnd);
    let tint = frame_tint(settings, light);
    win32::blend_frame_colors(hwnd, tint);

    let alpha = (settings.opacity.clamp(0.25, 1.0) * 235.0) as u8;
    win32::apply_blur(hwnd, settings.blur_enabled, settings.acrylic, tint, alpha);

    win32::prefer_rounded_corners(hwnd);
}

#[cfg(not(windows))]
fn decorate_window(_window: &WebviewWindow, _settings: &Settings, _light: bool) {}

#[cfg(windows)]
fn reassert_frame(window: &tauri::Window) {
    let Ok(handle) = window.hwnd() else {
        return;
    };
    let hwnd = win32::root_window(handle.0 as isize);

    win32::suppress_non_client_area(hwnd);
    win32::strip_caption(hwnd);
    win32::disable_nc_rendering(hwnd);
    win32::prefer_rounded_corners(hwnd);
}

#[cfg(not(windows))]
fn reassert_frame(_window: &tauri::Window) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    if !win32::claim_single_instance("BootsAutoClicker_single_instance") {
        eprintln!(
            "Syntax is already running — look for it in the system tray, \
             or end Syntax.exe in Task Manager."
        );
        return;
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .on_window_event(|window, event| {
            // Only the main window hides instead of closing, and only the main
            // window has a frame worth reasserting. The overlay must be free to
            // actually close, or switching it off would only hide it.
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                return;
            }

            if matches!(
                event,
                tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Focused(_)
                    | tauri::WindowEvent::ThemeChanged(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                reassert_frame(window);
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let initial = settings::load(&handle);

            win32::install_mouse_hook();

            if initial.window_width > 0.0 && initial.window_height > 0.0 {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.set_size(tauri::LogicalSize::new(
                        initial.window_width,
                        initial.window_height,
                    ));
                    let _ = window.center();
                }
            }

            let clickers = Arc::new(Clickers::new(&initial.profiles));
            let shared = Arc::new(Shared::new(&initial));
            let automator = Arc::new(Automator::new(initial.automation.clone()));
            let capture = Arc::new(Capture::new());
            let angler = fisher::Fisher::new(initial.fisher.clone());
            let sweets = gumdrop::Gumdrop::new(initial.gumdrop.clone());
            let looter = skywars::Skywars::new(initial.skywars.clone());
            let miner = davey::Davey::new(initial.davey.clone());
            let bow = crossbow::Crossbow::new(initial.crossbow.clone());

            let record = Arc::new(recorder::Recorder::new());
            recorder::attach(Arc::clone(&record));
            let launched = Instant::now();

            // Only a first guess for the frame, before the webview is up to
            // tell us what Windows is set to. A theme that defers to Windows
            // starts dark and is corrected a moment later.
            let light = matches!(initial.theme.as_str(), "light" | "gradient-light");

            app.manage(AppState {
                clickers: Arc::clone(&clickers),
                shared: Arc::clone(&shared),
                automator: Arc::clone(&automator),
                capture: Arc::clone(&capture),
                recorder: Arc::clone(&record),
                fisher: Arc::clone(&angler),
                gumdrop: Arc::clone(&sweets),
                skywars: Arc::clone(&looter),
                davey: Arc::clone(&miner),
                crossbow: Arc::clone(&bow),
                settings: Mutex::new(initial.clone()),
                light_theme: AtomicBool::new(light),
                launched,
            });

            hotkeys::spawn(
                handle.clone(),
                clickers,
                shared,
                automator,
                capture,
                angler,
                sweets,
                looter,
                miner,
                bow,
                launched,
            );
            build_tray(&handle)?;

            // bring the overlay up if it was left switched on
            overlay::apply(&handle, &initial.overlay);

            match app.get_webview_window("main") {
                Some(window) => {

                    let _ = window.set_decorations(false);

                    let _ = window.set_title("");
                    let _ = window.set_always_on_top(initial.always_on_top);
                    decorate_window(&window, &initial, light);
                }
                None => eprintln!("no window labelled \"main\" — nothing decorated"),
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            apply_settings,
            get_status,
            set_active,
            toggle_active,
            stop_all,
            reset_clicks,
            reset_stats,
            get_automation,
            apply_automation,
            get_automation_status,
            toggle_automation,
            stop_automation,
            get_fisher,
            apply_fisher,
            get_fisher_status,
            toggle_fisher,
            stop_fisher,
            reset_fisher_counts,
            default_profile,
            get_gumdrop,
            apply_gumdrop,
            get_gumdrop_status,
            fire_gumdrop,
            get_skywars,
            apply_skywars,
            get_skywars_status,
            fire_skywars,
            get_davey,
            apply_davey,
            get_davey_status,
            fire_davey,
            get_crossbow,
            apply_crossbow,
            get_crossbow_status,
            macro_defaults,
            fire_crossbow,
            get_overlay,
            apply_overlay,
            export_code,
            import_code,
            describe_code,
            save_preset,
            delete_preset,
            begin_position_capture,
            cursor_position,
            sample_pixel,
            reset_window_size,
            start_recording,
            stop_recording,
            recording_status,
            set_native_cursor,
            clear_native_cursor,
            set_theme_tint,
            peek_target,
            list_windows,
            to_client_point,
            describe_sequence,
            begin_capture,
            cancel_capture,
            get_optimizations,
            set_optimization,
            set_power_plan,
            launch_tool,
            set_admin_tweak,
            run_cleanup,
            open_repo_url,
            window_minimize,
            window_close
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
