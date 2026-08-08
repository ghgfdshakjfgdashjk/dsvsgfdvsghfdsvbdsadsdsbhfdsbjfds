//! Permanent key rebinding, through Windows rather than through Syntax.
//!
//! Windows keeps a table at
//! `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layout\Scancode Map`
//! that it reads once, at boot, and applies below everything else. A key
//! remapped there is remapped for the whole machine: every game, every
//! program, the login screen, and Syntax not running at all.
//!
//! That is the point, and it is also the catch. Nothing about this is live.
//! Writing the map changes nothing until the machine restarts, and clearing
//! it likewise. There is no way to make it take effect sooner, because the
//! only thing that reads it is the keyboard driver starting up.
//!
//! Because it sits under the whole system, a wrong entry is worth taking
//! seriously: remap the key you need to type your password and you find out
//! at the login screen. Every write here goes through `clear`-able state, and
//! the interface keeps a way out that needs no keyboard beyond a mouse.

use serde::{Deserialize, Serialize};

use crate::win32;

const MAP_PATH: &str = r"SYSTEM\CurrentControlSet\Control\Keyboard Layout";
const MAP_NAME: &str = "Scancode Map";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Rebind {
    /// The key you press.
    pub from_vk: u32,
    /// What Windows reports instead. Zero switches the key off entirely.
    pub to_vk: u32,
}

impl Default for Rebind {
    fn default() -> Self {
        Rebind {
            from_vk: 0,
            to_vk: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MovementSettings {
    pub rebinds: Vec<Rebind>,
}

impl Default for MovementSettings {
    fn default() -> Self {
        MovementSettings {
            rebinds: Vec::new(),
        }
    }
}

impl MovementSettings {
    pub fn sanitised(mut self) -> Self {
        // A key can only be remapped once -- the table is read top to bottom
        // and a second entry for the same key is simply ignored, which would
        // show a rebind in the interface that Windows never honours.
        let mut seen: Vec<u32> = Vec::new();
        self.rebinds.retain(|rebind| {
            if rebind.from_vk == 0 || rebind.from_vk > 0xFF {
                return false;
            }
            if rebind.to_vk > 0xFF {
                return false;
            }
            if seen.contains(&rebind.from_vk) {
                return false;
            }
            seen.push(rebind.from_vk);
            true
        });

        self.rebinds.truncate(64);
        self
    }
}

/// What the scancode map on this machine says right now.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapState {
    /// Rebinds Windows is actually applying, read back from the registry.
    pub live: Vec<Rebind>,
    /// Whether the saved list matches what is live. False means a restart is
    /// owed before the two agree.
    pub matches: bool,
}

/// The scancode Windows uses for a virtual key.
///
/// `MAPVK_VK_TO_VSC_EX` rather than the plain version, because the plain one
/// throws away the 0xE0 prefix that separates the arrow keys, right control
/// and the rest of the extended block from the numeric keypad keys sharing
/// their scancodes. Remap an arrow with the prefix lost and you move a keypad
/// key instead.
fn scancode(vk: u32) -> u16 {
    let raw = win32::vk_to_scancode_ex(vk);
    if raw == 0 {
        return 0;
    }

    let high = (raw >> 8) & 0xFF;
    let low = raw & 0xFF;

    if high == 0xE0 || high == 0xE1 {
        (0xE000 | low) as u16
    } else {
        low as u16
    }
}

/// Build the binary blob Windows expects.
///
/// Three header words, one word per mapping, and a zero word to finish:
///
/// ```text
/// 00000000  version, always zero
/// 00000000  flags, always zero
/// 0000000N  how many entries follow, counting the terminator
/// TTTTFFFF  map scancode FFFF to scancode TTTT
/// 00000000  terminator
/// ```
///
/// The destination sits in the high half and the source in the low half,
/// which is the way round it is easy to get wrong.
fn encode(rebinds: &[Rebind]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + rebinds.len() * 4);

    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&((rebinds.len() as u32) + 1).to_le_bytes());

    for rebind in rebinds {
        let from = scancode(rebind.from_vk) as u32;
        // zero means "no key at all", which is how a key is switched off
        let to = if rebind.to_vk == 0 {
            0
        } else {
            scancode(rebind.to_vk) as u32
        };
        out.extend_from_slice(&((to << 16) | from).to_le_bytes());
    }

    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

/// Read the map back and work out which keys it moves.
///
/// Scancodes are turned back into virtual keys so the interface can name
/// them. A scancode with no virtual key -- something no keyboard here has --
/// comes back as zero rather than being dropped, so the count still matches
/// what is really in the registry.
pub fn read_live() -> Vec<Rebind> {
    let Some(blob) = crate::optimize::read_binary(MAP_PATH, MAP_NAME) else {
        return Vec::new();
    };

    if blob.len() < 16 {
        return Vec::new();
    }

    let count = u32::from_le_bytes([blob[8], blob[9], blob[10], blob[11]]) as usize;
    if count == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    // the last entry is the terminator, so it is not a mapping
    for index in 0..count.saturating_sub(1) {
        let at = 12 + index * 4;
        if at + 4 > blob.len() {
            break;
        }
        let word = u32::from_le_bytes([blob[at], blob[at + 1], blob[at + 2], blob[at + 3]]);
        let from = (word & 0xFFFF) as u16;
        let to = ((word >> 16) & 0xFFFF) as u16;

        out.push(Rebind {
            from_vk: win32::scancode_to_vk(from),
            to_vk: if to == 0 { 0 } else { win32::scancode_to_vk(to) },
        });
    }

    out
}

/// Is what is saved the same set of moves the registry is applying?
fn agrees(saved: &[Rebind], live: &[Rebind]) -> bool {
    if saved.len() != live.len() {
        return false;
    }
    saved.iter().all(|want| {
        live.iter()
            .any(|have| have.from_vk == want.from_vk && have.to_vk == want.to_vk)
    })
}

pub fn state(saved: &MovementSettings) -> MapState {
    let live = read_live();
    MapState {
        matches: agrees(&saved.rebinds, &live),
        live,
    }
}

/// Write the map, or remove it when there is nothing to write.
///
/// An empty map is deleted rather than written as a header with no entries.
/// Windows treats the two the same, but a leftover value looks like a setting
/// that is on, and someone reading their own registry deserves better.
pub fn apply(settings: &MovementSettings) -> Result<(), String> {
    if settings.rebinds.is_empty() {
        return crate::optimize::delete_value_elevated(MAP_PATH, MAP_NAME);
    }

    let blob = encode(&settings.rebinds);
    let hex: String = blob.iter().map(|byte| format!("{byte:02x}")).collect();
    crate::optimize::write_binary_elevated(MAP_PATH, MAP_NAME, &hex)
}

/// Take the map away entirely, whatever is saved.
pub fn clear() -> Result<(), String> {
    crate::optimize::delete_value_elevated(MAP_PATH, MAP_NAME)
}
