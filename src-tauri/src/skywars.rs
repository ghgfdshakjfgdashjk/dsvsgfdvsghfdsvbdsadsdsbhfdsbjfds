use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

/// The chest slot colour, and the lighter-to-darker shade Roblox paints a
/// slot while the mouse is over it. Both are flat, exact colours.
const SLOT_COLOURS: [(i32, i32, i32); 2] = [(110, 69, 28), (77, 48, 19)];

/// Slots are drawn as one exact colour, so this stays tight on purpose.
/// It is what keeps dirt, wood and the rest of the map out.
const SLOT_TOLERANCE: i32 = 8;

/// Smallest thing we will treat as a slot, in pixels.
const MIN_SLOT: i32 = 10;

/// Fewest slots that can count as a chest.
const MIN_SLOTS: usize = 4;

/// An empty slot is flat colour. Any of these three means something is in it.
const OCCUPIED_PERCENT: i32 = 3;
const EDGE_JUMP: i32 = 20;
const EDGE_PERCENT: i32 = 2;
const SPREAD_MIN: i32 = 40;

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
pub struct SkywarsSettings {
    pub bind_enabled: bool,
    pub bind_vk: u32,
    pub click_hold_ms: f64,
    pub settle_ms: f64,
    pub between_ms: f64,
    pub clicks_per_item: u32,
    pub retry_gap_ms: f64,
    /// Where the speed control was left, so it reads right next time.
    ///
    /// Nothing here uses it. Moving it rewrites the delays above directly, so
    /// by the time they reach this struct they are already the real numbers.
    /// It is remembered only so the control has somewhere to start from.
    pub speed: f64,
    pub restore_cursor: bool,
}

impl Default for SkywarsSettings {
    fn default() -> Self {
        SkywarsSettings {
            bind_enabled: false,
            bind_vk: 0,
            // Tuned against a real chest rather than guessed at.
            click_hold_ms: 1.0,
            settle_ms: 7.0,
            between_ms: 7.0,
            clicks_per_item: 2,
            retry_gap_ms: 1.0,
            speed: 1.0,
            restore_cursor: true,
        }
    }
}

