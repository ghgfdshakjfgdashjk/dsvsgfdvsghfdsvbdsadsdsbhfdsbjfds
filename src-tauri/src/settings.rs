use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::automation::AutomationSettings;
use crate::fisher::FisherSettings;
use crate::gumdrop::GumdropSettings;
use crate::crossbow::CrossbowSettings;
use crate::overlay::OverlaySettings;
use crate::davey::DaveySettings;
use crate::skywars::SkywarsSettings;

pub const CPS_CEILING: f64 = 50_000.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClickPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    pub name: String,

    pub enabled: bool,

    pub mode: String,

    pub bind_enabled: bool,

    pub bind_vk: u32,

    pub button: String,

    pub delivery: String,

    pub target_mode: String,

    pub target_title: String,

    pub target_process: String,

    pub target_x: f64,
    pub target_y: f64,

    pub points: Vec<ClickPoint>,

    pub rate_mode: String,
    pub cps_min: f64,
    pub cps_max: f64,
    pub randomize: bool,

    pub jitter: f64,

    pub duty_enabled: bool,

    pub duty_cycle: f64,

    pub precision: String,

    pub limit_enabled: bool,
    pub limit_count: u64,

    pub time_limit_enabled: bool,
    pub time_limit_secs: f64,

    pub start_delay_enabled: bool,
    pub start_delay_ms: f64,

    pub filter_enabled: bool,
    pub filter_title: String,

    pub sequence_enabled: bool,

    pub sequence: String,

    pub burst_enabled: bool,
    pub burst_count: u64,
    pub burst_pause_ms: f64,

    pub pixel_enabled: bool,
    pub pixel_x: f64,
    pub pixel_y: f64,

    pub pixel_rgb: u32,

    pub pixel_tolerance: f64,

    pub pixel_stop_on: String,

    /// Shake the view while this clicker runs, in first person only.
    pub shake_enabled: bool,
    /// How far each swing moves the mouse, in pixels. A locked cursor turns
    /// that into how far the camera goes.
    pub shake_px: f64,
    /// How long the camera stays swung out before it is brought back, which
    /// is also the pause before the next swing. Long enough for the game to
    /// draw a frame, or the pair cancels before anything is rendered.
    pub shake_ms: f64,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: "Clicker".into(),
            enabled: true,
            mode: "toggle".into(),
            bind_enabled: true,
            bind_vk: 0x75,
            button: "left".into(),
            delivery: "window".into(),
            target_mode: "cursor".into(),
            target_title: String::new(),
            target_process: String::new(),
            target_x: -1.0,
            target_y: -1.0,
            rate_mode: "cps".into(),
            cps_min: 8.0,
            cps_max: 25.0,
            randomize: false,
            jitter: 0.0,
            duty_enabled: false,
            duty_cycle: 25.0,
            precision: "balanced".into(),
            limit_enabled: false,
            limit_count: 1000,
            time_limit_enabled: false,
            time_limit_secs: 30.0,
            start_delay_enabled: false,
            start_delay_ms: 500.0,
            filter_enabled: false,
            filter_title: String::new(),
            burst_enabled: false,
            burst_count: 5,
            burst_pause_ms: 250.0,
            pixel_enabled: false,
            pixel_x: 0.0,
            pixel_y: 0.0,
            pixel_rgb: 0,
            pixel_tolerance: 8.0,
            pixel_stop_on: "change".into(),
            points: Vec::new(),
            sequence_enabled: false,
            sequence: String::new(),
            shake_enabled: false,
            shake_px: 15.0,
            shake_ms: 17.0,
        }
    }
}

