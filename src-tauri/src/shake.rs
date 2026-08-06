//! Shakes the view while a game has hold of the pointer.
//!
//! Three things make this work, and all of them matter.
//!
//! **The movement is relative, not a jump to a coordinate.** A game in first
//! person reads the mouse through raw input, which reports how far the device
//! moved. `SetCursorPos` moves the pointer without any device having moved, so
//! it produces no raw input at all and the camera never budges. A relative
//! move through `SendInput` does, so the game sees it as you moving the mouse.
//!
//! **Every swing is undone by its exact opposite.** The camera goes out and
//! comes straight back, which is what reads as a shake rather than as a slow
//! drift off target. It also means the pointer ends each cycle exactly where
//! it began, so nothing accumulates over a long hold.
//!
//! **The pause between the two halves** is what the game needs to draw a frame
//! with the camera displaced. Without it the pair cancels before anything is
//! rendered and you see nothing.

use std::time::{Duration, Instant};

use crate::settings::Profile;
use crate::win32;

/// How near the centre the pointer has to land, right after we moved it away,
/// to count as having been put back. Tight on purpose: we know we displaced
/// it, so anything near the middle now got there because something put it
/// there. Not zero, since a client area an odd number of pixels wide leaves
/// our idea of the middle a pixel off the game's.
const CONFIRM_SLACK: i32 = 2;

/// How far the first swing goes while we still have no idea whether a game
/// holds the pointer.
///
/// The only honest test is to move the pointer and see whether it gets put
/// back, which costs a movement even when the answer is no. So the question is
/// asked as quietly as possible: far enough to clear `CONFIRM_SLACK` and to
/// survive the way Windows scales down small movements, and no further. If the
/// answer comes back no, that is all anyone saw.
const PROBE_PX: i32 = 10;

/// How near the exact centre the pointer must be before a probe is worth
/// spending, when nothing has been confirmed yet.
///
/// A game pins the pointer to the middle to the pixel. Anything else lands
/// there only by accident, and the tighter this is, the rarer that accident.
const RESTING_SLACK: i32 = 6;

/// How long to leave it after a probe came back no.
const RETRY_MS: u64 = 1200;

/// Long enough for a game to have drawn something, on any machine worth
/// playing on.
const FRAME_MS: f64 = 20.0;

/// How often to look while nothing appears to be holding the pointer. Costs
/// nothing and moves nothing, so it can be brisk.
const IDLE_MS: u64 = 50;

enum Phase {
    /// Next move swings out.
    Out,
    /// Next move undoes this one.
    Back(i32, i32),
}

pub struct Shake {
    phase: Phase,
    due: Option<Instant>,
    seed: u64,
    /// Whether the last swing came back proving a game holds the pointer.
    /// Until it does, swings stay small and rare.
    confirmed: bool,
}

impl Shake {
    pub fn new() -> Self {
        Shake {
            phase: Phase::Out,
            due: None,
            seed: 0x2545_F491_4F6C_DD1D,
            confirmed: false,
        }
    }

    /// One of `+n` or `-n`, never something small enough to be lost in the
    /// rounding.
    fn signed(&mut self, size: i32) -> i32 {
        if self.next() < 0.5 {
            -size
        } else {
            size
        }
    }

    fn next(&mut self) -> f64 {
        let mut x = self.seed | 1;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed = x;
        (x >> 11) as f64 / ((1u64 << 53) as f64)
    }

    /// A gap near the one asked for, wobbled a little so the shake does not
    /// beat against the game's frame rate and settle into a pattern.
    fn gap(&mut self, ms: f64) -> Duration {
        let spread = 0.8 + self.next() * 0.4;
        Duration::from_secs_f64((ms.max(1.0) * spread) / 1000.0)
    }

    /// Call this often. It only does anything when it is due.
    pub fn tick(&mut self, running: bool, profile: &Profile) {
        if !running || !profile.shake_enabled {
            self.rest();
            return;
        }

        let now = Instant::now();
        let due = *self.due.get_or_insert(now);
        if now < due {
            return;
        }

        match self.phase {
            // A swing already sent has to be undone whatever else is true, so
            // the camera is never left sitting off to one side.
            Phase::Back(dx, dy) => {
                // We moved the pointer clear of the centre a moment ago. If it
                // is sitting back on the centre now, something put it there,
                // and only a game holding the mouse does that. This is real
                // evidence, unlike the guess made before moving -- that one
                // only knew the pointer happened to be near the middle, which
                // it can be by chance.
                self.confirmed = win32::cursor_locked(CONFIRM_SLACK);

                win32::move_relative(-dx, -dy);
                self.phase = Phase::Out;

                self.due = Some(if self.confirmed {
                    now + self.gap(profile.shake_ms)
                } else {
                    now + Duration::from_millis(RETRY_MS)
                });
                return;
            }
            Phase::Out => {
                let reach = profile.shake_px.max(1.0).round() as i32;

                // Nothing moves on this path, so a wrong answer here is free.
                //
                // Once a game is known to hold the pointer, allow for a swing
                // still in flight, so a frame arriving late does not break the
                // rhythm. Before that, insist on the pointer resting all but
                // exactly on centre, because every yes costs a movement and
                // most near-centre pointers are just a pointer near a centre.
                let slack = if self.confirmed {
                    reach + 8
                } else {
                    RESTING_SLACK
                };

                if !win32::cursor_locked(slack) {
                    self.due = Some(now + Duration::from_millis(IDLE_MS));
                    return;
                }

                let (dx, dy) = if self.confirmed {
                    (
                        ((self.next() * 2.0 - 1.0) * reach as f64).round() as i32,
                        ((self.next() * 2.0 - 1.0) * reach as f64).round() as i32,
                    )
                } else {
                    // An unconfirmed swing is only ever a question, so it goes
                    // the smallest distance that can still be answered.
                    (self.signed(PROBE_PX), self.signed(PROBE_PX))
                };

                win32::move_relative(dx, dy);
                self.phase = Phase::Back(dx, dy);
            }
        }

        // A probe is only worth sending if the game gets a frame to react in.
        // Your own hold setting can be shorter than that, which is fine for
        // shaking but would have every probe come back no, so the question is
        // always given a frame's grace.
        let wait = if self.confirmed {
            profile.shake_ms
        } else {
            profile.shake_ms.max(FRAME_MS)
        };

        let gap = self.gap(wait);
        self.due = Some(now + gap);
    }

    /// Stop, putting the camera back if a swing was still owed its opposite.
    pub fn rest(&mut self) {
        if let Phase::Back(dx, dy) = self.phase {
            win32::move_relative(-dx, -dy);
            self.phase = Phase::Out;
        }
        self.due = None;
        // Whatever was true of the pointer last time says nothing about the
        // next run, so the next one earns its answer again.
        self.confirmed = false;
    }
}
