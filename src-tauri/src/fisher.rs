use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

pub const KINDS: usize = 5;

pub const KIND_NAMES: [&str; KINDS] = ["iron", "special", "emerald", "diamond", "gold"];

const DEFAULT_COLORS: [u32; KINDS] = [0x918C92, 0xDC6F7F, 0x00FF38, 0x02F7F7, 0xF8B52B];

const LEAD_MS: f64 = 16.0;

const SMOOTHING: f64 = 0.6;

const MARKER_UNIFORM: i32 = 18;

const MARKER_COVERAGE: i32 = 85;

const MARKER_ACCEPT: i32 = 95;

const MARKER_CONTINUITY: i32 = 25;

const MARKER_NARROWEST: usize = 4;

const MARKER_WIDEST: usize = 26;

const REPORT_EVERY_MS: u128 = 120;

const BAR_GONE: Duration = Duration::from_millis(400);

const MIN_HOLD: Duration = Duration::from_millis(30);

const DEADBAND: f64 = 4.0;

const KEY_HOLD: Duration = Duration::from_millis(60);

const CANCEL_TRIES: u32 = 4;

const CANCEL_WAIT: Duration = Duration::from_millis(220);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FisherSettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,
    pub types: Vec<bool>,
    pub colors: Vec<u32>,
    pub tolerance: u32,
    pub cast_button: String,
    pub cast_delay_ms: u64,
    pub recast_delay_ms: u64,
    pub reject_vk: u32,
    pub reject_delay_ms: u64,
    pub bite_timeout_secs: u64,
    pub fight_timeout_secs: u64,
    pub deadzone: i32,
    pub search_top: f64,
    pub search_bottom: f64,
    pub search_left: f64,
    pub search_right: f64,
}

impl Default for FisherSettings {
    fn default() -> Self {
        FisherSettings {
            bind_enabled: false,
            bind_vk: 0,
            types: vec![true; KINDS],
            colors: DEFAULT_COLORS.to_vec(),
            tolerance: 45,
            cast_button: "left".into(),
            cast_delay_ms: 600,
            recast_delay_ms: 900,
            reject_vk: 0x20,
            reject_delay_ms: 1000,
            bite_timeout_secs: 45,
            fight_timeout_secs: 30,
            deadzone: 6,
            search_top: 0.60,
            search_bottom: 0.80,
            search_left: 0.22,
            search_right: 0.78,
        }
    }
}

