//! Last key wins, for any number of keys at once.
//!
//! Hold A, then press D without letting go of A, and the game normally has to
//! decide what two opposing directions mean. SOCD decides for it: the most
//! recent press is the one that counts, and the moment you let it go the one
//! underneath comes back — still held, so it takes over without you
//! re-pressing anything.
//!
//! **Groups, not pairs.** Keys cancel each other only within their own group,
//! which is why A/D and W/S are separate: pressing W should not cancel A, or
//! you could never move diagonally. A group can hold as many keys as you
//! like, so three or four keys fighting over one axis works the same way two
//! do.
//!
//! **This one cannot outlive the app.** Unlike rebinding, which Windows reads
//! once and applies for good, deciding what a keystroke means has to happen
//! at the moment it is pressed. That needs something running. Syntax lives in
//! the tray, so closing the window is enough -- but quitting really does stop
//! it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::win32;

/// The key an emulated repeat is currently driving, and when the next one is
/// due.
struct Repeat {
    vk: u32,
    due: Instant,
    gap: Duration,
}

static REPEAT: Mutex<Option<Repeat>> = Mutex::new(None);
static REPEAT_THREAD: AtomicBool = AtomicBool::new(false);

/// Start repeating a key that took over from another.
///
/// Windows repeats whichever key the *hardware* most recently reported. When
/// you release the newer key, the older one is still physically down but the
/// keyboard never announces it again, so Windows has nothing to repeat and it
/// simply sits there held. A Wooting gets around this by re-reporting the key
/// itself, and the repeat resumes.
///
/// Nothing here can reach the hardware, and an injected press starts no
/// repeat timer, so the repeat is produced here instead -- at the delay and
/// rate Windows itself is set to, so it is indistinguishable from the real
/// thing.
fn start_repeat(vk: u32) {
    let (first, gap) = win32::keyboard_repeat();

    let Ok(mut held) = REPEAT.lock() else {
        return;
    };

    *held = Some(Repeat {
        vk,
        due: Instant::now() + first,
        gap,
    });

    // Started while still holding the lock, and the thread only stands down
    // while holding it too. Between them there is no moment where the key is
    // set but no thread is coming, or a thread exits just as one is wanted.
    if !REPEAT_THREAD.swap(true, Ordering::AcqRel) {
        spawn_repeat_thread();
    }
}

fn stop_repeat() {
    if let Ok(mut held) = REPEAT.lock() {
        *held = None;
    }
}

