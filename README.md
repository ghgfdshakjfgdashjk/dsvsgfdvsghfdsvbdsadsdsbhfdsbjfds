# Syntax

A very fast Windows autoclicker, with game macros and a live overlay, behind a
blue glassmorphism interface. Tauri v2 — Rust does the clicking, TypeScript
draws the glass.

All code here is original.

## Running it

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # produces an installer + .exe in src-tauri/target/release
```

Windows only — the input layer talks to `SendInput` and DWM directly.

## How it gets to 50,000 CPS

A naive autoclicker calls `SendInput` once per click, so the click rate is
capped by the syscall rate. Past a few thousand a second the overhead dominates
and the loop can't keep up.

Two things make the difference here.

**Batching.** One repetition — a single click, or a whole custom sequence — is
flattened into a contiguous `INPUT` array up front. The engine then submits many
copies of it in a single `SendInput` call, so the syscall rate stays flat while
the click rate scales.

**A credit model instead of a fixed schedule.** This is the subtle one. Windows
routinely overshoots a 1 ms sleep to 1.5 ms or worse. If you decide the batch
size in advance and just sleep between batches, every overshoot silently drops
clicks — ask for 5,000 CPS and you measure ~3,500.

So the loop doesn't schedule batches at all. Each wake-up it adds
`elapsed × rate` to a running credit balance and fires however many whole
repetitions that buys. An overshoot means the next batch is proportionally
bigger, so the *average* rate lands on target regardless of how the scheduler
behaves. Credit is capped so a long stall can't discharge as one enormous burst,
and it's zeroed when the window filter blocks clicking.

`timeBeginPeriod(1)` is held for the lifetime of the engine so short sleeps stay
short, and **Max** precision mode finishes each wait on a spin for
sub-millisecond wake-ups.

Batching only works for instant clicks. Raise the duty cycle above 0 and the
engine drops to discrete press → wait → release, which caps you at a few hundred
CPS by definition.

## Delivery: system-wide vs target window

This is the setting that matters most at high rates.

**System-wide** uses `SendInput`. Every event goes through Windows' single Raw
Input Thread, which serialises *all* input on the machine, and every low-level
mouse hook installed by other software — Discord's overlay, RGB suites,
recording tools — runs on each one. At 5,000 CPS that's 10,000 events a second
plus hook callbacks, and your real mouse movement queues up behind the backlog.
The symptom is cursor lag followed by a teleport when the queue drains. Nothing
can fix that; it's what injecting into the global input stream costs. Other
autoclickers don't show it only because they never get fast enough to saturate
the thread.

**Target window** posts messages (`WM_LBUTTONDOWN` and friends) directly to a
window's message queue instead. It resolves whatever sits under the cursor at
the moment you activate — `WindowFromPoint`, so it gets the deepest child
window, which is what browsers and most real applications actually listen on —
and fires at that from then on. Your physical cursor is never touched, no hooks
run, and nothing competes with your real mouse, so high rates stay smooth. You
can even move the mouse away and keep clicking the original window.

The tradeoff: anything reading raw input rather than the message queue will
ignore posted messages. Most full-screen games fall into that category.
Browsers, launchers, and ordinary desktop applications generally don't.

Target window is the default, because it's the one that actually delivers the
rate you ask for. Measured on a machine with Discord and RGB software running:
system-wide topped out around 1–2k CPS with audible driver complaints and a
stuttering cursor, while target window hit a requested 5,000 exactly, silently.

Switch to system-wide only when the target ignores posted messages — typically a
full-screen game — and keep the rate modest when you do.

## Controls

**Clicker**

- **Activation mode** — Toggle (tap to start, tap to stop) or Hold (clicks only
  while the bind is physically down).
- **Keybind** — any key or mouse button, including Mouse 4 / Mouse 5. Click the
  keycap and press what you want; Esc cancels.
- **Delivery** — system-wide or target window, as above. In target-window mode
  a readout shows what's under the cursor, and once active it shows what got
  latched onto.
- **Mouse button** — left, right, middle, Mouse 4 or Mouse 5.
- **CPS** — 1 to 50,000 on a logarithmic slider, with 10 / 50 / 500 / 5k / 50k
  presets.
- **Click duty cycle** — how much of each click's period the button stays held,
  as a percentage. The readout shows what that works out to in milliseconds at
  your current rate. 0 = instant.
- **Custom sequence** — see below.

**Timing**

- **Randomise rate** — draws a fresh CPS between min and max for every click.
- **Interval jitter** — wobbles each gap by ± a percentage so spacing is never
  perfectly even.
- **Precision** — Balanced (sleep-driven, light on CPU) or Max (spin-driven,
  exact, keeps one core busy while active).

**Rules**

- **Click limit** — stop automatically after N clicks. Enforced inside a batch
  too, so it never overshoots.
- **Window filter** — only click while the focused window title contains a given
  string.
- **Panic key** — instantly stops the clicker in any mode. Defaults to F12.

**Look**

- **Blur behind window** — real DWM blur, clipped to the same rounded rectangle
  the CSS shell uses.
- **Acrylic** — richer, grainier blur. Can make dragging feel laggy on Windows 10.
- **Panel opacity** and **always on top**.

## Macros

Each one is a dropdown on the Macros tab with its own hotkey, all off by
default. They're built for specific Roblox games and mostly work by reading the
screen or driving hotbar slots.

- **Fisher** — watches for the fishing minigame and plays it. Pick which fish to
  keep; anything switched off is cancelled instead of caught. It finds the
  slider by colour and steers it with the mouse button.
- **Gumdrop** — swap to the gumdrop, place it, swap to the pickaxe, break it,
  swap back to the sword. Every step's timing is adjustable.
- **Skywars** — open a chest, press the key, and it takes everything inside. It
  finds the grid by looking for solid squares of the slot colour, and only ever
  clicks rectangles it actually saw, so it can't stray onto the inventory beside
  it. A slot counts as full if it differs in colour, shows sprite edges, or has
  a spread of brightness — any one is enough, so pale, dark and brown items all
  register.
- **Davey** — holds a key, swaps to the pickaxe while it's still down, then
  clicks flat out the instant it's released.
- **Crossbow** — swap to the crossbow, shoot, swap back to the sword.

Two details that turned out to matter across all of them. A game reads the mouse
once a frame, so a press and release sent in the same instant can fall between
two frames and never register — every macro holds buttons down for a real
duration rather than pressing and releasing at once. And short `thread::sleep`
calls overshoot by a whole timer tick on Windows, which at these lengths is most
of the delay, so anything under about 1.5 ms spins instead.

## Overlay

A small always-on-top readout of whichever clicker is running, in the app's own
gradient. It's a separate borderless transparent window that ignores the cursor,
so clicks pass straight through to the game and it can never steal focus.

Put it in any corner or at exact screen coordinates. Corners are worked out from
the monitor it's actually on rather than assumed, so second screens and display
scaling both land correctly. Optionally restrict it to certain windows — give it
a list of names and it appears only while one of those is in front.

## Custom sequences

Instead of one plain click, a repetition can be a short script. Braced tokens
name buttons and special keys; everything else is typed literally.

```
{LMB}eee        left click, then press e three times
{RMB}{SPACE}    right click, then space
qq{MB4}         q, q, then mouse button 4
{SHIFT}{F5}     shift, then F5
```

The whole script counts as **one repetition**, so the CPS setting becomes
repetitions per second — `{LMB}eee` at 10 CPS fires 10 clicks and 30 key presses
every second. Sequences batch exactly like plain clicks do, so they're just as
fast.

Buttons: `{LMB}` `{RMB}` `{MMB}` `{MB4}` `{MB5}`
Keys: `{SPACE}` `{ENTER}` `{TAB}` `{ESC}` `{BACKSPACE}` `{DELETE}` `{SHIFT}`
`{CTRL}` `{ALT}` `{UP}` `{DOWN}` `{LEFTARROW}` `{RIGHTARROW}` `{HOME}` `{END}`
`{PGUP}` `{PGDN}` `{F1}`–`{F24}`

Literal characters resolve through the active keyboard layout with
`VkKeyScanW`, and are sent with a real scan code so games that read raw input
still see them. Capitals get an automatic shift. Characters the layout can't
produce fall back to unicode injection. Unrecognised tokens are dropped rather
than typed out, so a typo costs you one step instead of a burst of junk. The
preview strip under the input shows exactly what will fire.

When sequence mode is on, the **Mouse button** setting is ignored — the script
says what gets pressed.

A sequence that presses without releasing can't leave a button stuck down: any
button or key still held when you stop is released for you.

## A note on binds

If the bind is set to the same mouse button the clicker presses, the synthetic
clicks will feed straight back into the bind watcher. The UI flags this with a
warning — pick a different button or a keyboard key.

## Layout

```
src/
  main.ts            UI state, wiring, events
  styles.css         blue glassmorphism
  overlay.ts         the CPS overlay window
  overlay.css        its styling
index.html           shell markup
overlay.html         overlay markup, built as a second page
src-tauri/src/
  lib.rs             Tauri commands, tray, window glass
  engine.rs          the click loop
  clickers.rs        the clicker profiles
  hotkeys.rs         global bind watcher
  automation.rs      recorded step playback
  recorder.rs        input recording
  sequence.rs        {LMB}eee parser + INPUT builder
  optimize.rs        Windows tweaks and cleanup
  overlay.rs         overlay window and placement
  fisher.rs          fishing macro
  gumdrop.rs         gumdrop macro
  skywars.rs         chest looting macro
  davey.rs           davey macro
  crossbow.rs        crossbow macro
  win32.rs           hand-rolled Win32 FFI
  settings.rs        config + persistence
```

Settings are saved to the app config directory as `settings.json` and reloaded
on launch.