impl Profile {
    pub fn sanitised(mut self) -> Self {
        if self.mode != "hold" {
            self.mode = "toggle".into();
        }
        if !matches!(
            self.button.as_str(),
            "left" | "right" | "middle" | "mouse4" | "mouse5"
        ) {
            self.button = "left".into();
        }
        if self.precision != "max" {
            self.precision = "balanced".into();
        }
        if self.delivery != "window" {
            self.delivery = "system".into();
        }
        if self.rate_mode != "delay" {
            self.rate_mode = "cps".into();
        }
        if !matches!(self.target_mode.as_str(), "cursor" | "focused" | "pinned") {
            self.target_mode = "cursor".into();
        }

        self.cps_min = self.cps_min.clamp(0.01, CPS_CEILING);
        self.cps_max = self.cps_max.clamp(0.01, CPS_CEILING);
        if self.cps_min > self.cps_max {
            std::mem::swap(&mut self.cps_min, &mut self.cps_max);
        }
        self.jitter = self.jitter.clamp(0.0, 95.0);
        self.duty_cycle = self.duty_cycle.clamp(0.0, 95.0);
        if self.limit_count == 0 {
            self.limit_count = 1;
        }

        if !self.time_limit_secs.is_finite() {
            self.time_limit_secs = 30.0;
        }
        self.time_limit_secs = self.time_limit_secs.clamp(0.1, 86_400.0);
        if !self.start_delay_ms.is_finite() {
            self.start_delay_ms = 500.0;
        }
        self.start_delay_ms = self.start_delay_ms.clamp(0.0, 60_000.0);
        if self.name.trim().is_empty() {
            self.name = "Clicker".into();
        }

        self.burst_count = self.burst_count.max(1);
        if !self.burst_pause_ms.is_finite() {
            self.burst_pause_ms = 250.0;
        }
        self.burst_pause_ms = self.burst_pause_ms.clamp(0.0, 600_000.0);

        if !matches!(self.pixel_stop_on.as_str(), "change" | "match") {
            self.pixel_stop_on = "change".into();
        }
        if !self.pixel_tolerance.is_finite() {
            self.pixel_tolerance = 8.0;
        }
        self.pixel_tolerance = self.pixel_tolerance.clamp(0.0, 100.0);

        self.points.truncate(16);
        self.points.retain(|p| p.x.is_finite() && p.y.is_finite());

        if !self.shake_px.is_finite() {
            self.shake_px = 15.0;
        }
        self.shake_px = self.shake_px.clamp(1.0, 400.0);

        if !self.shake_ms.is_finite() {
            self.shake_ms = 17.0;
        }
        self.shake_ms = self.shake_ms.clamp(1.0, 60_000.0);

        self
    }
}

/// A whole saved configuration, kept as a share code so saving and sharing
/// are the same thing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub code: String,
}

impl Default for Preset {
    fn default() -> Self {
        Preset {
            name: "Preset".into(),
            code: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub profiles: Vec<Profile>,

    pub selected: usize,

    pub panic_vk: u32,

    pub edge_guard_enabled: bool,
    pub edge_guard_px: f64,

    pub edge_guard_mode: String,

    pub edge_guard_chrome: bool,

    /// One of `gradient`, `gradient-dark`, `gradient-light`, `dark`, `light`.
    ///
    /// Two things in one string: whether there is a gradient, and whether it
    /// is light or dark. Plain `gradient` leaves the second to Windows; the
    /// other two say it outright, for anyone who does not want to change a
    /// system setting to change an app.
    pub theme: String,

    pub accent_hue: f64,

    pub accent_sat: f64,

    pub cursor_style: String,

    pub cursor_image: String,

    pub cursor_size: f64,

    pub window_width: f64,
    pub window_height: f64,

    pub always_on_top: bool,
    pub blur_enabled: bool,
    pub acrylic: bool,

    pub opacity: f64,

    pub automation: AutomationSettings,

    pub fisher: FisherSettings,

    pub gumdrop: GumdropSettings,

    pub skywars: SkywarsSettings,

    pub davey: DaveySettings,

    pub crossbow: CrossbowSettings,

    pub overlay: OverlaySettings,

    /// Saved configurations you can switch between.
    pub presets: Vec<Preset>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            profiles: vec![Profile::default()],
            selected: 0,
            panic_vk: 0x7B,
            edge_guard_enabled: false,
            edge_guard_px: 12.0,
            edge_guard_mode: "window".into(),
            edge_guard_chrome: true,
            theme: "gradient-dark".into(),
            accent_hue: 222.0,
            accent_sat: 100.0,
            cursor_style: "image".into(),
            cursor_image: String::new(),
            cursor_size: 26.0,
            window_width: 0.0,
            window_height: 0.0,
            always_on_top: false,
            blur_enabled: true,
            acrylic: false,
            opacity: 0.72,
            automation: AutomationSettings::default(),
            fisher: FisherSettings::default(),
            gumdrop: GumdropSettings::default(),
            skywars: SkywarsSettings::default(),
            davey: DaveySettings::default(),
            crossbow: CrossbowSettings::default(),
            overlay: OverlaySettings::default(),
            presets: Vec::new(),
        }
    }
}

