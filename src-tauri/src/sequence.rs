use crate::win32::{self, ButtonSpec, INPUT};

#[derive(Clone, Copy)]
pub enum Msg {
    Mouse { msg: u32, wparam: usize },
    Key { msg: u32, vk: u16 },
}

#[derive(Clone)]
pub enum Step {

    Mouse { spec: ButtonSpec, label: String },

    MouseHalf {
        spec: ButtonSpec,
        press: bool,
        label: String,
    },

    Key { vk: u16, shift: bool, label: String },

    KeyHalf { vk: u16, press: bool, label: String },

    Unicode { unit: u16, label: String },
    Scroll { amount: i32, label: String },
    Wait { ms: f64, label: String },
}

impl Step {
    pub fn label(&self) -> String {
        match self {
            Step::Mouse { label, .. }
            | Step::MouseHalf { label, .. }
            | Step::Key { label, .. }
            | Step::KeyHalf { label, .. }
            | Step::Unicode { label, .. }
            | Step::Scroll { label, .. }
            | Step::Wait { label, .. } => label.clone(),
        }
    }

    pub fn is_wait(&self) -> bool {
        matches!(self, Step::Wait { .. })
    }

    pub fn press(&self) -> Vec<INPUT> {
        match self {
            Step::Mouse { spec, .. } => vec![win32::mouse_event(spec, spec.down)],
            Step::MouseHalf { spec, press, .. } => {
                vec![win32::mouse_event(spec, if *press { spec.down } else { spec.up })]
            }
            Step::Key { vk, shift, .. } => {
                let mut out = Vec::with_capacity(2);
                if *shift {
                    out.push(win32::key_event(win32::VK_SHIFT, false));
                }
                out.push(win32::key_event(*vk, false));
                out
            }
            Step::KeyHalf { vk, press, .. } => vec![win32::key_event(*vk, !*press)],
            Step::Unicode { unit, .. } => vec![win32::unicode_event(*unit, false)],
            Step::Scroll { amount, .. } => vec![win32::wheel_event(*amount)],
            Step::Wait { .. } => Vec::new(),
        }
    }

    pub fn release(&self) -> Vec<INPUT> {
        match self {
            Step::Mouse { spec, .. } => vec![win32::mouse_event(spec, spec.up)],
            Step::Key { vk, shift, .. } => {
                let mut out = Vec::with_capacity(2);
                out.push(win32::key_event(*vk, true));
                if *shift {
                    out.push(win32::key_event(win32::VK_SHIFT, true));
                }
                out
            }
            Step::Unicode { unit, .. } => vec![win32::unicode_event(*unit, true)],
            Step::MouseHalf { .. } | Step::KeyHalf { .. } | Step::Scroll { .. } | Step::Wait { .. } => {
                Vec::new()
            }
        }
    }

    pub fn full(&self) -> Vec<INPUT> {
        let mut out = self.press();
        out.extend(self.release());
        out
    }

    pub fn messages(&self) -> (Vec<Msg>, Vec<Msg>) {
        match self {
            Step::Mouse { spec, .. } => {
                let (down, up, wparam) = mouse_messages(spec);
                (
                    vec![Msg::Mouse { msg: down, wparam }],
                    vec![Msg::Mouse { msg: up, wparam: 0 }],
                )
            }
            Step::MouseHalf { spec, press, .. } => {
                let (down, up, wparam) = mouse_messages(spec);
                let msg = if *press { down } else { up };
                (
                    vec![Msg::Mouse {
                        msg,
                        wparam: if *press { wparam } else { 0 },
                    }],
                    Vec::new(),
                )
            }
            Step::Key { vk, shift, .. } => {
                let mut press = Vec::with_capacity(2);
                if *shift {
                    press.push(Msg::Key {
                        msg: win32::WM_KEYDOWN,
                        vk: win32::VK_SHIFT,
                    });
                }
                press.push(Msg::Key {
                    msg: win32::WM_KEYDOWN,
                    vk: *vk,
                });

                let mut release = Vec::with_capacity(2);
                release.push(Msg::Key {
                    msg: win32::WM_KEYUP,
                    vk: *vk,
                });
                if *shift {
                    release.push(Msg::Key {
                        msg: win32::WM_KEYUP,
                        vk: win32::VK_SHIFT,
                    });
                }
                (press, release)
            }
            Step::KeyHalf { vk, press, .. } => (
                vec![Msg::Key {
                    msg: if *press {
                        win32::WM_KEYDOWN
                    } else {
                        win32::WM_KEYUP
                    },
                    vk: *vk,
                }],
                Vec::new(),
            ),
            Step::Unicode { unit, .. } => {
                match char::from_u32(*unit as u32).and_then(win32::char_to_vk) {
                    Some((vk, _)) => (
                        vec![Msg::Key {
                            msg: win32::WM_KEYDOWN,
                            vk,
                        }],
                        vec![Msg::Key {
                            msg: win32::WM_KEYUP,
                            vk,
                        }],
                    ),
                    None => (Vec::new(), Vec::new()),
                }
            }

            Step::Scroll { .. } | Step::Wait { .. } => (Vec::new(), Vec::new()),
        }
    }
}

