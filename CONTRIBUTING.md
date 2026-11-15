# Contributing to PadPilot

Thanks for wanting to help. PadPilot is an accessibility tool, so every contribution is
measured against one question: does it still work for someone who controls the whole
computer with small movements on a touchpad? This guide covers how the code is laid out,
how to build and test it, and the few rules that keep it safe for the people who rely on
it.

## Reporting a problem

Open an issue and include:

- What you did, what you expected, and what happened.
- Roughly **when** it happened (the time of day is enough).
- The log files from that day, from `%LOCALAPPDATA%\PadPilot\logs`
  (`padpilot.*.log`, `watchdog.*.log` and `settings.*.log`). The settings window's
  **Open logs folder** button gets you there.
- Your touchpad and game, and whether the game runs windowed or fullscreen.

For delay or responsiveness problems, a telemetry recording helps most: run
`padpilot telemetry` (or `tools\windows\Record telemetry.cmd`), drive for a minute,
close the window, and attach the CSV file it names. It records, for every input event,
how long it waited and how long each step took.

## Project structure

```
crates/
  core/       all the logic, with no Windows code: tested on any system
    src/stick.rs         finger movement → stick position (sensitivity, dead zone, lift)
    src/gamepad.rs       stick position → controller (steering, throttle, brake)
    src/touchpad.rs      decoding Precision Touchpad reports
    src/overlay/         window layout, behaviour per mode, and drawing
    src/config.rs        config.toml: reading, and writing while keeping comments
    src/window_state.rs  window.toml: place, size, opacity
    src/watchdog.rs      what the watchdog does about what it hears
    examples/            preview.rs (renders the window), icon.rs (makes assets/)
  app/        padpilot.exe: Raw Input, ViGEmBus, the joystick window
  watchdog/   padpilot-watchdog.exe
  settings/   padpilot-settings.exe (egui)
  logging/    log files and where PadPilot keeps its files
  icon/       build helper that embeds assets/padpilot.ico into the executables
tools/windows/  double-click launchers for testing on a Windows machine
```

Decisions live in `core`, as plain functions and types that tests can call directly.
The other crates are thin: they turn Windows messages into calls to `core` and carry
out what it returns. Keep it that way; a feature that needs a decision belongs in
`core` with its tests, and only the Windows calls go in `app`.

## Building

