use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::sequence::{self, Step};
use crate::settings::Profile;
use crate::win32;

const MAX_BATCH_INPUTS: usize = 4096;

const BATCH_SECONDS: f64 = 0.05;

const MIN_BATCH_UNITS: usize = 8;

const MIN_SLICE_BALANCED: f64 = 0.001;
const MIN_SLICE_SPIN: f64 = 0.0002;

const MAX_CREDIT: f64 = 4096.0;

struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        Rng(seed | 1)
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    #[inline]
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    #[inline]
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

pub fn resolve_for(profile: &Profile) -> Option<win32::Target> {
    match profile.target_mode.as_str() {

        "focused" => win32::foreground_target(),

        "pinned" => {
            let hwnd = win32::find_window(&profile.target_title, &profile.target_process)?;
            let (cx, cy) = win32::client_center(hwnd);
            let x = if profile.target_x < 0.0 {
                cx
            } else {
                profile.target_x as i32
            };
            let y = if profile.target_y < 0.0 {
                cy
            } else {
                profile.target_y as i32
            };
            Some(win32::Target { hwnd, x, y })
        }

        _ => win32::target_under_cursor(),
    }
}

pub fn peek_target() -> TargetInfo {
    match win32::target_under_cursor() {
        Some(t) => TargetInfo {
            title: win32::window_title(t.hwnd),
            process: win32::window_process(t.hwnd),
            raw_input: win32::ignores_posted_input(t.hwnd),
        },
        None => TargetInfo::default(),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub title: String,
    pub process: String,

    pub raw_input: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub name: String,
    pub active: bool,
    pub clicks: u64,
    pub cps: f64,

    pub guarded: bool,

    pub total_clicks: u64,

    pub active_seconds: f64,

    pub target: String,
}

struct Inner {
    settings: Mutex<Profile>,

    generation: AtomicU64,
    active: AtomicBool,
    shutdown: AtomicBool,
    clicks: AtomicU64,

    total_clicks: AtomicU64,

    active_ms: AtomicU64,

    run_started: Mutex<Option<Instant>>,

    measured: AtomicU64,

    bind_vk: AtomicU32,
    hold_mode: AtomicBool,

    guard_blocked: AtomicBool,

    limit_latched: AtomicBool,

    target: Mutex<Option<win32::Target>>,
    target_title: Mutex<String>,
    gate: Mutex<bool>,
    wake: Condvar,
}

pub struct Engine {
    inner: Arc<Inner>,
}

fn effective_bind(profile: &Profile) -> u32 {
    if profile.enabled && profile.bind_enabled {
        profile.bind_vk
    } else {
        0
    }
}

impl Engine {
    pub fn new(profile: Profile) -> Self {
        let settings = profile.sanitised();
        let inner = Arc::new(Inner {
            bind_vk: AtomicU32::new(effective_bind(&settings)),
            hold_mode: AtomicBool::new(settings.mode == "hold"),
            guard_blocked: AtomicBool::new(false),
            limit_latched: AtomicBool::new(false),
            settings: Mutex::new(settings),
            generation: AtomicU64::new(1),
            active: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            clicks: AtomicU64::new(0),
            total_clicks: AtomicU64::new(0),
            active_ms: AtomicU64::new(0),
            run_started: Mutex::new(None),
            measured: AtomicU64::new(0),
            target: Mutex::new(None),
            target_title: Mutex::new(String::new()),
            gate: Mutex::new(false),
            wake: Condvar::new(),
        });

        let worker_state = Arc::clone(&inner);
        thread::Builder::new()
            .name("click-engine".into())
            .spawn(move || worker(worker_state))
            .expect("failed to spawn click engine thread");

        Engine { inner }
    }

    pub fn settings(&self) -> Profile {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn update_settings(&self, profile: Profile) -> Profile {
        let settings = profile.sanitised();
        self.inner
            .bind_vk
            .store(effective_bind(&settings), Ordering::Relaxed);
        self.inner
            .hold_mode
            .store(settings.mode == "hold", Ordering::Relaxed);
        *self.inner.settings.lock().unwrap() = settings.clone();
        self.inner.generation.fetch_add(1, Ordering::AcqRel);
        self.wake();
        settings
    }

    pub fn set_active(&self, on: bool) {
        let previous = self.inner.active.swap(on, Ordering::AcqRel);
        if previous == on {
            return;
        }
        if on {
            self.inner.clicks.store(0, Ordering::Relaxed);
            *self.inner.run_started.lock().unwrap() = Some(Instant::now());
            self.resolve_target();
        } else {
            self.inner.measured.store(0, Ordering::Relaxed);

            if let Some(started) = self.inner.run_started.lock().unwrap().take() {
                let elapsed = started.elapsed().as_millis() as u64;
                self.inner.active_ms.fetch_add(elapsed, Ordering::Relaxed);
            }
        }
        self.wake();
    }

    fn resolve_target(&self) {
        let profile = self.inner.settings.lock().unwrap().clone();
        if profile.delivery != "window" {
            *self.inner.target.lock().unwrap() = None;
            self.inner.target_title.lock().unwrap().clear();
            return;
        }

        let resolved = resolve_for(&profile);
        let title = resolved
            .map(|t| win32::window_title(t.hwnd))
            .unwrap_or_default();
        *self.inner.target.lock().unwrap() = resolved;
        *self.inner.target_title.lock().unwrap() = title;
    }

    pub fn toggle(&self) -> bool {
        let next = !self.is_active();
        self.set_active(next);
        next
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.inner.active.load(Ordering::Acquire)
    }

    #[inline]
    pub fn bind_vk(&self) -> u32 {
        self.inner.bind_vk.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn hold_mode(&self) -> bool {
        self.inner.hold_mode.load(Ordering::Relaxed)
    }

    pub fn set_guard_blocked(&self, blocked: bool) -> bool {
        self.inner.guard_blocked.swap(blocked, Ordering::AcqRel) != blocked
    }

    #[inline]
    pub fn is_guard_blocked(&self) -> bool {
        self.inner.guard_blocked.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn limit_latched(&self) -> bool {
        self.inner.limit_latched.load(Ordering::Relaxed)
    }

    pub fn clear_limit_latch(&self) {
        self.inner.limit_latched.store(false, Ordering::Relaxed);
    }

    pub fn reset_clicks(&self) {
        self.inner.clicks.store(0, Ordering::Relaxed);
    }

    pub fn status(&self) -> Status {
        let mut active_ms = self.inner.active_ms.load(Ordering::Relaxed);
        if let Some(started) = *self.inner.run_started.lock().unwrap() {
            active_ms += started.elapsed().as_millis() as u64;
        }

        Status {
            name: self.inner.settings.lock().unwrap().name.clone(),
            active: self.is_active(),
            clicks: self.inner.clicks.load(Ordering::Relaxed),
            cps: self.inner.measured.load(Ordering::Relaxed) as f64 / 1000.0,
            guarded: self.is_guard_blocked(),
            total_clicks: self.inner.total_clicks.load(Ordering::Relaxed),
            active_seconds: active_ms as f64 / 1000.0,
            target: self.inner.target_title.lock().unwrap().clone(),
        }
    }

    pub fn reset_stats(&self) {
        self.inner.total_clicks.store(0, Ordering::Relaxed);
        self.inner.active_ms.store(0, Ordering::Relaxed);
        if self.is_active() {
            *self.inner.run_started.lock().unwrap() = Some(Instant::now());
        }
    }

    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        self.inner.active.store(false, Ordering::Relaxed);
        self.wake();
    }

    fn wake(&self) {
        let mut flag = self.inner.gate.lock().unwrap();
        *flag = self.inner.active.load(Ordering::Relaxed);
        self.inner.wake.notify_all();
    }
}

fn spin_wait(duration: Duration) {
    let target = Instant::now() + duration;
    loop {
        let now = Instant::now();
        if now >= target {
            return;
        }
        let remaining = target - now;
        if remaining > Duration::from_micros(2000) {

            thread::sleep(remaining - Duration::from_micros(1500));
        } else {
            for _ in 0..64 {
                std::hint::spin_loop();
            }
        }
    }
}

fn wait_coarse(duration: Duration, spin: bool) {
    if duration.is_zero() {
        return;
    }
    if spin {
        spin_wait(duration);
    } else {
        thread::sleep(duration);
    }
}

const MAX_CATCHUP: Duration = Duration::from_millis(250);

fn settle(due: &mut Instant, spin: bool) {
    let now = Instant::now();

    if *due > now {
        sleep_exact(*due - now, spin);
    } else if now - *due > MAX_CATCHUP {

        *due = now;
    }
}

fn sleep_exact(duration: Duration, spin: bool) {
    if duration.is_zero() {
        return;
    }
    if spin || duration < Duration::from_micros(1200) {
        spin_wait(duration);
    } else {
        thread::sleep(duration);
    }
}

struct Plan {

    unit_len: usize,

    burst: Vec<win32::INPUT>,
    max_units: usize,

    step_io: Vec<(Vec<win32::INPUT>, Vec<win32::INPUT>)>,

    unit_msgs: Vec<sequence::Msg>,

    step_msgs: Vec<(Vec<sequence::Msg>, Vec<sequence::Msg>)>,

    emissions: Vec<sequence::Emission>,

    scripted: bool,

    empty: bool,

    dangling_inputs: Vec<win32::INPUT>,
    dangling_msgs: Vec<sequence::Msg>,
}

fn post_messages_at(hwnd: win32::HWND, x: i32, y: i32, msgs: &[sequence::Msg]) {
    let point = win32::pack_point(x, y);
    for msg in msgs {
        match msg {
            sequence::Msg::Mouse { msg, wparam } => {
                win32::post(hwnd, *msg, *wparam, point);
            }
            sequence::Msg::Key { msg, vk } => {
                let release = *msg == win32::WM_KEYUP;
                win32::post(hwnd, *msg, *vk as usize, win32::key_lparam(*vk, release));
            }
        }
    }
}

fn next_point(target: &win32::Target, extra: &[(i32, i32)], index: &mut usize) -> (i32, i32) {
    if extra.is_empty() {
        return (target.x, target.y);
    }

    let total = extra.len() + 1;
    let current = *index % total;
    *index = (current + 1) % total;

    if current == 0 {
        (target.x, target.y)
    } else {
        extra[current - 1]
    }
}

fn pixel_triggered(settings: &Profile) -> bool {
    let Some((r, g, b)) = win32::screen_pixel(settings.pixel_x as i32, settings.pixel_y as i32)
    else {

        return false;
    };

    let want = settings.pixel_rgb;
    let dr = r as i32 - ((want >> 16) & 0xFF) as i32;
    let dg = g as i32 - ((want >> 8) & 0xFF) as i32;
    let db = b as i32 - (want & 0xFF) as i32;

    let distance = dr.abs().max(dg.abs()).max(db.abs()) as f64 / 255.0 * 100.0;
    let matches = distance <= settings.pixel_tolerance;

    if settings.pixel_stop_on == "match" {
        matches
    } else {
        !matches
    }
}

fn build_plan(settings: &Profile) -> Plan {

    let scripting = settings.sequence_enabled;
    let steps: Vec<Step> = if scripting {
        sequence::parse(&settings.sequence)
    } else {
        sequence::single_click(&settings.button)
    };

    let empty = steps.is_empty();

    let unit = sequence::unit_inputs(&steps);
    let unit_len = unit.len().max(1);

    let ceiling = (MAX_BATCH_INPUTS / unit_len).max(1);
    let wanted = (settings.cps_max * BATCH_SECONDS).ceil() as usize;
    let max_units = wanted.max(MIN_BATCH_UNITS).min(ceiling);

    let mut burst = Vec::with_capacity(unit_len * max_units);
    for _ in 0..max_units {
        burst.extend_from_slice(&unit);
    }

    let step_io = steps.iter().map(|s| (s.press(), s.release())).collect();
    let step_msgs = steps.iter().map(|s| s.messages()).collect();
    let unit_msgs = sequence::unit_messages(&steps);
    let scripted = sequence::has_waits(&steps);
    let emissions = sequence::emissions(&steps);

    let (dangling_inputs, dangling_msgs) = sequence::dangling(&steps);

    Plan {
        unit_len,
        burst,
        max_units,
        step_io,
        unit_msgs,
        step_msgs,
        emissions,
        scripted,
        empty,
        dangling_inputs,
        dangling_msgs,
    }
}

fn worker(inner: Arc<Inner>) {
    win32::begin_timer_period();
    let mut rng = Rng::new();

    'session: loop {
        if inner.shutdown.load(Ordering::Relaxed) {
            break;
        }

        if !inner.active.load(Ordering::Acquire) {
            let guard = inner.gate.lock().unwrap();
            let _ = inner
                .wake
                .wait_timeout(guard, Duration::from_millis(150))
                .unwrap();
            continue;
        }

        let settings = inner.settings.lock().unwrap().clone();
        let generation = inner.generation.load(Ordering::Acquire);
        let plan = build_plan(&settings);

        if plan.empty {
            inner.measured.store(0, Ordering::Relaxed);
            while inner.active.load(Ordering::Relaxed)
                && !inner.shutdown.load(Ordering::Relaxed)
                && inner.generation.load(Ordering::Relaxed) == generation
            {
                thread::sleep(Duration::from_millis(50));
            }
            continue 'session;
        }

        let rate_lo = settings.cps_min.max(0.1);
        let rate_hi = settings.cps_max.max(rate_lo);
        let randomize = settings.randomize && (rate_hi - rate_lo) > 1e-9;
        let jitter = (settings.jitter / 100.0).clamp(0.0, 0.95);
        let duty = if settings.duty_enabled {
            (settings.duty_cycle / 100.0).clamp(0.0, 0.95)
        } else {
            0.0
        };

        let scripted = plan.scripted;
        let instant = duty <= 1e-6 && !scripted;
        let spin = settings.precision == "max";
        let min_slice = if spin { MIN_SLICE_SPIN } else { MIN_SLICE_BALANCED };

        let direct = settings.delivery == "window";
        let follows_focus = direct && settings.target_mode == "focused";

        let mut target = if direct {
            match *inner.target.lock().unwrap() {

                Some(t) if win32::window_alive(t.hwnd) && !win32::ignores_posted_input(t.hwnd) => {
                    Some(t)
                }
                Some(t) if win32::window_alive(t.hwnd) => None,
                _ => {
                    inner.active.store(false, Ordering::Release);
                    inner.measured.store(0, Ordering::Relaxed);
                    continue 'session;
                }
            }
        } else {
            None
        };
        let mut last_refocus = Instant::now();

        let needle = settings.filter_title.trim().to_lowercase();
        let use_filter = settings.filter_enabled && !needle.is_empty();
        let limited = settings.limit_enabled && settings.limit_count > 0;
        let time_limit = if settings.time_limit_enabled {
            Some(Duration::from_secs_f64(settings.time_limit_secs.max(0.1)))
        } else {
            None
        };

        let burst = if settings.burst_enabled {
            Some((settings.burst_count.max(1), settings.burst_pause_ms.max(0.0)))
        } else {
            None
        };
        let mut since_burst = 0u64;

        let extra_points: Vec<(i32, i32)> = settings
            .points
            .iter()
            .map(|p| (p.x as i32, p.y as i32))
            .collect();
        let mut point_index = 0usize;

        let watch_pixel = settings.pixel_enabled;
        let mut pixel_checked = Instant::now();

        let mut filter_checked = Instant::now();
        let mut filter_ok = if use_filter {
            win32::foreground_title().to_lowercase().contains(&needle)
        } else {
            true
        };

        if settings.start_delay_enabled && settings.start_delay_ms > 0.0 {
            let until = Instant::now() + Duration::from_secs_f64(settings.start_delay_ms / 1000.0);
            while Instant::now() < until {
                if !inner.active.load(Ordering::Relaxed)
                    || inner.shutdown.load(Ordering::Relaxed)
                    || inner.generation.load(Ordering::Relaxed) != generation
                {
                    continue 'session;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }

        let run_began = Instant::now();

        let mut credit = 0.0f64;
        let mut last_tick = Instant::now();

        let mut due = Instant::now();
        let mut rate_start = Instant::now();
        let mut rate_units = 0u64;

        while inner.active.load(Ordering::Relaxed)
            && !inner.shutdown.load(Ordering::Relaxed)
            && inner.generation.load(Ordering::Relaxed) == generation
        {
            let hit_count = limited && inner.clicks.load(Ordering::Relaxed) >= settings.limit_count;

            let hit_time = time_limit.is_some_and(|max| run_began.elapsed() >= max);

            let hit_pixel = if watch_pixel && pixel_checked.elapsed() >= Duration::from_millis(120)
            {
                pixel_checked = Instant::now();
                pixel_triggered(&settings)
            } else {
                false
            };

            if hit_count || hit_time || hit_pixel {
                release_dangling(&plan, &target);
                inner.limit_latched.store(true, Ordering::Release);
                inner.active.store(false, Ordering::Release);
                inner.measured.store(0, Ordering::Relaxed);
                continue 'session;
            }

            if inner.guard_blocked.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(20));

                credit = 0.0;
                last_tick = Instant::now();
                due = Instant::now();
                continue;
            }

            if follows_focus && last_refocus.elapsed() >= Duration::from_millis(250) {
                last_refocus = Instant::now();
                if let Some(found) = win32::foreground_target() {
                    if !win32::ignores_posted_input(found.hwnd) {
                        target = Some(found);
                        *inner.target.lock().unwrap() = Some(found);
                        *inner.target_title.lock().unwrap() = win32::window_title(found.hwnd);
                    }
                }
            }

            if use_filter {
                if filter_checked.elapsed() >= Duration::from_millis(150) {
                    filter_checked = Instant::now();
                    filter_ok = win32::foreground_title().to_lowercase().contains(&needle);
                }
                if !filter_ok {
                    thread::sleep(Duration::from_millis(20));

                    credit = 0.0;
                    last_tick = Instant::now();
                    due = Instant::now();
                    continue;
                }
            }

            let rate = if randomize {
                rng.range(rate_lo, rate_hi)
            } else {
                rate_hi
            }
            .max(0.1);

            let dispatched: u64;

            if instant {

                let now = Instant::now();
                let elapsed = (now - last_tick).as_secs_f64();
                last_tick = now;

                credit = (credit + elapsed * rate).min(MAX_CREDIT);

                let mut units = credit.floor() as usize;
                if units > 0 {
                    if units > plan.max_units {
                        units = plan.max_units;
                    }
                    if limited {
                        let done = inner.clicks.load(Ordering::Relaxed);
                        let remaining = settings.limit_count.saturating_sub(done) as usize;
                        if remaining < units {
                            units = remaining.max(1);
                        }
                    }

                    if let Some((count, _)) = burst {
                        let left = count.saturating_sub(since_burst).max(1) as usize;
                        if left < units {
                            units = left;
                        }
                    }
                    match &target {
                        Some(t) => {
                            for _ in 0..units {
                                let (x, y) = next_point(t, &extra_points, &mut point_index);
                                post_messages_at(t.hwnd, x, y, &plan.unit_msgs);
                            }
                        }
                        None => {
                            win32::send_inputs(&plan.burst[..units * plan.unit_len]);
                        }
                    }
                    credit -= units as f64;
                    dispatched = units as u64;
                } else {
                    dispatched = 0;
                }

                let mut wait = ((1.0 - credit).max(0.0) / rate).max(min_slice);
                if jitter > 0.0 {
                    let wobbled = wait * (1.0 + rng.range(-jitter, jitter));
                    if wobbled > 0.0 {
                        wait = wobbled;
                    }
                }
                wait_coarse(Duration::from_secs_f64(wait.min(0.25)), spin);
            } else {

                let mut period = 1.0 / rate;
                if jitter > 0.0 {
                    let wobbled = period * (1.0 + rng.range(-jitter, jitter));
                    if wobbled > 0.0 {
                        period = wobbled;
                    }
                }
                if scripted {

                    let mut cursor = due;

                    for emission in &plan.emissions {
                        let stopping = !inner.active.load(Ordering::Relaxed)
                            || inner.shutdown.load(Ordering::Relaxed)
                            || inner.generation.load(Ordering::Relaxed) != generation;

                        match emission {
                            sequence::Emission::Fire { inputs, msgs } => match &target {
                                Some(t) => {
                                    let (x, y) = next_point(t, &extra_points, &mut point_index);
                                    post_messages_at(t.hwnd, x, y, msgs);
                                }
                                None => {
                                    win32::send_inputs(inputs);
                                }
                            },
                            sequence::Emission::Wait(ms) => {
                                if stopping {
                                    continue;
                                }
                                cursor += Duration::from_secs_f64(ms / 1000.0);
                                settle(&mut cursor, spin);
                            }
                        }
                    }

                    inner.clicks.fetch_add(1, Ordering::Relaxed);
                    inner.total_clicks.fetch_add(1, Ordering::Relaxed);
                    rate_units += 1;

                    let window = rate_start.elapsed();
                    if window >= Duration::from_millis(250) {
                        let measured = rate_units as f64 / window.as_secs_f64();
                        inner
                            .measured
                            .store((measured * 1000.0) as u64, Ordering::Relaxed);
                        rate_start = Instant::now();
                        rate_units = 0;
                    }

                    due = cursor + Duration::from_secs_f64(period);
                    settle(&mut due, spin);
                    continue;
                }

                let slice = period / plan.step_io.len().max(1) as f64;
                let hold = slice * duty;
                let started = due;

                for index in 0..plan.step_io.len() {
                    if !inner.active.load(Ordering::Relaxed) {
                        break;
                    }

                    let slot = started + Duration::from_secs_f64(slice * index as f64);

                    let spot = target
                        .as_ref()
                        .map(|t| next_point(t, &extra_points, &mut point_index));

                    match (&target, spot) {
                        (Some(t), Some((px, py))) => {
                            post_messages_at(t.hwnd, px, py, &plan.step_msgs[index].0)
                        }
                        _ => {
                            win32::send_inputs(&plan.step_io[index].0);
                        }
                    }

                    if hold > 1e-6 {
                        let mut release_at = slot + Duration::from_secs_f64(hold);
                        settle(&mut release_at, spin);
                    }

                    match (&target, spot) {
                        (Some(t), Some((px, py))) => {
                            post_messages_at(t.hwnd, px, py, &plan.step_msgs[index].1)
                        }
                        _ => {
                            win32::send_inputs(&plan.step_io[index].1);
                        }
                    }

                    let mut next_slot = slot + Duration::from_secs_f64(slice);
                    settle(&mut next_slot, spin);
                }

                dispatched = 1;
                due += Duration::from_secs_f64(period);
                settle(&mut due, spin);
            }

            if dispatched > 0 {
                inner.clicks.fetch_add(dispatched, Ordering::Relaxed);
                inner.total_clicks.fetch_add(dispatched, Ordering::Relaxed);
                rate_units += dispatched;
                since_burst += dispatched;
            }

            if let Some((count, pause_ms)) = burst {
                if since_burst >= count {
                    since_burst = 0;

                    let until = Instant::now() + Duration::from_secs_f64(pause_ms / 1000.0);
                    while Instant::now() < until {
                        if !inner.active.load(Ordering::Relaxed)
                            || inner.shutdown.load(Ordering::Relaxed)
                            || inner.generation.load(Ordering::Relaxed) != generation
                        {
                            break;
                        }
                        thread::sleep(Duration::from_millis(5));
                    }

                    credit = 0.0;
                    last_tick = Instant::now();
                    due = Instant::now();

                    rate_start = Instant::now();
                    rate_units = 0;
                }
            }

            let window = rate_start.elapsed();
            if window >= Duration::from_millis(250) {
                let measured = rate_units as f64 / window.as_secs_f64();
                inner
                    .measured
                    .store((measured * 1000.0) as u64, Ordering::Relaxed);
                rate_start = Instant::now();
                rate_units = 0;
            }
        }

        release_dangling(&plan, &target);
    }

    win32::end_timer_period();
}

fn release_dangling(plan: &Plan, target: &Option<win32::Target>) {
    if plan.dangling_inputs.is_empty() {
        return;
    }
    match target {
        Some(t) => post_messages_at(t.hwnd, t.x, t.y, &plan.dangling_msgs),
        None => {
            win32::send_inputs(&plan.dangling_inputs);
        }
    }
}
