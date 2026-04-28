# project-picker

A keyboard-driven project launcher for Hyprland/Wayland. Fuzzy-searches recent and pinned projects, opens them in a Ghostty terminal.

## Requirements

- Rust (stable)
- Ghostty terminal
- Hyprland compositor

## Build & Install

```sh
git clone https://github.com/RNAV2019/project-picker.git
cd project-picker
cargo build --release
sudo cp target/release/project-picker /usr/local/bin/
```

## Usage

The binary acts as both daemon and CLI client:

```sh
project-picker           # start the daemon (runs in foreground)
project-picker --toggle  # show/hide the picker (starts daemon automatically if not running)
```

`--toggle` is the only command you need day-to-day. If the daemon isn't running it starts it in the background, then sends the toggle.

## Hyprland Setup

**Key binding** — add to `hyprland.conf` or your keybinds config:

```ini
bind = SUPER, P, exec, project-picker --toggle
```

**Window rules** — add to your window rules config:

```ini
windowrule = match:class uk.co.ryannavsaria.project-picker, float on
windowrule = match:class uk.co.ryannavsaria.project-picker, center on
windowrule = match:class uk.co.ryannavsaria.project-picker, border_size 0
windowrule = match:class uk.co.ryannavsaria.project-picker, rounding 18
```

Adjust `border_size` and `rounding` to taste. If you want a subtle border:

```ini
windowrule = match:class uk.co.ryannavsaria.project-picker, border_size 2
windowrule = match:class uk.co.ryannavsaria.project-picker, rounding 18
windowrule = border_color rgb(3a3a3a) rgb(2e2e2e), match:class uk.co.ryannavsaria.project-picker
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` / `Tab` | Navigate list |
| `Enter` | Open selected project |
| `Alt+P` | Pin / unpin selected project |
| `Alt+Backspace` | Remove selected project from recents |
| `Esc` | Close (or exit Add mode) |

## Data

Projects are stored in `~/.config/project-picker/`:

| File | Contents |
|------|----------|
| `recents.json` | Recently opened projects (most recent first) |
| `pinned.json` | Pinned projects |
