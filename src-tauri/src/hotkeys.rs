use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::automation::Automator;
use crate::clickers::{Clickers, Shared};
use crate::engine::Status;
use crate::win32;

const VK_ESCAPE: u32 = 0x1B;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub clickers: Vec<Status>,

    pub running: usize,
    pub capturing: bool,

    pub total_clicks: u64,
    pub active_seconds: f64,

    pub cpu_percent: f64,
}

pub fn build_status(clickers: &Clickers, capturing: bool, launched: Instant) -> AppStatus {
    let statuses: Vec<Status> = clickers.snapshot().iter().map(|e| e.status()).collect();

    let running = statuses.iter().filter(|s| s.active).count();
    let total_clicks = statuses.iter().map(|s| s.total_clicks).sum();
    let active_seconds = statuses.iter().map(|s| s.active_seconds).sum();

    let wall = launched.elapsed().as_secs_f64().max(0.001);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0);

    AppStatus {
        clickers: statuses,
        running,
        capturing,
        total_clicks,
        active_seconds,
        cpu_percent: (win32::process_cpu_seconds() / wall / cores) * 100.0,
    }
}

pub struct Capture {
    armed: AtomicBool,

    settled: AtomicBool,

    position_mode: AtomicBool,
}

impl Capture {
    pub fn new() -> Self {
        Capture {
            armed: AtomicBool::new(false),
            settled: AtomicBool::new(false),
            position_mode: AtomicBool::new(false),
        }
    }

    pub fn arm(&self) {
        self.position_mode.store(false, Ordering::Relaxed);
        self.settled.store(false, Ordering::Relaxed);
        self.armed.store(true, Ordering::Relaxed);
    }

    pub fn arm_position(&self) {
        self.position_mode.store(true, Ordering::Relaxed);
        self.settled.store(false, Ordering::Relaxed);
        self.armed.store(true, Ordering::Relaxed);
    }

    pub fn cancel(&self) {
        self.armed.store(false, Ordering::Relaxed);
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Relaxed)
    }
}

