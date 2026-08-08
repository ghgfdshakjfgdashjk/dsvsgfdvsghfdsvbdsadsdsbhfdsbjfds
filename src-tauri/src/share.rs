use serde::{Deserialize, Serialize};

use crate::settings::Settings;

/// Bumped if the shape ever changes in a way older builds cannot read.
const VERSION: u32 = 1;
const PREFIX: &str = "SYN1-";

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[derive(Serialize, Deserialize)]
struct Share {
    v: u32,
    /// What the code carries:
    ///
    /// * `all` — the lot, clickers included.
    /// * `clicker` — only the clickers.
    /// * `settings` — everything except the clickers, so whoever pastes it
    ///   keeps their own.
    scope: String,
    settings: Settings,
}

/// Base64, url-safe alphabet, no padding. Hand-rolled so sharing a config
/// costs the project no dependencies.
fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let packed = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;

        out.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        out.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(packed >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[packed as usize & 63] as char);
        }
    }

    out
}

fn value_of(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|a| *a == c).map(|i| i as u32)
}

fn decode(text: &str) -> Result<Vec<u8>, String> {
    let clean: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();

    let mut out = Vec::with_capacity(clean.len() / 4 * 3);

    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            return Err("the code is cut short".into());
        }

        let mut packed = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            let v = value_of(*c).ok_or("the code has characters that do not belong")?;
            packed |= v << (18 - 6 * i);
        }

        out.push((packed >> 16) as u8);
        if chunk.len() > 2 {
            out.push((packed >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(packed as u8);
        }
    }

    Ok(out)
}

/// Settings that are about this machine rather than about how you play, and
/// so have no business travelling with a shared code.
fn strip_local(settings: &mut Settings) {
    settings.cursor_image = String::new();
    settings.window_width = 0.0;
    settings.window_height = 0.0;
    // Rebinds are a change to the machine, not a preference. They belong to
    // whoever's keyboard it is, and nobody should be handing them around in
    // a code.
    settings.movement = crate::movement::MovementSettings::default();
}

pub fn export(settings: &Settings, scope: &str) -> String {
    let mut copy = settings.clone();
    strip_local(&mut copy);

    match scope {
        "clicker" => {
            // only the clickers travel, so flatten everything else to defaults
            let profiles = copy.profiles.clone();
            let selected = copy.selected;
            copy = Settings::default();
            copy.profiles = profiles;
            copy.selected = selected;
        }
        "settings" => {
            // The clickers are the one thing this scope does not carry, and
            // they are taken out here rather than ignored on the way in. A
            // clicker can hold a custom sequence, and a code you hand out is
            // not the place for one to be sitting where it is not wanted.
            let stock = Settings::default();
            copy.profiles = stock.profiles;
            copy.selected = stock.selected;
        }
        _ => {}
    }

    let share = Share {
        v: VERSION,
        scope: scope.to_string(),
        settings: copy,
    };

    match serde_json::to_vec(&share) {
        Ok(json) => format!("{PREFIX}{}", encode(&json)),
        Err(_) => String::new(),
    }
}

/// Read a code and fold it into what is already here.
///
/// Anything local to this machine is kept: a shared code should never take
/// away someone's cursor image or resize their window.
pub fn import(code: &str, current: &Settings) -> Result<Settings, String> {
    let trimmed = code.trim();
    let body = trimmed
        .strip_prefix(PREFIX)
        .or_else(|| trimmed.strip_prefix(PREFIX.to_lowercase().as_str()))
        .ok_or("That is not a Syntax code — they start with SYN1-")?;

    if body.is_empty() {
        return Err("That code is empty".into());
    }

    let raw = decode(body)?;
    let share: Share =
        serde_json::from_slice(&raw).map_err(|_| "That code is damaged or from a newer build")?;

    if share.v > VERSION {
        return Err("That code came from a newer version of Syntax".into());
    }

    let mut next = current.clone();

    if share.scope == "clicker" {
        next.profiles = share.settings.profiles;
        next.selected = share.settings.selected;
    } else {
        let cursor = next.cursor_image.clone();
        let width = next.window_width;
        let height = next.window_height;
        let presets = next.presets.clone();
        let profiles = next.profiles.clone();
        let selected = next.selected;
        let rebinds = next.movement.clone();

        next = share.settings;

        next.cursor_image = cursor;
        next.window_width = width;
        next.window_height = height;
        // never taken from a code, and never taken away by one
        next.movement = rebinds;
        // your saved profiles are yours; a code should not replace the shelf
        next.presets = presets;

        // A settings code is everything but the clickers, so the ones already
        // here stay put. Belt and braces: the sender took theirs out too, and
        // without this the stock clickers left in their place would land here
        // and quietly wipe yours.
        if share.scope == "settings" {
            next.profiles = profiles;
            next.selected = selected;
        }
    }

    Ok(next.sanitised())
}

/// What a code contains, without applying it.
pub fn describe(code: &str) -> Result<String, String> {
    let trimmed = code.trim();
    let body = trimmed
        .strip_prefix(PREFIX)
        .ok_or("That is not a Syntax code — they start with SYN1-")?;

    let raw = decode(body)?;
    let share: Share =
        serde_json::from_slice(&raw).map_err(|_| "That code is damaged or from a newer build")?;

    let count = share.settings.profiles.len();
    let clickers = if count == 1 { "clicker" } else { "clickers" };

    Ok(match share.scope.as_str() {
        "clicker" => format!("{count} {clickers}"),
        "settings" => "everything except clickers — yours are kept".to_string(),
        _ => format!("everything, including {count} {clickers}"),
    })
}
