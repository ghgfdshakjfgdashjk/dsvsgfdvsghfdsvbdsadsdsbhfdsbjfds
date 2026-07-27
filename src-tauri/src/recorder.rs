use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::automation::Step;
use crate::win32;

const MIN_GAP_MS: f64 = 12.0;

const MAX_GAP_MS: f64 = 10_000.0;

enum Captured {

    Click { button: &'static str, x: i32, y: i32 },

    Key { vk: u32 },
    Scroll { amount: i32 },
}

struct Event {
    at: Instant,
    what: Captured,
}

pub struct Recorder {
    active: AtomicBool,

    with_moves: AtomicBool,
    events: Mutex<Vec<Event>>,

    count: AtomicUsize,
    started: Mutex<Option<Instant>>,
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            active: AtomicBool::new(false),
            with_moves: AtomicBool::new(true),
            events: Mutex::new(Vec::new()),
            count: AtomicUsize::new(0),
            started: Mutex::new(None),
        }
    }

    pub fn start(&self, with_moves: bool) {
        self.events.lock().unwrap().clear();
        self.count.store(0, Ordering::Relaxed);
        self.with_moves.store(with_moves, Ordering::Relaxed);
        *self.started.lock().unwrap() = Some(Instant::now());
        self.active.store(true, Ordering::Release);
        win32::install_keyboard_hook();
    }

    pub fn is_recording(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    pub fn seconds(&self) -> f64 {
        self.started
            .lock()
            .unwrap()
            .map(|at| at.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn stop(&self) -> Vec<Step> {
        self.active.store(false, Ordering::Release);
        win32::remove_keyboard_hook();

        let events = std::mem::take(&mut *self.events.lock().unwrap());
        self.count.store(0, Ordering::Relaxed);
        *self.started.lock().unwrap() = None;

        build_steps(events, self.with_moves.load(Ordering::Relaxed))
    }

    fn push(&self, what: Captured) {
        if !self.is_recording() {
            return;
        }
        let mut events = self.events.lock().unwrap();

        if events.len() >= 2000 {
            return;
        }
        events.push(Event {
            at: Instant::now(),
            what,
        });
        self.count.store(events.len(), Ordering::Relaxed);
    }
}

fn build_steps(events: Vec<Event>, with_moves: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut previous: Option<Instant> = None;

    let mut last_point: Option<(i32, i32)> = None;

    for event in events {
        if let Some(before) = previous {
            let gap = (event.at - before).as_secs_f64() * 1000.0;
            if gap >= MIN_GAP_MS {
                steps.push(Step::Wait {
                    ms: (gap.min(MAX_GAP_MS) * 10.0).round() / 10.0,
                });
            }
        }
        previous = Some(event.at);

        match event.what {
            Captured::Click { button, x, y } => {
                if with_moves && last_point != Some((x, y)) {
                    steps.push(Step::Move { x, y });
                    last_point = Some((x, y));
                }
                steps.push(Step::Click {
                    button: button.into(),
                    count: 1,
                });
            }
            Captured::Key { vk } => steps.push(Step::Key { vk }),
            Captured::Scroll { amount } => steps.push(Step::Scroll { amount }),
        }
    }

    steps
}

static RECORDER: Mutex<Option<std::sync::Arc<Recorder>>> = Mutex::new(None);

pub fn attach(recorder: std::sync::Arc<Recorder>) {
    *RECORDER.lock().unwrap() = Some(recorder);
}

fn with_recorder(f: impl FnOnce(&Recorder)) {

    if let Ok(guard) = RECORDER.try_lock() {
        if let Some(recorder) = guard.as_ref() {
            f(recorder);
        }
    }
}

pub fn is_recording() -> bool {
    let mut recording = false;
    with_recorder(|r| recording = r.is_recording());
    recording
}

pub fn on_mouse_up(button: &'static str, x: i32, y: i32) {

    if win32::point_over_own_app(x, y) {
        return;
    }
    with_recorder(|r| r.push(Captured::Click { button, x, y }));
}

pub fn on_scroll(amount: i32, x: i32, y: i32) {
    if win32::point_over_own_app(x, y) {
        return;
    }
    with_recorder(|r| r.push(Captured::Scroll { amount }));
}

pub fn on_key_up(vk: u32) {

    if win32::own_app_focused() {
        return;
    }
    with_recorder(|r| r.push(Captured::Key { vk }));
}