You need [Rust](https://rustup.rs) and [just](https://github.com/casey/just).

**On Linux or WSL**, the Windows executables are cross-compiled with
[cargo-xwin](https://github.com/rust-cross/cargo-xwin):

```bash
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin just
just test    # every test that needs no hardware
just check   # rustfmt, and clippy with warnings as errors, for Linux and Windows
just build   # target/x86_64-pc-windows-msvc/release/*.exe
```

**On Windows**, `cargo test --workspace` and `cargo build --release` work directly.

To run PadPilot you need a Precision Touchpad and the
[ViGEmBus driver](https://github.com/nefarius/ViGEmBus/releases). The three executables
must sit in the same folder.

## Testing

### Automated

Behaviour is developed test first: write a test that fails for the reason you expect,
then the smallest change that makes it pass. Tests live in each crate's `tests/` folder
and call the code the way the program does, checking what a user would notice (where
the knob is, what the controller reports, which pixels are transparent), not how the
code gets there.

Before sending a change:

```bash
just check
just test
cargo mutants -p padpilot-core -f <the file you changed>   # cargo install cargo-mutants
```

[cargo-mutants](https://mutants.rs) plants small bugs in the code and checks that a test
catches each one. A surviving mutant usually means a missing test; cover it, or delete
the code if nothing needs it. Mutants that cannot change behaviour (for example in pure
shading arithmetic) are fine to leave.

### On Windows

Some things only a real touchpad and a real game can prove. `tools\windows` has a
launcher for each piece:

| Launcher | What it checks |
|---|---|
| `Test throttle.cmd` | The virtual controller: holds 50% throttle for 30 s. |
| `Test touchpad.cmd` | Touchpad input: logs every touch, move and lift. |
| `Test drive.cmd` | Touchpad to controller, without the window. |
| `Test overlay.cmd` | The joystick, with joystick mode ending after 10 s. |
| `Test freeze.cmd` | The watchdog: PadPilot freezes on purpose after 10 s in joystick mode. |
| `Record telemetry.cmd` | Normal use, recording input timing to a CSV file. |
| `PadPilot.cmd` | Normal use. |

Say in your pull request what you tested this way, and with which game.

## Rules for changes

### Safety

The person using PadPilot may have no other way to control the computer. A cursor that
stays locked, or a throttle that stays pressed, is the worst thing that can happen, so a
change must never weaken these:

- Right-click always leaves joystick mode.
- Leaving joystick mode, for any reason, frees the cursor and returns the controller
  to neutral.
- Joystick mode ends when the game closes or another window comes to the front.
- The watchdog frees the cursor and ends PadPilot when the heartbeat stops. Anything
  that can block the window's thread for long (file access, waiting on another
  process) does not belong on it.
- Nothing in joystick mode can close, move or resize the window.

If your change touches joystick mode, the cursor lock, the controller or the watchdog,
run `Test freeze.cmd` and confirm the car stops and the cursor is free.

### Accessibility

- Everything works with a touchpad alone: no keyboard shortcuts as the only way to do
  something, and no text fields that would need the on-screen keyboard.
- Click targets are large (about 40 px at 100% scaling, the settings window's buttons).
- The joystick window never takes focus from the game.
- Respect Windows display scaling: sizes in `core` are given at 100% and scaled.

### Code

- Platform-specific code stays in `app`, `watchdog`, `settings` and `logging`, so a
  macOS version can follow later.
- Every `unsafe` block has a `// SAFETY:` comment saying why it is sound.
- Outside data (touchpad reports, config files) is checked where it enters; past that
  point the types guarantee it is valid.
- Logging uses `tracing` with the component as `target` (`reader`, `stick`, `controller`,
  `overlay`, `watchdog`, `config`, `settings`). Per-event data goes at `trace`, never
  at `info`.

### Commits and pull requests

- Small commits, each one doing one thing, with a short message that says what changed
  (`Add opacity button`, `Hide the settings window through Windows when its close
  button is clicked`). The body, when needed, says why.
- One topic per pull request. Describe what changed, why, and how you checked it.

## Branches

- **`develop`** is where work comes together. Branch from it, and open pull requests
  against it.
- **`main`** only receives releases: when `develop` is ready, it is merged into `main`
  and the release is tagged there.

## Continuous integration

Every push to `main` or `develop`, and every pull request into them, runs
[CI](.github/workflows/ci.yml): on
Linux, formatting, clippy and the logic tests; on Windows, clippy, every test and a
release build, whose executables are attached to the run as an artifact. A pull request
needs both to pass. [Mutation testing](.github/workflows/mutants.yml) runs weekly and can
be started by hand from the Actions tab.

## Releasing

1. On `develop`, set the new version in the root `Cargo.toml` and commit it
   (`Release 0.9.0`).
2. Merge `develop` into `main`, and tag the merged commit with the release notes as the
   message: the first line becomes the release title, the rest its notes.
   ```bash
   git switch main
   git merge --ff-only develop
   git tag -a v0.9.0 -m "PadPilot 0.9.0" -m "What changed, for users."
   git push origin main v0.9.0
   git switch develop
   ```
3. The [release workflow](.github/workflows/release.yml) checks that the tag is on
   `main` and matches the version, runs the tests, builds on Windows and publishes the
   GitHub release with the zip and its SHA-256 checksum.

## The icon

The icon is drawn by the same code that draws the joystick (`paint_icon` in
`crates/core/src/overlay/paint.rs`). After changing it, regenerate the files and rebuild:

```bash
cargo run -p padpilot-core --example icon
```

## License

By contributing, you agree that your contributions are licensed under the project's
terms: either the [Apache License 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT),
at your option.
