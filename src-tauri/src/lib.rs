mod automation;
mod clickers;
mod engine;
mod hotkeys;
mod optimize;
mod recorder;
mod sequence;
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
    if settings.theme != "gradient" {
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
    const REPO: &str = "https://github.com/Boots3453/BootsAutoClicker";

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
    let show = MenuItem::with_id(app, "show", "Show BootsAutoClicker", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .tooltip("BootsAutoClicker")
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
            "BootsAutoClicker is already running — look for it in the system tray, \
             or end BootsAutoClicker.exe in Task Manager."
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

            let record = Arc::new(recorder::Recorder::new());
            recorder::attach(Arc::clone(&record));
            let launched = Instant::now();

            let light = initial.theme == "light";

            app.manage(AppState {
                clickers: Arc::clone(&clickers),
                shared: Arc::clone(&shared),
                automator: Arc::clone(&automator),
                capture: Arc::clone(&capture),
                recorder: Arc::clone(&record),
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
                launched,
            );
            build_tray(&handle)?;

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
