# Blink

Blink is a small native Linux screenshot application written in Rust and GTK 4.
It provides a compact capture window, system tray integration, and user-configured
global shortcuts without a browser engine or permanently large interface.

The first version deliberately stays focused on three actions:

- Capture Area
- Capture Screen
- Capture Window

Screenshots are saved as PNG files under `~/Pictures/Blink` and followed by a
native desktop notification.

## Application interface

Running `blink` opens Blink's own GTK window. The three capture cards hide the
window before asking the compositor to capture. The application menu provides:

- Open Screenshots Folder
- Configure Shortcuts
- Quit Blink

Closing the window hides it instead of ending the background application. Click
the tray icon to show it again. The tray menu also keeps direct capture actions
available.

## Global shortcuts

Blink uses the freedesktop Global Shortcuts portal. On the first run, press
**Configure global shortcuts** and GNOME will ask you to approve or change these
suggested bindings:

| Action | Suggested binding |
| --- | --- |
| Capture Area | `Print` |
| Capture Screen | `Shift+Print` |
| Capture Window | `Alt+Print` |

The portal owns the binding dialog and resolves conflicts, so Blink does not
silently steal GNOME shortcuts. Use **Configure global shortcuts** in Blink to
change the accepted bindings later. The shortcuts shown on Blink's capture cards
are updated from the portal.

The application must remain running for its global shortcuts to be active. Older
desktops without the Global Shortcuts portal can still use the window, tray menu,
and optional CLI interface.

## Wayland and X11 behavior

Blink uses the freedesktop Screenshot portal through `ashpd`. It never calls
`gnome-screenshot`, `scrot`, ImageMagick, Flameshot, or `xdg-open`.

- **Wayland:** GNOME/the compositor performs protected area and window selection.
  Blink does not bypass Wayland security.
- **X11:** This version uses the same portal backend for consistent behavior. The
  capture abstraction allows a dedicated native X11 backend later.
- **Portal versions:** Screenshot portal v3 can constrain selection to area,
  screen, or window. Older backends may show a general chooser for interactive
  captures.
- **Multiple monitors:** The compositor currently decides which monitor or desktop
  the screen action captures.

## Ubuntu requirements

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev \
  xdg-desktop-portal xdg-desktop-portal-gnome \
  gnome-shell-extension-appindicator
```

The application does not directly link to Xlib, Wayland client libraries, or
`libdbus`; D-Bus integration is implemented in Rust. AppIndicator support is only
needed for the tray icon. Install a current Rust toolchain with
[rustup](https://rustup.rs/) if needed; Blink uses Rust edition 2024.

## Build and run

```bash
cargo build
cargo run
```

For an optimized user installation:

```bash
cargo install --path .
blink
```

The CLI routes remain available for scripting and diagnostics, but are not the
primary interface:

```bash
blink area
blink screen
blink window
```

## Desktop launcher and optional autostart

After `cargo install --path .`:

```bash
mkdir -p ~/.local/share/applications
cp data/dev.blink.Blink.desktop ~/.local/share/applications/
```

If `blink` is not on the graphical session's `PATH`, replace `Exec=blink` in the
copied launcher with the absolute path printed by `command -v blink`.

Autostart remains opt-in:

```bash
mkdir -p ~/.config/autostart
cp data/dev.blink.Blink.desktop ~/.config/autostart/
```

## Development checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test
```

## Manual test checklist

From a graphical terminal in the Ubuntu desktop session:

1. Run `cargo run` and approve or customize the three global shortcuts.
2. Click each capture card and verify Blink hides before the portal UI appears.
3. Verify each PNG and notification under `~/Pictures/Blink`.
4. Trigger all three actions with the accepted shortcuts while another app is
   focused.
5. Open **Configure global shortcuts**, change a binding, and verify the card label
   updates.
6. Close the Blink window, restore it from the tray, and test the tray actions.
7. Cancel a capture chooser and verify no file or crash occurs.

## Known limitations

- Global shortcut availability and configuration UI depend on the desktop portal
  backend. Current GNOME and KDE portal backends support it; older releases may
  not.
- Exact screenshot target filtering requires Screenshot portal v3.
- Multi-monitor selection and a dedicated native X11 backend are not implemented.
- Tray visibility depends on a StatusNotifierItem/AppIndicator host.
- Blink does not yet prevent multiple simultaneously running instances.

## Next step

The next smallest improvement is single-instance application activation. A second
Blink launch should focus the existing window, and CLI capture requests should be
forwarded to that process rather than starting another instance.