pub fn spawn(
    app: AppHandle,
    clickers: Arc<Clickers>,
    shared: Arc<Shared>,
    automator: Arc<Automator>,
    capture: Arc<Capture>,
    fisher: Arc<crate::fisher::Fisher>,
    gumdrop: Arc<crate::gumdrop::Gumdrop>,
    skywars: Arc<crate::skywars::Skywars>,
    davey: Arc<crate::davey::Davey>,
    crossbow: Arc<crate::crossbow::Crossbow>,
    launched: Instant,
) {
    thread::Builder::new()
        .name("bind-watcher".into())
        .spawn(move || {

            let mut bind_was_down: Vec<bool> = Vec::new();
            let mut auto_was_down = false;
            let mut panic_was_down = false;
            let mut fish_was_down = false;
            let mut drop_was_down = false;
            let mut sky_was_down = false;
            let mut dvy_was_down = false;
            let mut bow_was_down = false;
            let mut last_auto_running = false;
            let mut last_fish_running = false;
            let mut last_push = Instant::now();
            let mut last_guard_check = Instant::now();
            let mut guard_state = false;
            let mut shake = crate::shake::Shake::new();

            let mut engines = clickers.snapshot();
            let mut roster = clickers.roster();

            let idle_tick = Duration::from_millis(5);
            let busy_tick = Duration::from_millis(1);
            let mut tick = busy_tick;

            let mut visible = true;
            let mut last_visibility_check = Instant::now();
            let mut last_overlay_check = Instant::now();

            loop {
                thread::sleep(tick);

                let current = clickers.roster();
                if current != roster {
                    roster = current;
                    engines = clickers.snapshot();
                }

                if clickers.is_closing() {
                    shake.rest();
                    break;
                }
                bind_was_down.resize(engines.len(), false);

                if capture.is_armed() {
                    if !capture.settled.load(Ordering::Relaxed) {
                        if win32::scan_first_pressed().is_none() {
                            capture.settled.store(true, Ordering::Relaxed);
                        }
                    } else if let Some(vk) = win32::scan_first_pressed() {

                        let position = win32::cursor_position();
                        let picking = capture.position_mode.load(Ordering::Relaxed);
                        capture.cancel();

                        while win32::bind_held(vk) {
                            thread::sleep(Duration::from_millis(4));
                        }

                        if vk == VK_ESCAPE {
                            let _ = app.emit("bind-cancelled", ());
                        } else if picking {
                            let _ = app.emit("position-captured", position);
                        } else {
                            let _ = app.emit("bind-captured", vk);
                        }
                    }
                    for was in bind_was_down.iter_mut() {
                        *was = true;
                    }
                    auto_was_down = true;
                    fish_was_down = true;
                    drop_was_down = true;
                    sky_was_down = true;
                    dvy_was_down = true;
                    bow_was_down = true;
                    continue;
                }

                let margin = shared.guard_px.load(Ordering::Relaxed) as i32;
                let any_active = engines.iter().any(|e| e.is_active());

                if margin > 0 && any_active {
                    if last_guard_check.elapsed() >= Duration::from_millis(40) {
                        last_guard_check = Instant::now();

                        let blocked = if win32::cursor_over_own_app() {
                            false
                        } else {
                            let near_edge = if shared.guard_screen.load(Ordering::Relaxed) {
                                win32::cursor_near_screen_edge(margin)
                            } else {
                                win32::cursor_near_window_edge(margin)
                            };
                            let on_chrome = shared.guard_chrome.load(Ordering::Relaxed)
                                && win32::cursor_over_window_chrome();
                            near_edge || on_chrome
                        };

                        if blocked != guard_state {
                            guard_state = blocked;
                            let _ = app.emit("guard-tripped", blocked);
                        }
                        for engine in &engines {
                            engine.set_guard_blocked(blocked);
                        }
                    }
                } else if guard_state {
                    guard_state = false;
                    for engine in &engines {
                        engine.set_guard_blocked(false);
                    }
                }

                let mut changed = false;
                for (index, engine) in engines.iter().enumerate() {
                    let bind_vk = engine.bind_vk();
                    let down = win32::bind_held(bind_vk);
                    let was = bind_was_down[index];

                    if engine.hold_mode() {

                        if !down {
                            engine.clear_limit_latch();
                        }
                        let want = down && !engine.limit_latched();
                        if want != engine.is_active() {
                            engine.set_active(want);
                            changed = true;
                        }
                    } else if down && !was {
                        engine.clear_limit_latch();
                        engine.toggle();
                        changed = true;
                    }

                    bind_was_down[index] = down;
                }

                let auto_vk = automator.bind_vk();
                let auto_down = win32::bind_held(auto_vk);
                if auto_down && !auto_was_down {
                    automator.toggle();
                }
                auto_was_down = auto_down;

                let auto_running = automator.is_running();
                if auto_running != last_auto_running {
                    last_auto_running = auto_running;
                    let _ = app.emit("automation-changed", auto_running);
                }

                let fish_vk = fisher.bind_vk();
                let fish_down = win32::bind_held(fish_vk);
                if fish_down && !fish_was_down {
                    fisher.toggle();
                }
                fish_was_down = fish_down;

                let fish_running = fisher.is_running();
                if fish_running != last_fish_running {
                    last_fish_running = fish_running;
                    let _ = app.emit("fisher-changed", fish_running);
                }

                let drop_vk = gumdrop.bind_vk();
                let drop_down = win32::bind_held(drop_vk);
                if drop_down && !drop_was_down {
                    gumdrop.fire();
                    let _ = app.emit("gumdrop-status", gumdrop.status());
                }
                drop_was_down = drop_down;

                let sky_vk = skywars.bind_vk();
                let sky_down = win32::bind_held(sky_vk);
                if sky_down && !sky_was_down {
                    skywars.fire();
                    let _ = app.emit("skywars-status", skywars.status());
                }
                sky_was_down = sky_down;

                let dvy_vk = davey.bind_vk();
                let dvy_down = dvy_vk != 0 && win32::bind_held(dvy_vk);
                if dvy_down && !dvy_was_down {
                    davey.fire();
                    let _ = app.emit("davey-status", davey.status());
                }
                dvy_was_down = dvy_down;

                let bow_vk = crossbow.bind_vk();
                let bow_down = bow_vk != 0 && win32::bind_held(bow_vk);
                if bow_down && !bow_was_down {
                    crossbow.fire();
                    let _ = app.emit("crossbow-status", crossbow.status());
                }
                bow_was_down = bow_down;

                let panic_vk = shared.panic_vk.load(Ordering::Relaxed);
                let panic_down = win32::bind_held(panic_vk);
                if panic_down && !panic_was_down {
                    clickers.stop_all();
                    automator.set_running(false);
                    fisher.set_running(false);
                }
                panic_was_down = panic_down;

                tick = if capture.is_armed()
                    || auto_running
                    || fish_running
                    || engines.iter().any(|e| e.is_active())
                {
                    busy_tick
                } else {
                    idle_tick
                };

                // Only the clicker you are actually running gets to move the
                // pointer, so two of them can never fight over it.
                match engines.iter().find(|e| e.is_active()) {
                    Some(live) => shake.tick(true, &live.settings()),
                    None => shake.rest(),
                }

                if last_visibility_check.elapsed() >= Duration::from_millis(500) {
                    last_visibility_check = Instant::now();

                    let main_up = app
                        .get_webview_window("main")
                        .map(|w| {
                            w.is_visible().unwrap_or(true) && !w.is_minimized().unwrap_or(false)
                        })
                        .unwrap_or(false);

                    // the overlay needs the same stream of numbers, and it is
                    // usually up precisely when the main window is not
                    visible = main_up || crate::overlay::is_up(&app);
                }

                // let the overlay come and go with whatever is in front
                if last_overlay_check.elapsed() >= Duration::from_millis(400)
                    && crate::overlay::is_up(&app)
                {
                    last_overlay_check = Instant::now();
                    crate::overlay::follow_foreground(&app, &win32::foreground_title());
                }

                if changed || (visible && last_push.elapsed() >= Duration::from_millis(120)) {
                    last_push = Instant::now();
                    let _ = app.emit(
                        "status",
                        build_status(&clickers, capture.is_armed(), launched),
                    );
                    let _ = app.emit("automation-status", automator.status());
                    let _ = app.emit("fisher-status", fisher.status());
                    let _ = app.emit("gumdrop-status", gumdrop.status());
                    let _ = app.emit("skywars-status", skywars.status());
                }
            }
        })
        .expect("failed to spawn bind watcher thread");
}