fn mouse_messages(spec: &ButtonSpec) -> (u32, u32, usize) {
    match spec.vk {
        0x02 => (win32::WM_RBUTTONDOWN, win32::WM_RBUTTONUP, win32::MK_RBUTTON),
        0x04 => (win32::WM_MBUTTONDOWN, win32::WM_MBUTTONUP, win32::MK_MBUTTON),
        0x05 => (
            win32::WM_XBUTTONDOWN,
            win32::WM_XBUTTONUP,

            (win32::XBUTTON1_W << 16) | win32::MK_XBUTTON1,
        ),
        0x06 => (
            win32::WM_XBUTTONDOWN,
            win32::WM_XBUTTONUP,
            (win32::XBUTTON2_W << 16) | win32::MK_XBUTTON2,
        ),
        _ => (win32::WM_LBUTTONDOWN, win32::WM_LBUTTONUP, win32::MK_LBUTTON),
    }
}

pub fn single_click(button: &str) -> Vec<Step> {
    let spec = win32::button_spec(button);
    vec![Step::Mouse {
        spec,
        label: button_label(button).into(),
    }]
}

fn button_label(button: &str) -> &'static str {
    match button {
        "right" => "RMB",
        "middle" => "MMB",
        "mouse4" => "MB4",
        "mouse5" => "MB5",
        _ => "LMB",
    }
}

fn button_from_token(token: &str) -> Option<&'static str> {
    Some(match token {
        "LMB" | "LEFT" | "LCLICK" => "left",
        "RMB" | "RIGHT" | "RCLICK" => "right",
        "MMB" | "MIDDLE" | "MCLICK" => "middle",
        "MB4" | "MOUSE4" | "M4" => "mouse4",
        "MB5" | "MOUSE5" | "M5" => "mouse5",
        _ => return None,
    })
}

fn named_key(token: &str) -> Option<u16> {
    let vk = match token {
        "ENTER" | "RETURN" => 0x0D,
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ESC" | "ESCAPE" => 0x1B,
        "BACKSPACE" | "BKSP" => 0x08,
        "DELETE" | "DEL" => 0x2E,
        "INSERT" | "INS" => 0x2D,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDN" => 0x22,
        "UP" => 0x26,
        "DOWN" => 0x28,
        "LEFTARROW" => 0x25,
        "RIGHTARROW" => 0x27,
        "SHIFT" => 0x10,
        "CTRL" | "CONTROL" => 0x11,
        "ALT" => 0x12,
        "CAPS" | "CAPSLOCK" => 0x14,
        _ => {

            let rest = token.strip_prefix('F')?;
            let n: u16 = rest.parse().ok()?;
            if (1..=24).contains(&n) {
                0x6F + n
            } else {
                return None;
            }
        }
    };
    Some(vk)
}

fn key_from_token(token: &str) -> Option<u16> {
    if let Some(vk) = named_key(token) {
        return Some(vk);
    }
    let mut chars = token.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    win32::char_to_vk(first).map(|(vk, _)| vk)
}