/// Drives one repeating key, and exits as soon as there is none.
///
/// It exits rather than idling because a thread waking every 2ms forever is
/// real work to do nothing with, and takeovers happen at human speed -- so
/// starting one again costs nothing worth measuring.
fn spawn_repeat_thread() {
    thread::Builder::new()
        .name("socd-repeat".into())
        .spawn(|| loop {
            thread::sleep(Duration::from_millis(2));

            // Decide inside the lock, send outside it. The keyboard hook
            // wants this lock too, and holding it across a system call would
            // put every keystroke on the machine behind it.
            let send = {
                let Ok(mut held) = REPEAT.lock() else {
                    continue;
                };

                match held.as_mut() {
                    None => {
                        REPEAT_THREAD.store(false, Ordering::Release);
                        return;
                    }
                    Some(repeat) if Instant::now() >= repeat.due => {
                        repeat.due += repeat.gap;
                        Some(repeat.vk)
                    }
                    _ => None,
                }
            };

            if let Some(vk) = send {
                win32::send_inputs(&[win32::key_event(vk as u16, false)]);
            }
        })
        .ok();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SocdSettings {
    pub enabled: bool,
    /// Keys that cancel one another. Each inner list is one group.
    pub groups: Vec<Vec<u32>>,
}

impl Default for SocdSettings {
    fn default() -> Self {
        SocdSettings {
            enabled: false,
            // The two movement axes, kept apart so a diagonal still works.
            groups: vec![vec![0x41, 0x44], vec![0x57, 0x53]],
        }
    }
}

impl SocdSettings {
    pub fn sanitised(mut self) -> Self {
        for group in self.groups.iter_mut() {
            group.retain(|vk| *vk > 0 && *vk <= 0xFF);

            // Not `dedup`, which only catches neighbours -- these are in the
            // order they were clicked, and that order is what the interface
            // shows, so sorting to suit a shortcut is not on.
            let mut kept: Vec<u32> = Vec::with_capacity(group.len());
            group.retain(|vk| {
                if kept.contains(vk) {
                    false
                } else {
                    kept.push(*vk);
                    true
                }
            });

            group.truncate(12);
        }

        // A group of one is kept, even though it cancels nothing.
        //
        // Dropping them looked reasonable and was wrong: a group is built one
        // key at a time, so it passes through having exactly one key on its
        // way to having two. Throwing it away at that moment meant the group
        // vanished the instant you clicked the first key, and there was no
        // way to make one at all. Only genuinely empty groups go.
        self.groups.retain(|group| !group.is_empty());
        self.groups.truncate(8);

        // A key in two groups would be released by one and re-pressed by the
        // other, which is a fight with no winner. First group keeps it.
        let mut claimed: Vec<u32> = Vec::new();
        for group in self.groups.iter_mut() {
            group.retain(|vk| {
                if claimed.contains(vk) {
                    false
                } else {
                    claimed.push(*vk);
                    true
                }
            });
        }
        self.groups.retain(|group| !group.is_empty());

        self
    }
}

/// One group's worth of live state.
struct Live {
    keys: Vec<u32>,
    /// Physically held keys, oldest first. The last one is in charge.
    held: Vec<u32>,
}

struct State {
    on: bool,
    groups: Vec<Live>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Hand the hook a fresh configuration.
pub fn apply(settings: &SocdSettings) {
    let cleaned = settings.clone().sanitised();

    if let Ok(mut held) = STATE.lock() {
        // Anything currently suppressed is let go of first. Changing the
        // groups underneath a held key would otherwise strand it: the group
        // that owed it a release no longer exists.
        if let Some(state) = held.as_ref() {
            release_all(state);
        }

        *held = Some(State {
            on: cleaned.enabled,
            groups: cleaned
                .groups
                .iter()
                .map(|keys| Live {
                    keys: keys.clone(),
                    held: Vec::new(),
                })
                .collect(),
        });
    }

    if cleaned.enabled {
        win32::install_socd_hook();
    } else {
        win32::remove_socd_hook();
    }
}

/// Put every key we are holding down back up, so nothing is left stuck.
fn release_all(state: &State) {
    stop_repeat();

    for group in &state.groups {
        if let Some(winner) = group.held.last() {
            win32::send_inputs(&[win32::key_event(*winner as u16, true)]);
        }
    }
}

pub fn shutdown() {
    if let Ok(mut held) = STATE.lock() {
        if let Some(state) = held.as_ref() {
            release_all(state);
        }
        *held = None;
    }
    win32::remove_socd_hook();
}

/// Decide what to do with one key event.
///
/// Returns true to swallow it. Called from inside the keyboard hook, so it
/// does the least it can get away with: a lock, a short search, and at most
/// one injected event.
pub fn intercept(vk: u32, down: bool) -> bool {
    let Ok(mut guard) = STATE.try_lock() else {
        // Rather than block the whole machine's input waiting for a lock,
        // let the key through untouched. A missed cancel is a bad frame; a
        // stalled hook is a keyboard that stops responding.
        return false;
    };

    let Some(state) = guard.as_mut() else {
        return false;
    };
    if !state.on {
        return false;
    }

    let Some(group) = state.groups.iter_mut().find(|g| g.keys.contains(&vk)) else {
        return false;
    };

    if down {
        if group.held.last() == Some(&vk) {
            // The winner repeating itself. Let it through: some games read
            // repeats, and swallowing them changes nothing for those that
            // do not.
            return false;
        }

        if group.held.contains(&vk) {
            // A key that is down but not in charge, repeating. This is the
            // event that has to be swallowed -- left alone, the keyboard's
            // own auto-repeat would re-press a key we deliberately let go
            // of, and the cancel would last one repeat interval.
            return true;
        }

        // A new press takes over, and whatever was in charge is released.
        if let Some(previous) = group.held.last() {
            win32::send_inputs(&[win32::key_event(*previous as u16, true)]);
        }
        group.held.push(vk);

        // The hardware will repeat this one on its own, being the key most
        // recently pressed. Emulating it as well would double the rate.
        stop_repeat();
        return false;
    }

    let was_winner = group.held.last() == Some(&vk);
    group.held.retain(|held| *held != vk);

    if !was_winner {
        // Already released as far as anything downstream knows, so its real
        // release would be a second one.
        return true;
    }

    // The key in charge is gone. Whatever is still held underneath takes
    // over, without needing to be pressed again.
    if let Some(next) = group.held.last() {
        let next = *next;
        win32::send_inputs(&[
            win32::key_event(vk as u16, true),
            win32::key_event(next as u16, false),
        ]);

        // This is the takeover the hardware will never repeat, so it is the
        // one worth repeating ourselves.
        start_repeat(next);
        return true;
    }

    // Nothing left holding the group up.
    stop_repeat();
    false
}