impl SkywarsSettings {
    pub fn sanitised(mut self) -> Self {
        self.click_hold_ms = clamp_ms(self.click_hold_ms, 2000.0);
        self.settle_ms = clamp_ms(self.settle_ms, 2000.0);
        self.between_ms = clamp_ms(self.between_ms, 5000.0);
        self.clicks_per_item = self.clicks_per_item.clamp(1, 5);
        self.retry_gap_ms = clamp_ms(self.retry_gap_ms, 2000.0);
        if !self.speed.is_finite() || self.speed <= 0.0 {
            self.speed = 1.0;
        }
        self.speed = self.speed.clamp(0.1, 10.0);
        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkywarsStatus {
    pub busy: bool,
    pub runs: u32,
    pub taken: u32,
    pub note: String,
}

struct Inner {
    settings: Mutex<SkywarsSettings>,
    asked: AtomicBool,
    busy: AtomicBool,
    runs: AtomicU32,
    taken: AtomicU32,
    note: Mutex<String>,
}

pub struct Skywars {
    inner: Arc<Inner>,
}

impl Skywars {
    pub fn new(settings: SkywarsSettings) -> Arc<Skywars> {
        let inner = Arc::new(Inner {
            settings: Mutex::new(settings.sanitised()),
            asked: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            runs: AtomicU32::new(0),
            taken: AtomicU32::new(0),
            note: Mutex::new("Not run yet".into()),
        });

        let skywars = Arc::new(Skywars {
            inner: Arc::clone(&inner),
        });

        thread::Builder::new()
            .name("skywars".into())
            .spawn(move || worker(inner))
            .expect("failed to spawn skywars thread");

        skywars
    }

    pub fn settings(&self) -> SkywarsSettings {
        self.inner.settings.lock().unwrap().clone()
    }

    pub fn apply(&self, settings: SkywarsSettings) {
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

    pub fn status(&self) -> SkywarsStatus {
        SkywarsStatus {
            busy: self.inner.busy.load(Ordering::Acquire),
            runs: self.inner.runs.load(Ordering::Relaxed),
            taken: self.inner.taken.load(Ordering::Relaxed),
            note: self.inner.note.lock().unwrap().clone(),
        }
    }
}

fn channels(colour: u32) -> (i32, i32, i32) {
    (
        ((colour >> 16) & 0xFF) as i32,
        ((colour >> 8) & 0xFF) as i32,
        (colour & 0xFF) as i32,
    )
}

fn is_slot(colour: u32) -> bool {
    let (r, g, b) = channels(colour & 0x00FF_FFFF);
    SLOT_COLOURS.iter().any(|slot| {
        (r - slot.0).abs() <= SLOT_TOLERANCE
            && (g - slot.1).abs() <= SLOT_TOLERANCE
            && (b - slot.2).abs() <= SLOT_TOLERANCE
    })
}

fn luma(colour: u32) -> i32 {
    let (r, g, b) = channels(colour & 0x00FF_FFFF);
    (r * 299 + g * 587 + b * 114) / 1000
}

fn median(values: &mut [i32]) -> i32 {
    values.sort_unstable();
    values[values.len() / 2]
}

#[derive(Clone, Copy)]
pub struct Slot {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    count: i32,
}

impl Slot {
    fn width(&self) -> i32 {
        self.x1 - self.x0 + 1
    }
    fn height(&self) -> i32 {
        self.y1 - self.y0 + 1
    }
    fn centre(&self) -> (i32, i32) {
        ((self.x0 + self.x1) / 2, (self.y0 + self.y1) / 2)
    }
}

fn visit(state: &mut [u8], stack: &mut Vec<i32>, at: i32) {
    let index = at as usize;
    if state[index] == 1 {
        state[index] = 2;
        stack.push(at);
    }
}

/// Every connected patch of slot colour on the screen.
fn patches(frame: &[u32], width: i32, height: i32) -> Vec<Slot> {
    // 0 = not the slot colour, 1 = slot colour, 2 = already counted.
    // One pass, one buffer: half the memory traffic of a mask plus a
    // separate seen-map, which is the bulk of the scan.
    let total = (width * height) as usize;
    let mut state = vec![0u8; total];
    for (index, colour) in frame.iter().enumerate().take(total) {
        if is_slot(*colour) {
            state[index] = 1;
        }
    }

    let mut found = Vec::new();
    let mut stack: Vec<i32> = Vec::with_capacity(1024);

    for start in 0..total as i32 {
        if state[start as usize] != 1 {
            continue;
        }

        state[start as usize] = 2;
        stack.clear();
        stack.push(start);

        let mut x0 = start % width;
        let mut x1 = x0;
        let mut y0 = start / width;
        let mut y1 = y0;
        let mut count = 0;

        while let Some(at) = stack.pop() {
            count += 1;
            let x = at % width;
            let y = at / width;

            if x < x0 { x0 = x; }
            if x > x1 { x1 = x; }
            if y < y0 { y0 = y; }
            if y > y1 { y1 = y; }

            if x > 0 { visit(&mut state, &mut stack, at - 1); }
            if x < width - 1 { visit(&mut state, &mut stack, at + 1); }
            if y > 0 { visit(&mut state, &mut stack, at - width); }
            if y < height - 1 { visit(&mut state, &mut stack, at + width); }
        }

        found.push(Slot { x0, y0, x1, y1, count });
    }

    found
}

/// The slots of the chest, and nothing else.
///
/// Every rectangle this returns was seen on screen as a solid square of the
/// slot colour, so those rectangles are the only places we ever click. There
/// is no guessing at slots we could not see, which is what used to send the
/// mouse to the wrong part of the screen.
fn find_slots(frame: &[u32], width: i32, height: i32) -> Vec<Slot> {
    let mut slots: Vec<Slot> = patches(frame, width, height)
        .into_iter()
        .filter(|slot| {
            let w = slot.width();
            let h = slot.height();

            // big enough to be a slot, small enough not to be scenery
            if w < MIN_SLOT || h < MIN_SLOT || w > width / 2 || h > height / 2 {
                return false;
            }
            // square
            if w * 100 < h * 70 || w * 100 > h * 145 {
                return false;
            }
            // filled in, not a thin outline or a ragged shape
            slot.count * 100 >= w * h * 45
        })
        .collect();

    if slots.len() < MIN_SLOTS {
        return Vec::new();
    }

    // Slots in a chest are all one size. Anything else was never a slot.
    let mut widths: Vec<i32> = slots.iter().map(|s| s.width()).collect();
    let mut heights: Vec<i32> = slots.iter().map(|s| s.height()).collect();
    let slot_w = median(&mut widths);
    let slot_h = median(&mut heights);

    slots.retain(|slot| {
        (slot.width() - slot_w).abs() * 100 <= slot_w * 25
            && (slot.height() - slot_h).abs() * 100 <= slot_h * 25
    });

    if slots.len() < MIN_SLOTS {
        return Vec::new();
    }

    slots
}

/// How many rows or columns a set of centres falls into.
fn count_lines(mut values: Vec<i32>, pitch: i32) -> usize {
    if values.is_empty() {
        return 0;
    }

    values.sort_unstable();
    let limit = (pitch * 45 / 100).max(1);

    let mut lines = 1;
    for i in 1..values.len() {
        if values[i] - values[i - 1] > limit {
            lines += 1;
        }
    }

    lines
}

/// An empty slot is flat colour. Anything sitting in it shows up as a colour
/// that is not the slot, as hard sprite edges, or as a spread of brightness --
/// any one of the three is enough, so pale, dark and brown items all register.
fn occupied(frame: &[u32], width: i32, slot: &Slot) -> bool {
    let span = slot.width().min(slot.height());
    let inset = (span / 6).max(2);

    let left = slot.x0 + inset;
    let right = slot.x1 - inset;
    let top = slot.y0 + inset;
    let bottom = slot.y1 - inset;

    if right - left < 4 || bottom - top < 4 {
        return false;
    }

    let mut total = 0;
    let mut different = 0;
    let mut edges = 0;
    let mut dark = i32::MAX;
    let mut light = i32::MIN;

    for y in top..=bottom {
        let offset = (y * width) as usize;
        let mut previous: Option<i32> = None;

        for x in left..=right {
            let colour = frame[offset + x as usize];
            let level = luma(colour);

            total += 1;
            if !is_slot(colour) {
                different += 1;
            }
            if level < dark { dark = level; }
            if level > light { light = level; }

            if let Some(before) = previous {
                if (level - before).abs() >= EDGE_JUMP {
                    edges += 1;
                }
            }
            previous = Some(level);
        }
    }

    if total == 0 {
        return false;
    }

    different * 100 / total >= OCCUPIED_PERCENT
        || edges * 100 / total >= EDGE_PERCENT
        || light - dark >= SPREAD_MIN
}

/// Windows will happily overshoot a short thread::sleep by a whole timer
/// tick, which at these lengths is most of the delay. Spin for the short
/// ones and sleep only when it is long enough to be worth it.
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

fn click_at(x: i32, y: i32, settings: &SkywarsSettings) {
    let spec = win32::button_spec("left");
    let travel = win32::move_event(x, y);

    // move the real cursor first so the game sees the pointer arrive
    win32::move_cursor(x, y);
    win32::send_inputs(&[travel]);
    wait(settings.settle_ms);

    // click more than once in case the game misses the first press
    let times = settings.clicks_per_item.clamp(1, 5);
    for round in 0..times {
        if round > 0 {
            wait(settings.retry_gap_ms);
        }

        // restate the position with the press so the click cannot land stale
        win32::send_inputs(&[travel, win32::mouse_event(&spec, spec.down)]);
        wait(settings.click_hold_ms);
        win32::send_inputs(&[win32::mouse_event(&spec, spec.up)]);
    }
}

fn run_once(inner: &Inner, settings: &SkywarsSettings) {
    let (screen_w, screen_h) = win32::screen_size();
    if screen_w <= 0 || screen_h <= 0 {
        *inner.note.lock().unwrap() = "Could not read the screen".into();
        return;
    }

    let taken = win32::Grabber::new(screen_w, screen_h)
        .and_then(|mut grab| grab.grab(0, 0).map(|pixels| pixels.to_vec()));

    let Some(frame) = taken else {
        *inner.note.lock().unwrap() = "Could not capture the screen".into();
        return;
    };

    let slots = find_slots(&frame, screen_w, screen_h);
    if slots.len() < MIN_SLOTS {
        *inner.note.lock().unwrap() = "No chest on screen".into();
        return;
    }

    // The chest is exactly the area its slots cover. Nothing outside this
    // box is ever touched, so the inventory beside it is left alone.
    let left = slots.iter().map(|s| s.x0).min().unwrap_or(0);
    let right = slots.iter().map(|s| s.x1).max().unwrap_or(0);
    let top = slots.iter().map(|s| s.y0).min().unwrap_or(0);
    let bottom = slots.iter().map(|s| s.y1).max().unwrap_or(0);

    let mut widths: Vec<i32> = slots.iter().map(|s| s.width()).collect();
    let mut heights: Vec<i32> = slots.iter().map(|s| s.height()).collect();
    let slot_w = median(&mut widths);
    let slot_h = median(&mut heights);

    let across = count_lines(slots.iter().map(|s| s.centre().0).collect(), slot_w);
    let down = count_lines(slots.iter().map(|s| s.centre().1).collect(), slot_h);
    let grid = format!("{across}x{down}");

    let mut spots: Vec<(i32, i32)> = Vec::new();
    for slot in &slots {
        if !occupied(&frame, screen_w, slot) {
            continue;
        }

        let (x, y) = slot.centre();
        if x < left || x > right || y < top || y > bottom {
            continue;
        }

        spots.push((x, y));
    }

    // work through the chest the way you would read it
    spots.sort_by_key(|(x, y)| ((y - top) / slot_h.max(1), *x));

    if spots.is_empty() {
        *inner.note.lock().unwrap() = format!("{grid} chest, nothing in it");
        return;
    }

    let was = win32::cursor_position();

    for (x, y) in &spots {
        click_at(*x, *y, settings);
        wait(settings.between_ms);
    }

    if settings.restore_cursor {
        win32::move_cursor(was.0, was.1);
    }

    inner
        .taken
        .fetch_add(spots.len() as u32, Ordering::Relaxed);
    *inner.note.lock().unwrap() = format!("{grid} chest, took {} items", spots.len());
}

fn worker(inner: Arc<Inner>) {
    loop {
        if !inner.asked.swap(false, Ordering::AcqRel) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        inner.busy.store(true, Ordering::Release);
        let settings = inner.settings.lock().unwrap().clone();
        run_once(&inner, &settings);
        inner.runs.fetch_add(1, Ordering::Relaxed);
        inner.busy.store(false, Ordering::Release);
    }
}
