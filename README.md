# Blink

Blink is a small native Linux screenshot utility written in Rust. It runs without
a main window, exposes capture actions from a system tray menu, and also provides
CLI commands suitable for desktop keyboard shortcuts.

This is the first minimal version. It captures areas, screens, and windows, saves
PNG files under `~/Pictures/Blink`, and sends a desktop notification after a
successful save.

## Platform behavior

Blink uses the freedesktop Screenshot portal through `ashpd`. It does not invoke
`gnome-screenshot`, `scrot`, ImageMagick, Flameshot, or `xdg-open`.

- **Wayland:** GNOME/the compositor performs the capture and presents protected
  selection UI. Blink does not bypass Wayland security. Screenshot portal v3 can
  constrain the chooser to area, screen, or window. Older portal backends may
  ignore that target hint and show their general interactive chooser for area and
  window capture.
- **X11:** This version deliberately uses the same portal backend. Ubuntu's GNOME
  portal supports X11 sessions and gives consistent permission and chooser
  behavior. A native X11 selection backend can be added behind the existing
  `CaptureBackend` abstraction later.
- **Screens:** The compositor decides whether “screen” means one display or the
  complete desktop on a multi-monitor setup. Explicit monitor selection is not in
  this version.

The tray uses the StatusNotifierItem D-Bus specification through `ksni`. Ubuntu's
GNOME session normally includes AppIndicator support. On stock GNOME without a
StatusNotifierItem/AppIndicator extension, Blink can run but the tray icon will
not be visible.

## Ubuntu requirements

The Rust dependencies are pure Rust and do not link to GTK, Xlib, Wayland client,
or `libdbus`, so their development packages and `pkg-config` are not required.
Install a linker and the desktop portal runtime:

```bash
sudo apt update
sudo apt install build-essential xdg-desktop-portal xdg-desktop-portal-gnome \
  gnome-shell-extension-appindicator
```

Install a current Rust toolchain with [rustup](https://rustup.rs/) if `cargo` and
`rustc` are not already available. Blink uses Rust edition 2024; distribution
Rust packages that are too old will not build it.

## Build and run

```bash
cargo build
cargo run                 # tray application; no main window
cargo run -- area         # rectangular area
cargo run -- screen       # screen/desktop
cargo run -- window       # application window
```

For an optimized binary:

```bash
cargo build --release
cargo install --path .
```

The installed command is normally `~/.cargo/bin/blink`. Confirm its absolute path
with `command -v blink` before using it in desktop settings.

## Tray menu

Running `blink` without arguments registers a tray menu containing:

- Capture Area
- Capture Screen
- Capture Window
- Open Screenshots Folder
- Quit

Capture failures are written to stderr and shown as notifications while the tray
application is running. Closing the compositor chooser is treated as cancellation,
not a crash. Console logging is intentionally minimal and no log file is created.

## Keyboard shortcuts on Ubuntu GNOME

Blink does not register global shortcuts itself because GNOME owns the Print key
bindings and an application should not silently replace them.

1. Build and install Blink with `cargo install --path .`.
2. Run `command -v blink` and copy the absolute path it prints.
3. Open **Settings → Keyboard → View and Customize Shortcuts → Screenshots**.
4. Disable or change each existing GNOME binding you intentionally want Blink to
   replace. Do not remove bindings you still use.
5. Go to **Custom Shortcuts**, add these commands using the absolute path:

   | Name | Command | Binding |
   | --- | --- | --- |
   | Blink — Area | `/home/you/.cargo/bin/blink area` | `Print` |
   | Blink — Screen | `/home/you/.cargo/bin/blink screen` | `Shift+Print` |
   | Blink — Window | `/home/you/.cargo/bin/blink window` | `Alt+Print` |

GNOME will report a conflict when a binding is still assigned. Resolve it
explicitly rather than allowing Blink to override it silently. The shortcuts
launch short-lived CLI processes; the tray does not have to be running.

## Desktop launcher and optional autostart

After `cargo install --path .`, install the launcher for the current user:

```bash
mkdir -p ~/.local/share/applications
cp data/dev.blink.Blink.desktop ~/.local/share/applications/
```

If `blink` is not on the graphical session's `PATH`, edit the copied desktop file
and replace `Exec=blink` with the absolute path from `command -v blink`.

To opt into launch-at-login later, copy the same launcher; development does not
enable this automatically:

```bash
mkdir -p ~/.config/autostart
cp data/dev.blink.Blink.desktop ~/.config/autostart/
```

## Manual test checklist

Run these from a graphical terminal in the Ubuntu desktop session:

1. `cargo run -- area` — select a rectangle, then verify a PNG and notification.
2. `cargo run -- screen` — approve the portal if prompted and verify the result.
3. `cargo run -- window` — select a window in the compositor UI and verify it.
4. Cancel the area/window chooser and verify Blink exits without a panic or file.
5. `cargo run` — verify the five tray actions and that Quit removes the icon.
6. Take two captures within one second and verify the second filename has `-1`.

## Known limitations

- Exact target filtering requires Screenshot portal v3. Older Ubuntu portal
  backends can present a general chooser even when `area` or `window` was asked
  for.
- Multi-monitor selection and a dedicated native X11 backend are not implemented.
- Tray visibility depends on the desktop providing a StatusNotifierItem watcher.
- There is no single-instance guard, so launching `blink` repeatedly without a
  subcommand can create more than one tray process.
- GNOME shortcut setup is manual by design.

## Next steps

The next smallest improvement is a single-instance D-Bus application service:
subsequent `blink` invocations could forward capture requests to the existing tray
process instead of creating separate short-lived processes. After that, a native
X11 backend can be added without changing the CLI, saving, or tray layers.