fn char_step(ch: char) -> Option<Step> {

    if ch.is_whitespace() {
        return None;
    }
    match win32::char_to_vk(ch) {
        Some((vk, shift)) => Some(Step::Key {
            vk,
            shift,
            label: ch.to_string(),
        }),
        None => u16::try_from(ch as u32).ok().map(|unit| Step::Unicode {
            unit,
            label: ch.to_string(),
        }),
    }
}

fn token_step(token: &str) -> Option<Step> {

    if let Some((name, arg)) = token.split_once(':') {
        let name = name.trim();
        let arg = arg.trim();

        return match name {
            "WAITMS" | "WAIT" | "MS" => {
                let ms: f64 = arg.parse().ok()?;
                Some(Step::Wait {
                    ms: ms.clamp(0.0, 600_000.0),
                    label: format!("{ms} ms"),
                })
            }
            "WAITS" | "S" => {
                let seconds: f64 = arg.parse().ok()?;
                Some(Step::Wait {
                    ms: (seconds * 1000.0).clamp(0.0, 600_000.0),
                    label: format!("{seconds} s"),
                })
            }
            "SCROLL" => {
                let amount: i32 = arg.parse().ok()?;
                Some(Step::Scroll {
                    amount: amount.clamp(-50, 50),
                    label: format!("scroll {amount}"),
                })
            }
            "KEYDOWN" | "KD" => key_from_token(arg).map(|vk| Step::KeyHalf {
                vk,
                press: true,
                label: format!("{arg}↓"),
            }),
            "KEYUP" | "KU" => key_from_token(arg).map(|vk| Step::KeyHalf {
                vk,
                press: false,
                label: format!("{arg}↑"),
            }),
            _ => None,
        };
    }

    if token.len() > 1 {
        let (head, tail) = token.split_at(token.len() - 1);
        if let Some(button) = button_from_token(head) {
            let press = match tail {
                "D" => true,
                "U" => false,
                _ => return whole_token_step(token),
            };
            return Some(Step::MouseHalf {
                spec: win32::button_spec(button),
                press,
                label: format!("{}{}", button_label(button), if press { "↓" } else { "↑" }),
            });
        }
    }

    whole_token_step(token)
}

fn whole_token_step(token: &str) -> Option<Step> {
    if let Some(button) = button_from_token(token) {
        return single_click(button).into_iter().next();
    }
    named_key(token).map(|vk| Step::Key {
        vk,
        shift: false,
        label: token.to_string(),
    })
}

pub fn parse(text: &str) -> Vec<Step> {
    let chars: Vec<char> = text.chars().collect();
    let mut steps = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '{' {
            if let Some(offset) = chars[index + 1..].iter().position(|c| *c == '}') {
                let token: String = chars[index + 1..index + 1 + offset]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_ascii_uppercase();
                index += offset + 2;

                if let Some(step) = token_step(&token) {
                    steps.push(step);
                }
                continue;
            }
        }

        if let Some(step) = char_step(chars[index]) {
            steps.push(step);
        }
        index += 1;
    }

    steps
}

pub fn has_waits(steps: &[Step]) -> bool {
    steps.iter().any(Step::is_wait)
}

pub fn unit_inputs(steps: &[Step]) -> Vec<INPUT> {
    let mut out = Vec::new();
    for step in steps {
        out.extend(step.full());
    }
    out
}

pub fn unit_messages(steps: &[Step]) -> Vec<Msg> {
    let mut out = Vec::new();
    for step in steps {
        let (press, release) = step.messages();
        out.extend(press);
        out.extend(release);
    }
    out
}

pub enum Emission {
    Fire { inputs: Vec<INPUT>, msgs: Vec<Msg> },
    Wait(f64),
}

pub fn emissions(steps: &[Step]) -> Vec<Emission> {
    steps
        .iter()
        .map(|step| match step {
            Step::Wait { ms, .. } => Emission::Wait(*ms),
            _ => {
                let (press, release) = step.messages();
                let mut msgs = press;
                msgs.extend(release);
                Emission::Fire {
                    inputs: step.full(),
                    msgs,
                }
            }
        })
        .collect()
}
