# mako-rs
 
A lightweight Wayland notification daemon written in Rust.
 
## Installation
 
### NixOS / Home Manager
 
Add `mako-rs` to your `flake.nix` inputs:
 
```nix
inputs.mako-rs.url = "github:ar175-lol/mako-rs";
```
 
Then import the Home Manager module and enable it:
 
```nix
{
  imports = [ inputs.mako-rs.homeModules.default ];
 
  programs.mako-rs = {
    enable = true;
    # any options below are optional — shown with their defaults
  };
}
```
 
Enabling `programs.mako-rs` installs the binary and registers a
`systemd --user` service (`mako-rs.service`) tied to
`graphical-session.target`, so it starts automatically with your
graphical session and restarts on failure. Check it with:
 
```bash
systemctl --user status mako-rs.service
journalctl --user -u mako-rs.service -f
```
 
If the service fails to start, the most common cause is
`WAYLAND_DISPLAY` / the D-Bus session address not being available in
the systemd user environment yet — make sure your compositor config
imports them (e.g. via `dbus-update-activation-environment --systemd`
on startup) before `graphical-session.target` is reached.
 
#### Options
 
All options live under `programs.mako-rs`:
 
| Option            | Type          | Default              | Description                                      |
|-------------------|---------------|-----------------------|---------------------------------------------------|
| `enable`          | bool          | `false`               | Enable the mako-rs service and package.            |
| `fontFamily`      | string        | `"JetBrains Mono"`    | Font used for both summary and body text.          |
| `fontSize`        | float         | `12.0`                | Font size for the notification summary (title).    |
| `bodyFontSize`    | float         | `14.0`                | Font size for the notification body.               |
| `bgColor`         | list of float | `[0.12 0.12 0.18]`    | Background color as `[r g b]`, each `0.0`–`1.0`.   |
| `textColor`       | list of float | `[0.8 0.84 0.96]`     | Text color as `[r g b]`, each `0.0`–`1.0`.         |
| `borderColor`     | list of float | `[0.54 0.71 0.98]`    | Border color as `[r g b]`, each `0.0`–`1.0`.       |
| `borderSize`      | float         | `2.0`                 | Border width in pixels.                            |
| `padding`         | float         | `15.0`                | Inner padding in pixels.                           |
| `width`           | int           | `360`                 | Notification width in pixels.                      |
| `minHeight`       | int           | `80`                  | Minimum notification height in pixels.             |
| `maxBufferHeight` | int           | `1024`                | Maximum notification height in pixels.             |
| `maxVisible`      | int           | `5`                   | Maximum number of notifications stacked at once.   |
| `gap`             | int           | `5`                   | Vertical gap between stacked notifications, in pixels. |
| `topMargin`       | int           | `20`                  | Margin from the top of the screen, in pixels.      |
| `rightMargin`     | int           | `20`                  | Margin from the right of the screen, in pixels.    |
| `timeoutSecs`     | int           | `5`                   | Default timeout (seconds) for low/normal urgency notifications with no explicit expiry. |
| `sweepMs`         | int           | `100`                 | How often (milliseconds) expired notifications are checked and cleared. |
 
Example with overrides:
 
```nix
programs.mako-rs = {
  enable = true;
  fontFamily = "Fira Code";
  width = 420;
  maxVisible = 3;
  bgColor = [ 0.05 0.05 0.10 ];
  borderColor = [ 0.90 0.30 0.30 ];
};
```
 
### Other distributions
 
First, ensure you have `cargo` and the required development libraries
installed on your system.
 
#### 1. Install Dependencies
 
You need development packages for `wayland`, `cairo`, and
`libxkbcommon`.
 
#### 2. Build and Install
 
Clone the repository and compile the binary in release mode:
 
```bash
git clone https://github.com/ar175-lol/mako-rs.git
cd mako-rs
cargo build --release
```
 
The binary will be at `target/release/mako-rs`. It needs a running
D-Bus session and an active Wayland compositor to do anything -- it
registers itself as `org.freedesktop.Notifications` on the session
bus and draws layer-shell surfaces directly.
 
To start it automatically, register it as a `systemd --user` service
tied to your graphical session (see the Home Manager section above
for the equivalent unit configuration), or invoke it from your
compositor's startup config.

p.s yes written with ai but I'm to lazy to explain this