impl FisherSettings {
    pub fn sanitised(mut self) -> Self {
        self.types.resize(KINDS, true);
        self.colors.resize(KINDS, 0);
        for (index, color) in self.colors.iter_mut().enumerate() {
            *color &= 0x00FF_FFFF;
            if *color == 0 {
                *color = DEFAULT_COLORS[index];
            }
        }
        self.tolerance = self.tolerance.clamp(10, 120);
        self.cast_delay_ms = self.cast_delay_ms.clamp(0, 10_000);
        self.recast_delay_ms = self.recast_delay_ms.clamp(0, 10_000);
        self.reject_delay_ms = self.reject_delay_ms.clamp(0, 10_000);
        self.bite_timeout_secs = self.bite_timeout_secs.clamp(5, 300);
        self.fight_timeout_secs = self.fight_timeout_secs.clamp(5, 300);
        self.deadzone = self.deadzone.clamp(0, 60);

        for value in [
            &mut self.search_top,
            &mut self.search_bottom,
            &mut self.search_left,
            &mut self.search_right,
        ] {
            if !value.is_finite() {
                *value = 0.5;
            }
            *value = value.clamp(0.0, 1.0);
        }
        if self.search_bottom < self.search_top + 0.05 {
            self.search_bottom = (self.search_top + 0.05).min(1.0);
        }
        if self.search_right < self.search_left + 0.10 {
            self.search_right = (self.search_left + 0.10).min(1.0);
        }

        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FisherStatus {
    pub running: bool,
    pub phase: String,
    pub detail: String,
    pub caught: Vec<u32>,
    pub rejected: u32,
    pub missed: u32,
    pub bar_found: bool,
    pub log: Vec<String>,
}

struct Tally {
    caught: [u32; KINDS],
    rejected: u32,
    missed: u32,
    phase: String,
    detail: String,
    bar_found: bool,
    log: Vec<String>,
}

impl Tally {
    fn new() -> Self {
        Tally {
            caught: [0; KINDS],
            rejected: 0,
            missed: 0,
            phase: "Idle".into(),
            detail: String::new(),
            bar_found: false,
            log: Vec::new(),
        }
    }
}

struct Inner {
    running: AtomicBool,
    generation: AtomicU32,
    settings: Mutex<FisherSettings>,
    tally: Mutex<Tally>,
}

pub struct Fisher {
    inner: Arc<Inner>,
}

impl Fisher {
    pub fn new(settings: FisherSettings) -> Arc<Fisher> {
        let inner = Arc::new(Inner {
            running: AtomicBool::new(false),
            generation: AtomicU32::new(0),
            settings: Mutex::new(settings.sanitised()),
            tally: Mutex::new(Tally::new()),
        });

        let fisher = Arc::new(Fisher {
            inner: Arc::clone(&inner),
        });

        thread::Builder::new()
            .name("fisher".into())
            .spawn(move || worker(inner))
            .expect("failed to spawn fisher thread");

        fisher
    }

    pub fn settings(&self) -> FisherSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn apply(&self, settings: FisherSettings) {
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

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    pub fn set_running(&self, on: bool) {
        if on == self.is_running() {
            return;
        }
        self.inner.generation.fetch_add(1, Ordering::Relaxed);
        self.inner.running.store(on, Ordering::Release);

        if !on {
            let mut tally = self.inner.tally.lock().unwrap();
            tally.phase = "Idle".into();
            tally.detail = String::new();
            tally.bar_found = false;
        }
    }

    pub fn toggle(&self) -> bool {
        let next = !self.is_running();
        self.set_running(next);
        next
    }

    pub fn reset_counts(&self) {
        let mut tally = self.inner.tally.lock().unwrap();
        tally.caught = [0; KINDS];
        tally.rejected = 0;
        tally.missed = 0;
        tally.log.clear();
    }

    pub fn status(&self) -> FisherStatus {
        let tally = self.inner.tally.lock().unwrap();

        FisherStatus {
            running: self.inner.running.load(Ordering::Acquire),
            phase: tally.phase.clone(),
            detail: tally.detail.clone(),
            caught: tally.caught.to_vec(),
            rejected: tally.rejected,
            missed: tally.missed,
            bar_found: tally.bar_found,
            log: tally.log.clone(),
        }
    }
}

fn channels(color: u32) -> (i32, i32, i32) {
    (
        ((color >> 16) & 0xFF) as i32,
        ((color >> 8) & 0xFF) as i32,
        (color & 0xFF) as i32,
    )
}

fn is_slider(color: u32) -> bool {
    let (r, g, b) = channels(color);
    g > 95 && g - r > 28 && g - b > 16
}

struct Bar {
    top: i32,
    bottom: i32,
    slider_end: i32,
}

fn longest_slider_run(row: &[u32], from: i32, to: i32) -> Option<(i32, i32)> {
    let last = to.min(row.len() as i32 - 1);
    if last <= from {
        return None;
    }

    let mut best: Option<(i32, i32)> = None;
    let mut start: Option<i32> = None;

    for x in from..=last {
        if is_slider(row[x as usize] & 0x00FF_FFFF) {
            if start.is_none() {
                start = Some(x);
            }
        } else if let Some(began) = start.take() {
            let length = x - began;
            if best.map(|(a, b)| b - a < length).unwrap_or(true) {
                best = Some((began, x));
            }
        }
    }

    if let Some(began) = start {
        let length = last + 1 - began;
        if best.map(|(a, b)| b - a < length).unwrap_or(true) {
            best = Some((began, last + 1));
        }
    }

    best
}

fn column_extent(frame: &[u32], width: i32, height: i32, x: i32, row: i32) -> (i32, i32) {
    let mut top = row;
    while top - 1 >= 0 && is_slider(frame[((top - 1) * width + x) as usize] & 0x00FF_FFFF) {
        top -= 1;
    }

    let mut bottom = row;
    while bottom + 1 < height && is_slider(frame[((bottom + 1) * width + x) as usize] & 0x00FF_FFFF)
    {
        bottom += 1;
    }

    (top, bottom)
}

fn find_bar(
    frame: &[u32],
    width: i32,
    height: i32,
    origin_y: i32,
    min_thickness: i32,
) -> Option<Bar> {
    let minimum = (width / 14).max(30);
    let mut best: Option<(i32, i32, i32, i32)> = None;

    let mut y = 0;
    while y < height {
        let offset = (y * width) as usize;
        let row = &frame[offset..offset + width as usize];

        if let Some((from, to)) = longest_slider_run(row, 0, width - 1) {
            let length = to - from;
            if length >= minimum && best.map(|(_, a, b, _)| b - a < length).unwrap_or(true) {
                let middle = (from + to) / 2;
                let (top, bottom) = column_extent(frame, width, height, middle, y);
                if bottom - top + 1 >= min_thickness {
                    best = Some((from, to, top, bottom));
                }
            }
        }

        y += 3;
    }

    let (_, slider_end, top, bottom) = best?;

    Some(Bar {
        top: origin_y + top,
        bottom: origin_y + bottom,
        slider_end,
    })
}

fn scan_slider(
    strip: &[u32],
    width: i32,
    row: i32,
    from: i32,
    to: i32,
) -> Option<(i32, i32)> {
    let offset = (row * width) as usize;
    longest_slider_run(&strip[offset..offset + width as usize], from, to)
}

fn apart(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    let dr = a.0 - b.0;
    let dg = a.1 - b.1;
    let db = a.2 - b.2;
    (((dr * dr + dg * dg + db * db) as f64).sqrt()) as i32
}

#[derive(Default)]
struct Probe {
    rows: i32,
    steady: i32,
    runs: i32,
    widest: usize,
    sample: u32,
    spread: i32,
}

fn scan_marker(
    strip: &[u32],
    width: i32,
    height: i32,
    colors: &[u32],
    stats: &mut [i32],
    probe: &mut Probe,
) -> Option<(i32, usize, u32, i32, i32)> {
    *probe = Probe::default();
    for slot in stats.iter_mut() {
        *slot = 0;
    }

    let mut rows = 0i32;
    let mut y = 0;
    while y < height {
        let offset = (y * width) as usize;
        for i in 0..width as usize {
            let (r, g, b) = channels(strip[offset + i] & 0x00FF_FFFF);
            let at = i * 6;
            stats[at] += r;
            stats[at + 1] += g;
            stats[at + 2] += b;
            stats[at + 3] += r * r;
            stats[at + 4] += g * g;
            stats[at + 5] += b * b;
        }
        rows += 1;
        y += 2;
    }

    if rows < 2 {
        return None;
    }

    let ceiling = MARKER_UNIFORM * MARKER_UNIFORM * MARKER_COVERAGE / 100;

    let mean_at = |i: usize| -> (i32, i32, i32) {
        let at = i * 6;
        (
            stats[at] / rows,
            stats[at + 1] / rows,
            stats[at + 2] / rows,
        )
    };

    let spread_at = |i: usize| -> i32 {
        let at = i * 6;
        let mut spread = 0;
        for k in 0..3 {
            let mean = stats[at + k] as f64 / rows as f64;
            let square = stats[at + 3 + k] as f64 / rows as f64;
            spread += (square - mean * mean).max(0.0) as i32;
        }
        spread
    };

    let steady = |i: usize| -> bool { spread_at(i) <= ceiling };

    probe.rows = rows;
    probe.steady = (0..width as usize).filter(|i| steady(*i)).count() as i32;
    probe.spread = spread_at((width / 2) as usize);

    let mut best: Option<(usize, usize, i32)> = None;
    let mut from: Option<usize> = None;
    let mut candidates = 0i32;

    for i in 0..=width as usize {
        let joins = i < width as usize
            && steady(i)
            && (i == 0 || from.is_none() || apart(mean_at(i), mean_at(i - 1)) <= MARKER_CONTINUITY);

        if joins {
            if from.is_none() {
                from = Some(i);
            }
            continue;
        }

        if let Some(start) = from {
            let span = i - start;
            probe.runs += 1;
            if span > probe.widest {
                probe.widest = span;
                probe.sample = {
                    let mean = mean_at((start + i - 1) / 2);
                    ((mean.0 as u32) << 16) | ((mean.1 as u32) << 8) | mean.2 as u32
                };
            }

            if span >= MARKER_NARROWEST && span <= MARKER_WIDEST {
                let middle = (start + i - 1) / 2;
                let mean = mean_at(middle);

                let mut gap = i32::MAX;
                for target in colors.iter() {
                    gap = gap.min(apart(mean, channels(*target)));
                }

                candidates += 1;
                if best.map(|(_, _, seen)| gap < seen).unwrap_or(true) {
                    best = Some((start, i - 1, gap));
                }
            }
        }

        from = if i < width as usize && steady(i) {
            Some(i)
        } else {
            None
        };
    }

    let (start, end, gap) = best?;
    let middle = (start + end) / 2;
    let mean = mean_at(middle);

    let mut kind = 0;
    let mut closest = i32::MAX;
    for (index, target) in colors.iter().enumerate() {
        let seen = apart(mean, channels(*target));
        if seen < closest {
            closest = seen;
            kind = index;
        }
    }

    let packed = ((mean.0 as u32) << 16) | ((mean.1 as u32) << 8) | mean.2 as u32;
    Some((middle as i32, kind, packed, gap, candidates))
}

fn press(vk: u16) {
    win32::send_inputs(&[win32::key_event(vk, false)]);
    thread::sleep(KEY_HOLD);
    win32::send_inputs(&[win32::key_event(vk, true)]);
}

fn hold(spec: &win32::ButtonSpec, down: bool) {
    let event = win32::mouse_event(spec, if down { spec.down } else { spec.up });
    win32::send_inputs(&[event]);
}

fn record(inner: &Inner, line: String) {
    let mut tally = inner.tally.lock().unwrap();
    tally.log.push(line);
    let extra = tally.log.len().saturating_sub(14);
    if extra > 0 {
        tally.log.drain(0..extra);
    }
}

fn note(inner: &Inner, phase: &str, detail: &str) {
    let mut tally = inner.tally.lock().unwrap();
    tally.phase = phase.to_string();
    tally.detail = detail.to_string();
}

fn still_running(inner: &Inner, generation: u32) -> bool {
    inner.running.load(Ordering::Acquire)
        && inner.generation.load(Ordering::Relaxed) == generation
}

fn worker(inner: Arc<Inner>) {
    let idle = Duration::from_millis(120);

    loop {
        if !inner.running.load(Ordering::Acquire) {
            thread::sleep(idle);
            continue;
        }

        let generation = inner.generation.load(Ordering::Relaxed);
        session(&inner, generation);
    }
}

fn session(inner: &Arc<Inner>, generation: u32) {
    let (screen_w, screen_h) = win32::screen_size();
    if screen_w <= 0 || screen_h <= 0 {
        note(inner, "Error", "could not read screen size");
        thread::sleep(Duration::from_millis(500));
        return;
    }

    let opening = inner.settings.lock().unwrap().clone();

    let scout_x = ((screen_w as f64) * opening.search_left) as i32;
    let scout_y = ((screen_h as f64) * opening.search_top) as i32;
    let scout_w = (((screen_w as f64) * opening.search_right) as i32 - scout_x)
        .max(60)
        .min(screen_w - scout_x);
    let scout_h = (((screen_h as f64) * opening.search_bottom) as i32 - scout_y)
        .max(24)
        .min(screen_h - scout_y);

    let mut scout = match win32::Grabber::new(scout_w, scout_h) {
        Some(grabber) => grabber,
        None => {
            note(inner, "Error", "could not open a screen capture");
            thread::sleep(Duration::from_millis(500));
            return;
        }
    };

    let min_thickness = (screen_h * 18 / 1080).max(5);

    let spec = win32::button_spec(&opening.cast_button);
    let mut holding = false;

    while still_running(inner, generation) {
        let settings = inner.settings.lock().unwrap().clone();

        note(inner, "Casting", "throwing the rod");
        hold(&spec, true);
        thread::sleep(Duration::from_millis(60));
        hold(&spec, false);

        thread::sleep(Duration::from_millis(settings.cast_delay_ms));
        if !still_running(inner, generation) {
            break;
        }

        note(inner, "Waiting", "watching for a bite");
        let waiting_since = Instant::now();
        let mut bar = None;

        while still_running(inner, generation) {
            if waiting_since.elapsed() > Duration::from_secs(settings.bite_timeout_secs) {
                note(inner, "Waiting", "timed out, recasting");
                break;
            }

            if let Some(frame) = scout.grab(scout_x, scout_y) {
                if let Some(found) = find_bar(frame, scout_w, scout_h, scout_y, min_thickness) {
                    bar = Some(found);
                    break;
                }
            }

            thread::sleep(Duration::from_millis(12));
        }

        let Some(bar) = bar else {
            continue;
        };

        {
            let mut tally = inner.tally.lock().unwrap();
            tally.bar_found = true;
        }

        let strip_x = scout_x;
        let strip_w = scout_w;
        let middle = (bar.top + bar.bottom) / 2;
        let reach = ((bar.bottom - bar.top) * 30 / 100).max(3);
        let strip_y = (middle - reach).max(0);
        let strip_h = (reach * 2 + 1).min(screen_h - strip_y).max(4);

        let mut strip = match win32::Grabber::new(strip_w, strip_h) {
            Some(grabber) => grabber,
            None => continue,
        };

        let slider_row = (middle - strip_y).clamp(0, strip_h - 1);
        let mut stats = vec![0i32; strip_w as usize * 6];
        let mut probe = Probe::default();

        note(inner, "Reeling", "on the line");
        record(
            inner,
            format!("--- bite: block y={}..{} band {} tall ---", bar.top, bar.bottom, strip_h),
        );

        let fighting_since = Instant::now();
        let mut blank: Option<Instant> = None;
        let mut last_flip = Instant::now();
        let mut agreed: Option<(usize, u32)> = None;
        let mut flips = 0u32;
        let mut landed = false;
        let mut cancelled = false;

        let mut kind: Option<usize> = None;
        let mut marker_at = bar.slider_end as f64;
        let mut slider_at = bar.slider_end as f64;
        let mut marker_speed = 0.0f64;
        let mut slider_speed = 0.0f64;
        let mut clock = Instant::now();
        let mut primed = false;

        let mut widest = 0;
        let mut sighted = false;
        let mut last_unknown: Option<Instant> = None;
        let mut last_report = Instant::now();

        while still_running(inner, generation) {
            if fighting_since.elapsed() > Duration::from_secs(settings.fight_timeout_secs) {
                break;
            }

            let Some(frame) = strip.grab(strip_x, strip_y) else {
                break;
            };

            let slider = scan_slider(frame, strip_w, slider_row, 0, strip_w - 1);

            let Some((slider_start, slider_end)) = slider else {
                if blank.map(|at: Instant| at.elapsed() >= BAR_GONE) .unwrap_or(false) {
                    landed = true;
                    break;
                }
                if blank.is_none() {
                    blank = Some(Instant::now());
                }
                thread::sleep(Duration::from_millis(4));
                continue;
            };

            blank = None;

            let now = Instant::now();
            let step = ((now - clock).as_secs_f64() * 1000.0).clamp(1.0, 80.0);
            clock = now;

            let found = scan_marker(
                frame,
                strip_w,
                strip_h,
                &settings.colors,
                &mut stats,
                &mut probe,
            );

            let due = last_unknown
                .map(|at| at.elapsed().as_millis() > 700)
                .unwrap_or(true);

            match found {
                Some((_, _, colour, gap, count)) if gap > MARKER_ACCEPT / 2 && due => {
                    let _ = count;
                    last_unknown = Some(now);
                    record(
                        inner,
                        format!("marker #{colour:06X} is {gap} from any known colour"),
                    );
                }
                None if due && !sighted => {
                    last_unknown = Some(now);
                    record(
                        inner,
                        format!(
                            "no marker: rows={} steady={} runs={} widest={} eg#{:06X}",
                            probe.rows, probe.steady, probe.runs, probe.widest, probe.sample
                        ),
                    );
                }
                _ => {}
            }

            let seen = found.filter(|(_, _, _, gap, count)| *gap <= MARKER_ACCEPT || *count == 1);
            if seen.is_some() {
                sighted = true;
            }


            let marker_now = match seen {
                Some((at, _, _, _, _)) => at as f64,
                None => marker_at,
            };

            let span = slider_end - slider_start;
            widest = widest.max(span);

            let position_now = ((slider_start + slider_end) / 2) as f64;

            if primed {
                let m = (marker_now - marker_at) / step;
                let s = (position_now - slider_at) / step;
                marker_speed = marker_speed * SMOOTHING + m * (1.0 - SMOOTHING);
                slider_speed = slider_speed * SMOOTHING + s * (1.0 - SMOOTHING);
            }
            primed = true;

            marker_at = marker_now;
            slider_at = position_now;

            let gap = marker_now - position_now;
            let closing = marker_speed - slider_speed;
            let signal = gap + LEAD_MS * closing;

            let want_hold = if !sighted || signal.abs() < DEADBAND {
                holding
            } else {
                signal > 0.0
            };

            if want_hold != holding && last_flip.elapsed() >= MIN_HOLD {
                hold(&spec, want_hold);
                holding = want_hold;
                last_flip = Instant::now();
                flips += 1;
            }

            if let (None, Some((_, found, _, _, _))) = (kind, seen) {
                agreed = match agreed {
                    Some((was, count)) if was == found => Some((was, count + 1)),
                    _ => Some((found, 1u32)),
                };

                if let Some((settled, count)) = agreed {
                    if count >= 2 {
                        kind = Some(settled);

                        if !settings.types[settled] {
                            note(
                                inner,
                                "Cancelling",
                                &format!("{} is switched off", KIND_NAMES[settled]),
                            );
                            record(
                                inner,
                                format!("{} is off, pressing cancel", KIND_NAMES[settled]),
                            );

                            if holding {
                                hold(&spec, false);
                                holding = false;
                            }
                            cancelled = true;
                            break;
                        }

                        note(
                            inner,
                            "Reeling",
                            &format!("{} on the line", KIND_NAMES[settled]),
                        );
                    }
                }
            }

            if last_report.elapsed().as_millis() > REPORT_EVERY_MS {
                last_report = now;
                let label = kind.map(|k| KIND_NAMES[k]).unwrap_or("?");
                let mark = format!("{}", marker_at as i32);
                record(
                    inner,
                    format!(
                        "{}s {label} mark={mark} bar={} [{}..{}] flips={} {}",
                        fighting_since.elapsed().as_secs(),
                        slider_at as i32,
                        slider_start,
                        slider_end,
                        flips,
                        if holding { "HOLD" } else { "free" }
                    ),
                );
            }

            thread::sleep(Duration::from_millis(2));
        }

        if holding {
            hold(&spec, false);
            holding = false;
        }

        if cancelled {
            for attempt in 1..=CANCEL_TRIES {
                press(settings.reject_vk as u16);
                thread::sleep(CANCEL_WAIT);

                let gone = match strip.grab(strip_x, strip_y) {
                    Some(frame) => {
                        scan_slider(frame, strip_w, slider_row, 0, strip_w - 1).is_none()
                    }
                    None => true,
                };

                if gone {
                    record(inner, format!("cancelled after {attempt} press(es)"));
                    break;
                }

                if attempt == CANCEL_TRIES {
                    record(inner, "cancel did not clear the bar".into());
                }
            }
        }

        {
            let mut tally = inner.tally.lock().unwrap();
            tally.bar_found = false;
            let outcome = if cancelled {
                "cancelled"
            } else if landed {
                "bar gone"
            } else {
                "timed out"
            };
            tally.log.push(format!("--- {outcome} ---"));

            match (cancelled, kind) {
                (true, _) => tally.rejected += 1,
                (false, Some(index)) => tally.caught[index] += 1,
                (false, None) => {}
            }
        }

        if !still_running(inner, generation) {
            break;
        }

        note(inner, "Waiting", "recasting");
        let pause = if cancelled {
            settings.reject_delay_ms
        } else {
            settings.recast_delay_ms
        };
        thread::sleep(Duration::from_millis(pause));
    }

    if holding {
        hold(&spec, false);
    }

    note(inner, "Idle", "");
}
