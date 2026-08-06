use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CrossbowSettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,
    pub crossbow_slot: u32,
    pub sword_slot: u32,
    /// Fire the tactical crossbow first, then the ordinary one, before going
    /// back to the sword. Off, only the ordinary one is used.
    pub tactical_enabled: bool,
    pub tactical_slot: u32,
    /// Used in place of `after_switch_ms` for the second shot only, so the
    /// crossbow can be given longer to come out after the tactical bow has
    /// fired without slowing down the first swap.
    pub second_switch_ms: u64,
    /// How long each slot number is held.
    pub key_hold_ms: u64,
    /// Time to let the crossbow actually come out before firing.
    pub after_switch_ms: u64,
    /// How long the mouse button is held for the shot. A game reads the
    /// button once a frame, so this has to outlast a frame to be seen.
    pub click_hold_ms: u64,
    /// Gap after the shot, before going back to the sword.
    pub after_click_ms: u64,
}

impl Default for CrossbowSettings {
    fn default() -> Self {
        CrossbowSettings {
            bind_enabled: false,
            bind_vk: 0,
            crossbow_slot: 4,
            sword_slot: 1,
            tactical_enabled: false,
            tactical_slot: 3,
            second_switch_ms: 120,
            key_hold_ms: 12,
            after_switch_ms: 40,
            click_hold_ms: 40,
            after_click_ms: 40,
        }
    }
}

impl CrossbowSettings {
    pub fn sanitised(mut self) -> Self {
        self.crossbow_slot = self.crossbow_slot.clamp(1, 9);
        self.sword_slot = self.sword_slot.clamp(1, 9);
        self.tactical_slot = self.tactical_slot.clamp(1, 9);
        self.second_switch_ms = self.second_switch_ms.min(5_000);
        self.key_hold_ms = self.key_hold_ms.min(2_000);
        self.after_switch_ms = self.after_switch_ms.min(5_000);
        self.click_hold_ms = self.click_hold_ms.min(2_000);
        self.after_click_ms = self.after_click_ms.min(5_000);
        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossbowStatus {
    pub busy: bool,
    pub runs: u32,
}

struct Inner {
    settings: Mutex<CrossbowSettings>,
    asked: AtomicBool,
    busy: AtomicBool,
    runs: AtomicU32,
}

pub struct Crossbow {
    inner: Arc<Inner>,
}

impl Crossbow {
    pub fn new(settings: CrossbowSettings) -> Arc<Crossbow> {
        let inner = Arc::new(Inner {
            settings: Mutex::new(settings.sanitised()),
            asked: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            runs: AtomicU32::new(0),
        });

        let crossbow = Arc::new(Crossbow {
            inner: Arc::clone(&inner),
        });

        thread::Builder::new()
            .name("crossbow".into())
            .spawn(move || worker(inner))
            .expect("failed to spawn crossbow thread");

        crossbow
    }

    pub fn settings(&self) -> CrossbowSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn apply(&self, settings: CrossbowSettings) {
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

    pub fn status(&self) -> CrossbowStatus {
        CrossbowStatus {
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

fn click(hold: u64) {
    let spec = win32::button_spec("left");

    if hold == 0 {
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

/// Swap to a slot, wait `settle` for the weapon to come out, shoot, and pause.
fn shoot_from(slot: u32, settle: u64, settings: &CrossbowSettings) {
    tap(slot_key(slot), settings.key_hold_ms);
    wait(settle);

    click(settings.click_hold_ms);
    wait(settings.after_click_ms);
}

fn run_once(settings: &CrossbowSettings) {
    // The tactical one goes first, so the slower bolt is already in the air
    // when the second shot follows it.
    //
    // That second swap gets its own delay. Coming off a shot is not the same
    // as coming off a sword: the game is still finishing the first weapon's
    // animation, so the crossbow can take noticeably longer to be ready to
    // fire than it does from a standing start.
    let settle = if settings.tactical_enabled {
        shoot_from(settings.tactical_slot, settings.after_switch_ms, settings);
        settings.second_switch_ms
    } else {
        settings.after_switch_ms
    };

    shoot_from(settings.crossbow_slot, settle, settings);

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
