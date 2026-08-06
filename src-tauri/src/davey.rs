use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

/// X, which is what this holds unless you change it.
const DEFAULT_HOLD_VK: u32 = 0x58;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DaveySettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,
    /// The key held down for the whole run.
    pub hold_vk: u32,
    /// How long the key stays down, and so when the clicking starts. The
    /// launch takes 25 frames of a 60fps clock, which is 417 ms.
    pub hold_ms: u64,
    pub pickaxe_slot: u32,
    /// How long the slot number is held.
    pub key_hold_ms: u64,
    /// How fast to click once the key is released.
    pub burst_cps: f64,
    /// How long to keep clicking for.
    pub burst_ms: u64,
    /// Share of each click the button spends held down. A game that reads
    /// the button once a frame can miss a press that goes down and up
    /// between two of its frames, so this keeps it down long enough to be
    /// seen.
    pub burst_duty: f64,
}

impl Default for DaveySettings {
    fn default() -> Self {
        DaveySettings {
            bind_enabled: false,
            bind_vk: 0,
            hold_vk: DEFAULT_HOLD_VK,
            hold_ms: 417,
            pickaxe_slot: 2,
            key_hold_ms: 12,
            burst_cps: 250.0,
            burst_ms: 500,
            burst_duty: 50.0,
        }
    }
}

impl DaveySettings {
    pub fn sanitised(mut self) -> Self {
        if self.hold_vk == 0 || self.hold_vk > 0xFF {
            self.hold_vk = DEFAULT_HOLD_VK;
        }
        self.hold_ms = self.hold_ms.min(10_000);
        self.pickaxe_slot = self.pickaxe_slot.clamp(1, 9);
        self.key_hold_ms = self.key_hold_ms.min(2_000);
        if !self.burst_cps.is_finite() {
            self.burst_cps = 250.0;
        }
        self.burst_cps = self.burst_cps.clamp(1.0, 50_000.0);
        self.burst_ms = self.burst_ms.min(10_000);
        if !self.burst_duty.is_finite() {
            self.burst_duty = 50.0;
        }
        self.burst_duty = self.burst_duty.clamp(5.0, 95.0);
        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaveyStatus {
    pub busy: bool,
    pub runs: u32,
}

struct Inner {
    settings: Mutex<DaveySettings>,
    asked: AtomicBool,
    busy: AtomicBool,
    runs: AtomicU32,
}

pub struct Davey {
    inner: Arc<Inner>,
}

impl Davey {
    pub fn new(settings: DaveySettings) -> Arc<Davey> {
        let inner = Arc::new(Inner {
            settings: Mutex::new(settings.sanitised()),
            asked: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            runs: AtomicU32::new(0),
        });

        let davey = Arc::new(Davey {
            inner: Arc::clone(&inner),
        });

        thread::Builder::new()
            .name("davey".into())
            .spawn(move || worker(inner))
            .expect("failed to spawn davey thread");

        davey
    }

    pub fn settings(&self) -> DaveySettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn apply(&self, settings: DaveySettings) {
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

    pub fn status(&self) -> DaveyStatus {
        DaveyStatus {
            busy: self.inner.busy.load(Ordering::Acquire),
            runs: self.inner.runs.load(Ordering::Relaxed),
        }
    }
}

/// Windows overshoots a short sleep by a whole timer tick, which at these
/// lengths is most of the delay. Spin for the short ones.
fn wait(ms: u64) {
    if ms == 0 {
        return;
    }

    let span = Duration::from_millis(ms);
    if span < Duration::from_micros(1500) {
        let until = Instant::now() + span;
        while Instant::now() < until {
            std::hint::spin_loop();
        }
    } else {
        thread::sleep(span);
    }
}

fn tap(vk: u16, hold: u64) {
    if hold == 0 {
        win32::send_inputs(&[win32::key_event(vk, false), win32::key_event(vk, true)]);
        return;
    }

    win32::send_inputs(&[win32::key_event(vk, false)]);
    wait(hold);
    win32::send_inputs(&[win32::key_event(vk, true)]);
}

fn slot_key(slot: u32) -> u16 {
    (0x30 + slot.clamp(1, 9)) as u16
}

/// Wait until `ms` after `from`, or return at once if that moment has gone.
fn hold_until(from: Instant, ms: u64) {
    let mark = Duration::from_millis(ms);
    if let Some(left) = mark.checked_sub(from.elapsed()) {
        wait(left.as_millis() as u64);
    }
}

/// Spin until a moment, without overshooting it the way sleeping would.
fn spin_to(mark: Instant) {
    while Instant::now() < mark {
        std::hint::spin_loop();
    }
}

/// Click flat out for a stretch of time, starting the instant it is called.
///
/// Anything passed as `lead` rides along with the very first press in one
/// batch, so there is no gap between the two -- not even a system call.
///
/// Each click holds the button down for a share of its slot rather than
/// pressing and releasing in the same instant, so a game reading the button
/// once a frame still finds it down. Every click is due at a fixed offset
/// from the start rather than "now plus a gap", so one that runs late does
/// not push the rest back -- and it never banks debt, so falling behind
/// cannot turn this into an uncapped loop.
fn burst(cps: f64, ms: u64, duty: f64, lead: &[win32::INPUT]) {
    if ms == 0 || cps <= 0.0 {
        // still hand back whatever was riding along
        if !lead.is_empty() {
            win32::send_inputs(lead);
        }
        return;
    }

    let spec = win32::button_spec("left");
    let period = Duration::from_secs_f64(1.0 / cps);
    let down_for = period.mul_f64((duty / 100.0).clamp(0.05, 0.95));

    let start = Instant::now();
    let finish = start + Duration::from_millis(ms);
    let mut due = start;
    let mut first = true;

    while Instant::now() < finish {
        if first {
            // whatever came before goes out together with the first press,
            // so nothing at all sits between them
            let mut batch = lead.to_vec();
            batch.push(win32::mouse_event(&spec, spec.down));
            win32::send_inputs(&batch);
            first = false;
        } else {
            win32::send_inputs(&[win32::mouse_event(&spec, spec.down)]);
        }

        spin_to((due + down_for).min(finish));
        win32::send_inputs(&[win32::mouse_event(&spec, spec.up)]);

        due += period;
        let now = Instant::now();
        if due < now {
            due = now;
        }

        spin_to(due.min(finish));
    }
}

fn run_once(settings: &DaveySettings) {
    let hold_vk = settings.hold_vk as u16;
    let started = Instant::now();

    // Down, and it stays down.
    win32::send_inputs(&[win32::key_event(hold_vk, false)]);

    // Swap to the pickaxe straight away, while it is still held.
    tap(slot_key(settings.pickaxe_slot), settings.key_hold_ms);

    // Hold out the rest of the time, measured from the press so the swap
    // does not push it back.
    hold_until(started, settings.hold_ms);

    // Let go and start clicking in the same breath: the release travels in
    // the same batch as the first click, so there is nothing in between.
    burst(
        settings.burst_cps,
        settings.burst_ms,
        settings.burst_duty,
        &[win32::key_event(hold_vk, true)],
    );
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
