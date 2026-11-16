<p align="center"><img src="assets/padpilot.png" width="96" alt="PadPilot icon"></p>

# PadPilot

**An accessibility tool for playing racing games with a laptop touchpad.**

PadPilot is for people with a limited range of motion: people who use the whole computer
through a laptop's touchpad and cannot hold a controller, reach a keyboard for long, or
make large movements, whether from a disability they were born with or one that has grown
over time. Games are a big part of many people's lives, and losing the ability to play
should not be the price of losing some movement.

PadPilot puts a small joystick on the screen. Right-click it, and small finger movements
on the touchpad become an analog stick that the game sees as an ordinary Xbox controller:
slide up for throttle, down for brake, sideways to steer. How far the finger has to travel
is adjustable, so even a very small range of motion can reach full throttle and full lock.

## What you need

- Windows 10 or 11.
- A **Precision Touchpad** (almost every laptop from the last years has one).
- The **ViGEmBus** driver, which lets programs create a virtual Xbox controller:
  download `ViGEmBus_1.21.442_x64_x86_arm64.exe` from
  [its releases page](https://github.com/nefarius/ViGEmBus/releases) and install it once.
- A game that supports an Xbox controller. Run it in **borderless windowed** mode:
  exclusive fullscreen can hide the joystick.

## Getting started

Download the zip from the releases page and put `padpilot.exe`, `padpilot-watchdog.exe`
and `padpilot-settings.exe` in the same folder.

PadPilot is not code-signed, so Windows treats a downloaded copy with suspicion. Before
unzipping, right-click the zip, choose **Properties**, tick **Unblock** and press OK;
otherwise Windows may ask for confirmation for each of the three programs. If SmartScreen
says "Windows protected your PC", choose **More info → Run anyway**. The very first start
can also take a moment while Windows checks the new files.

Start `padpilot.exe`. The joystick appears in the bottom-right corner of the screen.

1. Start the game and click into it.
2. **Right-click the joystick.** The ring turns blue and the cursor stays on the
   joystick: this is joystick mode.
3. Put a finger on the touchpad and slide. Where the finger lands does not matter; only
   its movement counts. Lift it and the stick springs back to the center.
4. **Right-click again** to leave joystick mode and get the cursor back.

| Stick | Controller | Usual effect |
|---|---|---|
| Left / right | Left stick, sideways | Steering |
| Up | Right trigger | Throttle |
| Down | Left trigger | Brake, then reverse |

On the ring's edge you always get full input: throttle (or brake) stays at 100% while you
steer, up to the diagonal, and steering reaches full lock from the diagonal on.

## The window

In normal mode (ring not blue):

- **Move** it by dragging the title bar ("tap twice and drag" works).
- **Resize** it with the handle in the bottom-right corner, or by scrolling with two
  fingers over the joystick. It always stays square.
- The title bar buttons: **opacity** (each click steps 100% → 80% → … → 20% → 100%;
  it never gets invisible), **settings**, **minimize**, **close**.

Position, size and opacity are remembered. The window never takes focus away from the
game. In joystick mode the buttons, moving and resizing are switched off, and taps and
clicks are ignored, so a stray tap cannot close or move it.

## Safety

A locked cursor or a stuck throttle with no way out would be the worst failure, so:

- **Right-click** always leaves joystick mode.
- Joystick mode also ends by itself when **the game closes or another window comes to the
  front** (for example the Windows key). The controller returns to neutral.
- A **watchdog** runs next to PadPilot. If PadPilot stops responding for a second, the
  watchdog frees the cursor and closes it, which unplugs the virtual controller, so the
  car stops. It then tells you what happened. If the watchdog cannot run, joystick mode
  switches itself off after 60 seconds instead.
- As a last resort, **Ctrl+Alt+Del → Task Manager**: Task Manager coming to the front
  ends joystick mode, and PadPilot can be ended from there.

## Settings

The settings button opens a window with sliders and buttons (no typing needed). Changes
apply to the joystick at once, even while driving:

- **Sensitivity**: how far the finger moves for full deflection (default 10 mm; smaller
  is sharper).
- **Dead zone**: the part around the center that does nothing (default 5%).
- **When you lift your finger**: spring back to the center, or hold the position like
  cruise control.
- **Log detail**, a button to **open the logs folder**, and **restore defaults**.

The settings live in `%APPDATA%\PadPilot\config.toml`, which explains each value and can
also be edited by hand. The window's place, size and opacity are in `window.toml` next to
it.

## Logs

PadPilot, the watchdog and the settings window write daily logs to
`%LOCALAPPDATA%\PadPilot\logs` and keep the last 14 days. If something goes wrong, the
time it happened and these files are what helps find out why.

## Building from source

PadPilot is written in Rust. The executables can be built on Windows, or cross-compiled
from Linux or WSL with [cargo-xwin](https://github.com/rust-cross/cargo-xwin):

```bash
cargo install cargo-xwin just
just test    # tests that need no hardware
just check   # format and lints for Linux and Windows
just build   # executables in target/x86_64-pc-windows-msvc/release
```

On Windows itself, `cargo build --release` works too. [CONTRIBUTING.md](CONTRIBUTING.md) has
the full setup and how changes are tested. `tools/windows` has double-click
launchers for development and for checking a new machine one piece at a time
(`padpilot help` lists the test commands). The icon is drawn by the joystick's own
painting code; `cargo run -p padpilot-core --example icon` regenerates `assets/`.

| Crate | What it does |
|---|---|
| `crates/core` | Stick, controller mapping, touchpad decoding, window layout and drawing, settings. No Windows code, so it is tested anywhere. |
| `crates/app` | `padpilot.exe`: touchpad input, the virtual controller, the joystick window. |
| `crates/watchdog` | `padpilot-watchdog.exe`: frees the cursor and ends a frozen PadPilot. |
| `crates/settings` | `padpilot-settings.exe`: the settings window. |
| `crates/logging` | Log files and where PadPilot keeps its files. |
| `crates/icon` | Build helper that puts the icon into the executables. |

## Anti-cheat and online games

PadPilot does not read, change or inject anything into a game. It creates a virtual Xbox
controller through the ViGEmBus driver and shows a window on top of the game; to the game
it looks like a controller plugged in.

Some games' anti-cheat systems still block or flag virtual controllers, because the same
kind of driver is also used by macro and cheating tools. So:

- In some online games the virtual controller may simply not be recognized.
- In competitive online games there is a small risk that the anti-cheat treats it as
  suspicious. Check the game's rules, and if in doubt, ask the game's support whether
  virtual controllers are allowed for accessibility.
- Single-player and offline games are not affected by this.

## Known limitations

- Windows only for now; the code keeps Windows parts separate so other systems can follow.
- The controller mapping is fixed (made for Forza Horizon's defaults).
- ViGEmBus is no longer developed. It works today, but a future Windows update could
  break it; the controller output is kept behind one interface so another driver can
  replace it.
- An accidental right-click leaves joystick mode mid-race.

## Contributing

Bug reports, ideas and code are all welcome; [CONTRIBUTING.md](CONTRIBUTING.md) explains
how to report a problem, build and test PadPilot, and the safety and accessibility rules
every change keeps.

## Support

PadPilot is free and always will be. If it has brought a game back within reach for you
and you feel like saying thanks, you can [buy me a coffee](https://buymeacoffee.com/joaopedro.luz17)
— entirely optional, hugely appreciated.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT),
at your option.

PadPilot is built with many open-source crates; their licenses are in
`THIRD-PARTY-LICENSES.md` in each release zip.
