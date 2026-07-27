use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::sequence;
use crate::win32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Step {

    Move { x: i32, y: i32 },
    Click { button: String, count: u32 },

    Key { vk: u32 },

    Text { value: String },
    Wait { ms: f64 },
    Scroll { amount: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutomationSettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,

    pub repeat: u32,

    pub step_delay_ms: f64,
    pub steps: Vec<Step>,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        AutomationSettings {
            bind_enabled: true,
            bind_vk: 0x76,
            repeat: 1,
            step_delay_ms: 40.0,
            steps: Vec::new(),
        }
    }
}

impl AutomationSettings {
    pub fn sanitised(mut self) -> Self {
        self.step_delay_ms = self.step_delay_ms.clamp(0.0, 60_000.0);
        for step in &mut self.steps {
            match step {
                Step::Click { button, count } => {
                    if !matches!(
                        button.as_str(),
                        "left" | "right" | "middle" | "mouse4" | "mouse5"
                    ) {
                        *button = "left".into();
                    }
                    *count = (*count).clamp(1, 10_000);
                }
                Step::Wait { ms } => *ms = ms.clamp(0.0, 600_000.0),
                Step::Scroll { amount } => *amount = (*amount).clamp(-50, 50),
                _ => {}
            }
        }
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStatus {
    pub running: bool,

    pub step: u32,
    pub pass: u32,
}

struct Inner {
    settings: Mutex<AutomationSettings>,
    generation: AtomicU64,
    running: AtomicBool,
    shutdown: AtomicBool,
    step: AtomicU32,
    pass: AtomicU32,
    bind_vk: AtomicU32,
    gate: Mutex<bool>,
    wake: Condvar,
}

pub struct Automator {
    inner: Arc<Inner>,
}

fn effective_bind(settings: &AutomationSettings) -> u32 {
    if settings.bind_enabled {
        settings.bind_vk
    } else {
        0
    }
}

impl Automator {
    pub fn new(settings: AutomationSettings) -> Self {
        let settings = settings.sanitised();
        let inner = Arc::new(Inner {
            bind_vk: AtomicU32::new(effective_bind(&settings)),
            settings: Mutex::new(settings),
            generation: AtomicU64::new(1),
            running: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            step: AtomicU32::new(0),
            pass: AtomicU32::new(0),
            gate: Mutex::new(false),
            wake: Condvar::new(),
        });

        let worker_state = Arc::clone(&inner);
        thread::Builder::new()
            .name("automation".into())
            .spawn(move || worker(worker_state))
            .expect("failed to spawn automation thread");

        Automator { inner }
    }

    pub fn settings(&self) -> AutomationSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn update_settings(&self, settings: AutomationSettings) -> AutomationSettings {
        let settings = settings.sanitised();
        self.inner
            .bind_vk
            .store(effective_bind(&settings), Ordering::Relaxed);
        *self.inner.settings.lock().unwrap() = settings.clone();
        self.inner.generation.fetch_add(1, Ordering::AcqRel);

        self.set_running(false);
        settings
    }

    pub fn set_running(&self, on: bool) {
        let previous = self.inner.running.swap(on, Ordering::AcqRel);
        if previous == on {
            return;
        }
        if on {
            self.inner.step.store(0, Ordering::Relaxed);
            self.inner.pass.store(0, Ordering::Relaxed);
        }
        let mut flag = self.inner.gate.lock().unwrap();
        *flag = on;
        self.inner.wake.notify_all();
    }

    pub fn toggle(&self) -> bool {
        let next = !self.is_running();
        self.set_running(next);
        next
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    #[inline]
    pub fn bind_vk(&self) -> u32 {
        self.inner.bind_vk.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> AutomationStatus {
        AutomationStatus {
            running: self.is_running(),
            step: self.inner.step.load(Ordering::Relaxed),
            pass: self.inner.pass.load(Ordering::Relaxed),
        }
    }

    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        self.set_running(false);
    }
}

fn interruptible_sleep(inner: &Inner, duration: Duration, generation: u64) {
    const SLICE: Duration = Duration::from_millis(20);
    let mut remaining = duration;
    while remaining > Duration::ZERO {
        if !inner.running.load(Ordering::Relaxed)
            || inner.shutdown.load(Ordering::Relaxed)
            || inner.generation.load(Ordering::Relaxed) != generation
        {
            return;
        }
        let slice = remaining.min(SLICE);
        thread::sleep(slice);
        remaining = remaining.saturating_sub(slice);
    }
}

fn run_step(step: &Step, inner: &Inner, generation: u64) {
    match step {
        Step::Move { x, y } => win32::move_cursor(*x, *y),

        Step::Click { button, count } => {
            let spec = win32::button_spec(button);
            let burst = win32::build_burst(&spec, 1);
            for index in 0..*count {
                if !inner.running.load(Ordering::Relaxed) {
                    return;
                }
                win32::send_inputs(&burst);

                if index + 1 < *count {
                    interruptible_sleep(inner, Duration::from_millis(25), generation);
                }
            }
        }

        Step::Key { vk } => {
            let key = *vk as u16;
            win32::send_inputs(&[win32::key_event(key, false)]);
            thread::sleep(Duration::from_millis(15));
            win32::send_inputs(&[win32::key_event(key, true)]);
        }

        Step::Text { value } => {

            for parsed in sequence::parse(value) {
                if !inner.running.load(Ordering::Relaxed) {
                    return;
                }
                win32::send_inputs(&parsed.full());
                thread::sleep(Duration::from_millis(12));
            }
        }

        Step::Wait { ms } => {
            interruptible_sleep(inner, Duration::from_secs_f64(ms / 1000.0), generation)
        }

        Step::Scroll { amount } => {
            win32::send_inputs(&[win32::wheel_event(*amount)]);
        }
    }
}

fn worker(inner: Arc<Inner>) {
    loop {
        if inner.shutdown.load(Ordering::Relaxed) {
            break;
        }

        if !inner.running.load(Ordering::Acquire) {
            let guard = inner.gate.lock().unwrap();
            let _ = inner
                .wake
                .wait_timeout(guard, Duration::from_millis(150))
                .unwrap();
            continue;
        }

        let settings = inner.settings.lock().unwrap().clone();
        let generation = inner.generation.load(Ordering::Acquire);

        if settings.steps.is_empty() {
            inner.running.store(false, Ordering::Release);
            continue;
        }

        let gap = Duration::from_secs_f64(settings.step_delay_ms / 1000.0);
        let infinite = settings.repeat == 0;
        let mut pass = 0u32;

        'playback: while inner.running.load(Ordering::Relaxed)
            && !inner.shutdown.load(Ordering::Relaxed)
            && inner.generation.load(Ordering::Relaxed) == generation
        {
            inner.pass.store(pass + 1, Ordering::Relaxed);

            for (index, step) in settings.steps.iter().enumerate() {
                if !inner.running.load(Ordering::Relaxed)
                    || inner.generation.load(Ordering::Relaxed) != generation
                {
                    break 'playback;
                }
                inner.step.store(index as u32, Ordering::Relaxed);
                run_step(step, &inner, generation);
                if !gap.is_zero() {
                    interruptible_sleep(&inner, gap, generation);
                }
            }

            pass += 1;
            if !infinite && pass >= settings.repeat {
                break;
            }
        }

        inner.running.store(false, Ordering::Release);
        inner.step.store(0, Ordering::Relaxed);
    }
}