impl Settings {

    pub fn sanitised(mut self) -> Self {
        self.profiles = self.profiles.into_iter().map(Profile::sanitised).collect();
        if self.profiles.is_empty() {
            self.profiles.push(Profile::default());
        }
        if self.selected >= self.profiles.len() {
            self.selected = self.profiles.len() - 1;
        }

        if !matches!(self.edge_guard_mode.as_str(), "window" | "screen") {
            self.edge_guard_mode = "window".into();
        }
        self.edge_guard_px = self.edge_guard_px.clamp(1.0, 200.0);

        for value in [&mut self.window_width, &mut self.window_height] {
            if !value.is_finite() || *value < 0.0 {
                *value = 0.0;
            }
        }
        if self.window_width > 0.0 {
            self.window_width = self.window_width.clamp(700.0, 6000.0);
        }
        if self.window_height > 0.0 {
            self.window_height = self.window_height.clamp(460.0, 4000.0);
        }

        if !matches!(
            self.theme.as_str(),
            "gradient" | "gradient-dark" | "gradient-light" | "dark" | "light"
        ) {
            self.theme = "gradient-dark".into();
        }
        if !self.accent_hue.is_finite() {
            self.accent_hue = 222.0;
        }
        self.accent_hue = self.accent_hue.rem_euclid(360.0);
        if !self.accent_sat.is_finite() {
            self.accent_sat = 100.0;
        }
        self.accent_sat = self.accent_sat.clamp(0.0, 100.0);

        if !matches!(
            self.cursor_style.as_str(),
            "image" | "dot" | "ring" | "cross" | "arrow" | "custom" | "system"
        ) {
            self.cursor_style = "image".into();
        }
        self.cursor_size = self.cursor_size.clamp(12.0, 64.0);

        if !self.cursor_image.starts_with("data:image/") {
            self.cursor_image.clear();
        }
        if self.cursor_style == "custom" && self.cursor_image.is_empty() {
            self.cursor_style = "image".into();
        }

        self.opacity = self.opacity.clamp(0.25, 1.0);
        self.fisher = self.fisher.sanitised();
        self.gumdrop = self.gumdrop.sanitised();
        self.skywars = self.skywars.sanitised();
        self.davey = self.davey.sanitised();
        self.crossbow = self.crossbow.sanitised();
        self.overlay = self.overlay.sanitised();

        self.presets = self
            .presets
            .into_iter()
            .map(|mut preset| {
                preset.name = preset.name.trim().chars().take(40).collect();
                if preset.name.is_empty() {
                    preset.name = "Preset".into();
                }
                preset
            })
            .filter(|preset| !preset.code.is_empty())
            .take(20)
            .collect();
        self
    }
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
}

/// v0.3.0 and earlier installed under a different identifier, so their
/// settings sit in a different folder. Read them once, then save into the
/// current one.
const LEGACY_DIRS: [&str; 1] = ["com.kylei.boots-autoclicker"];

fn legacy_paths(app: &AppHandle) -> Vec<PathBuf> {
    let Ok(dir) = app.path().app_config_dir() else {
        return Vec::new();
    };
    let Some(parent) = dir.parent() else {
        return Vec::new();
    };
    LEGACY_DIRS
        .iter()
        .map(|name| parent.join(name).join("settings.json"))
        .collect()
}

fn read_at(path: &PathBuf) -> Option<Settings> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Settings>(&text)
        .ok()
        .map(|parsed| parsed.sanitised())
}

pub fn load(app: &AppHandle) -> Settings {
    if let Some(path) = config_path(app) {
        if let Some(settings) = read_at(&path) {
            return settings;
        }
    }

    for path in legacy_paths(app) {
        if let Some(settings) = read_at(&path) {
            let _ = save(app, &settings);
            return settings;
        }
    }

    Settings::default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = config_path(app).ok_or_else(|| "no writable config directory".to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}
