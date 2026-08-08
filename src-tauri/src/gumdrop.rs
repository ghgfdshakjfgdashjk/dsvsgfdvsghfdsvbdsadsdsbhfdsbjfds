use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

/// A delay, kept sane and kept fractional.
///
/// These are held as fractions rather than whole milliseconds so the speed
/// control can scale them and scale them back without drift. Halving 7 gives
/// 3.5, not 4, so doubling it returns the 7 you started with. Only the wait
/// itself rounds, and it rounds once, at the end.
fn clamp_ms(value: f64, cap: f64) -> f64 {
    if !value.is_finite() || value < 0.0 {
        return 0.0;
    }
    value.min(cap)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GumdropSettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,
    pub gumdrop_slot: u32,
    pub pickaxe_slot: u32,
    pub sword_slot: u32,
    pub key_hold_ms: f64,
    pub click_hold_ms: f64,
    pub after_gumdrop_ms: f64,
    pub place_wait_ms: f64,
    pub after_pickaxe_ms: f64,
    pub after_break_ms: f64,
    /// Where the speed control was left, so it reads right next time.
    ///
    /// Nothing here uses it. Moving it rewrites the delays above directly, so
    /// they are already the real numbers by the time they arrive.
    pub speed: f64,
}

impl Default for GumdropSettings {
    fn default() -> Self {
        GumdropSettings {
            bind_enabled: false,
            bind_vk: 0,
            gumdrop_slot: 4,
            pickaxe_slot: 2,
            sword_slot: 1,
            // Slower than it needs to be, on purpose. The full swap-place-
            // swap-break-swap run is hard to time by eye, and a first run
            // that works and can be sped up beats a fast one that misses a
            // step and leaves you holding the wrong item.
            key_hold_ms: 16.0,
            click_hold_ms: 16.0,
            after_gumdrop_ms: 28.0,
            place_wait_ms: 28.0,
            after_pickaxe_ms: 28.0,
            after_break_ms: 28.0,
            speed: 1.0,
        }
    }
}

impl GumdropSettings {
    pub fn sanitised(mut self) -> Self {
        self.gumdrop_slot = self.gumdrop_slot.clamp(1, 9);
        self.pickaxe_slot = self.pickaxe_slot.clamp(1, 9);
        self.sword_slot = self.sword_slot.clamp(1, 9);
        self.key_hold_ms = clamp_ms(self.key_hold_ms, 2000.0);
        self.click_hold_ms = clamp_ms(self.click_hold_ms, 2000.0);
        self.after_gumdrop_ms = clamp_ms(self.after_gumdrop_ms, 5000.0);
        self.place_wait_ms = clamp_ms(self.place_wait_ms, 5000.0);
        self.after_pickaxe_ms = clamp_ms(self.after_pickaxe_ms, 5000.0);
        self.after_break_ms = clamp_ms(self.after_break_ms, 5000.0);
        if !self.speed.is_finite() || self.speed <= 0.0 {
            self.speed = 1.0;
        }
        self.speed = self.speed.clamp(0.1, 10.0);
        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GumdropStatus {
    pub busy: bool,
    pub runs: u32,
}

struct Inner {
    settings: Mutex<GumdropSettings>,
    asked: AtomicBool,
    busy: AtomicBool,
    runs: AtomicU32,
}

pub struct Gumdrop {
    inner: Arc<Inner>,
}

impl Gumdrop {
    pub fn new(settings: GumdropSettings) -> Arc<Gumdrop> {
        let inner = Arc::new(Inner {
            settings: Mutex::new(settings.sanitised()),
            asked: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            runs: AtomicU32::new(0),
        });

        let gumdrop = Arc::new(Gumdrop {
            inner: Arc::clone(&inner),
        });

        thread::Builder::new()
            .name("gumdrop".into())
            .spawn(move || worker(inner))
            .expect("failed to spawn gumdrop thread");

        gumdrop
    }

    pub fn settings(&self) -> GumdropSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn apply(&self, settings: GumdropSettings) {
        *self.inner.settings.lock().unwrap() = settings.sanitised();
    }

    pub fn bind_vk(&self) -> u32 {
        let settings = self.inner.settings.lock().unwrap();
        if settings.bind_enabled {
            settings.bind_vk
        } else {
            0
        }
    }

    pub fn fire(&self) {
        if self.inner.busy.load(Ordering::Acquire) {
            return;
        }
        self.inner.asked.store(true, Ordering::Release);
    }

    pub fn status(&self) -> GumdropStatus {
        GumdropStatus {
            busy: self.inner.busy.load(Ordering::Acquire),
            runs: self.inner.runs.load(Ordering::Relaxed),
        }
    }
}

fn wait(ms: f64) {
    if !(ms > 0.0) {
        return;
    }

    let span = Duration::from_secs_f64(ms / 1000.0);
    if span < Duration::from_micros(1500) {
        let until = Instant::now() + span;
        while Instant::now() < until {
            std::hint::spin_loop();
        }
    } else {
        thread::sleep(span);
    }
}

fn tap(vk: u16, hold: f64) {
    if hold <= 0.0 {
        win32::send_inputs(&[win32::key_event(vk, false), win32::key_event(vk, true)]);
        return;
    }

    win32::send_inputs(&[win32::key_event(vk, false)]);
    wait(hold);
    win32::send_inputs(&[win32::key_event(vk, true)]);
}

fn click(hold: f64) {
    let spec = win32::button_spec("left");

    if hold <= 0.0 {
        win32::send_inputs(&[
            win32::mouse_event(&spec, spec.down),
            win32::mouse_event(&spec, spec.up),
        ]);
        return;
    }

    win32::send_inputs(&[win32::mouse_event(&spec, spec.down)]);
    wait(hold);
    win32::send_inputs(&[win32::mouse_event(&spec, spec.up)]);
}

fn slot_key(slot: u32) -> u16 {
    (0x30 + slot.clamp(1, 9)) as u16
}

fn run_once(settings: &GumdropSettings) {
    tap(slot_key(settings.gumdrop_slot), settings.key_hold_ms);
    wait(settings.after_gumdrop_ms);

    click(settings.click_hold_ms);
    wait(settings.place_wait_ms);

    tap(slot_key(settings.pickaxe_slot), settings.key_hold_ms);
    wait(settings.after_pickaxe_ms);

    click(settings.click_hold_ms);
    wait(settings.after_break_ms);

    tap(slot_key(settings.sword_slot), settings.key_hold_ms);
}

fn worker(inner: Arc<Inner>) {
    loop {
        if !inner.asked.swap(false, Ordering::AcqRel) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        inner.busy.store(true, Ordering::Release);
        let settings = inner.settings.lock().unwrap().clone();
        run_once(&settings);
        inner.runs.fetch_add(1, Ordering::Relaxed);
        inner.busy.store(false, Ordering::Release);
    }
}
