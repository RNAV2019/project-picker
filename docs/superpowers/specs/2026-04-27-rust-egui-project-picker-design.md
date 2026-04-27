# Rust + egui Project Picker — Design Spec

**Date:** 2026-04-27  
**Status:** Draft

---

## Overview

Migrate the existing GTK4/Python project picker to Rust using egui rendered via wgpu on a Wayland layer-shell surface. The app runs as a persistent daemon, hidden by default, toggled via a Unix socket. It fuzzy-searches recent projects, opens a ghostty terminal at the selected path, and supports an add-project mode with glob path completion and automatic project icon detection.

---

## Architecture

Single Rust binary with two runtime modes selected by a CLI flag:

```
project-picker          # start daemon (no-op if already running)
project-picker --toggle # send toggle signal to running daemon
```

A Hyprland keybind calls `project-picker --toggle`. If no daemon is running when `--toggle` is called, the binary starts the daemon in the background and then sends the toggle signal, so a single keybind works even cold.

### Module layout

```
src/
  main.rs         # arg parsing; branch to daemon or client
  daemon.rs       # unix socket server + calloop event loop glue
  app.rs          # egui App struct: all UI state + per-frame update
  projects.rs     # load/save recents.json, fuzzy match
  paths.rs        # glob path completion for add-mode
  terminal.rs     # spawn ghostty via uwsm-app
  icons.rs        # icon resolution + in-memory cache
  ui/
    mod.rs
    search.rs     # search bar widget
    list.rs       # section headers, project/action/suggestion rows
    hints.rs      # bottom keyboard hints bar
    theme.rs      # all color, font, and spacing constants
```

---

## IPC / Daemon Model

```
┌─────────────────────────┐
│  Hyprland keybind       │
│  exec: project-picker   │
│         --toggle        │
└────────────┬────────────┘
             │ connect + send "toggle\n"
             ▼
  /tmp/project-picker.sock
             │
             ▼
┌─────────────────────────┐
│  Daemon (always running)│
│  - layer-shell window   │
│  - hidden by default    │
│  - on toggle: show/hide │
│  - on show: grab focus  │
└─────────────────────────┘
```

Socket protocol: newline-delimited plaintext. Initial command set: `toggle\n`. Extensible without breaking changes.

**On show:**
1. Set layer surface keyboard interactivity to `Exclusive` (ensures Hyprland grants keyboard focus immediately — `OnDemand` alone is insufficient for reliable focus grab as a launcher)
2. Call `wl_surface.commit()`
3. Make window visible
4. Clear search query, reset selected index, reset mode to Search
5. Focus search entry

**On hide:**
1. Set layer surface keyboard interactivity to `None`
2. Call `wl_surface.commit()`
3. Hide window

State preserved across hide/show cycles: recents list, icon cache.

**Note on compositor behavior:** Using `Exclusive` keyboard interactivity on show is required for reliable focus grab in Hyprland. This is compositor-specific behavior; on other compositors the behavior may differ.

---

## UI Components & Styling

### Theme constants (`ui/theme.rs`)

| Token | Value |
|---|---|
| `BG` | `#1c1c1c` |
| `ROW_HOVER` | `#252525` |
| `ROW_SELECTED` | `#2e2e2e` |
| `SECTION_HEADER` | `#6b6b6b` |
| `TEXT_PRIMARY` | `#e8e8e8` |
| `TEXT_MUTED` | `#6b6b6b` |
| `KBD_BG` | `#2e2e2e` |
| `KBD_TEXT` | `#c8c8c8` |
| `ACCENT` | `#5c8fff` |
| Window width | 680 px |
| Window max height | 480 px |

### Search bar (`ui/search.rs`)

- Full-width, no visible border, background matches window
- Magnifying glass SVG icon left-aligned
- Placeholder: `"Search projects..."` (switches to `"Type directory path..."` in add-mode)
- Cursor color: `ACCENT`

### List rows (`ui/list.rs`)

**Section header row** — non-interactive:
```
Actions                    (small, uppercase, TEXT_MUTED)
```

**Action row** (e.g. Add project):
```
[ icon 20px ]  Label                              ›
```

**Project row**:
```
[ icon 20px ]  Project name  (TEXT_PRIMARY, 14px bold)     timestamp (TEXT_MUTED)
               ~/path/to/project  (TEXT_MUTED, 12px)
```

**Suggestion row** (add-mode path completion):
```
[ folder icon ]  ~/path/suggestion
```

Row height: 48 px for project rows, 40 px for action/suggestion rows, 28 px for section headers.  
Hover: background → `ROW_HOVER`. Selected: background → `ROW_SELECTED`.

### Hints bar (`ui/hints.rs`)

```
[ ↑ ] [ ↓ ]  Navigate    [ Enter ]  Select    [ Alt+⌫ ]  Remove    [ Esc ]  Close
```

Kbd badges: rounded rect, `KBD_BG` fill, `KBD_TEXT` label, 4 px corner radius.

---

## Icon System (`icons.rs`)

### Resolution order (per project path)

1. **Image file scan** — check project root for (in order):
   `icon.png`, `logo.png`, `logo.svg`, `favicon.ico`, `.github/logo.png`, `.github/LOGO.png`
2. **Stack detection** — check for marker files and map to bundled SVG:

| Marker file | Icon |
|---|---|
| `Cargo.toml` | Rust |
| `package.json` | JavaScript |
| `tsconfig.json` | TypeScript |
| `go.mod` | Go |
| `requirements.txt` / `pyproject.toml` / `*.py` | Python |
| `Gemfile` | Ruby |
| `pom.xml` / `build.gradle` | Java |
| `CMakeLists.txt` | C/C++ |

3. **Fallback** — generic folder SVG

### Bundled SVG rasterization

Bundled SVGs are rasterized at startup using `resvg` 0.42+, which requires `usvg` and `tiny-skia` as companion crates (resvg 0.42 removed its standalone API):

1. Parse SVG: `usvg::Tree::from_data(&svg_bytes, &usvg::Options::default())`
2. Allocate pixel buffer: `tiny_skia::Pixmap::new(width, height)`
3. Render: `resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut())`
4. Convert to egui: `egui::ColorImage::from_rgba_unmultiplied([width, height], pixmap.data())`

Bundled SVGs are rasterized once at daemon startup to `egui::TextureHandle` and stored in the icon cache.

### Image file loading

PNG/JPG files are decoded with the `image` crate and converted to RGBA8 bytes, then to `egui::ColorImage::from_rgba_unmultiplied`.

For ICO files (multi-frame format): use the `image` crate's ICO decoder, iterate all available frames, and select the frame with dimensions closest to 20×20 (the display size). Fall back to the first frame if only one is present.

The actual `TextureHandle` allocation happens on the main thread (egui requirement).

### Caching

`IconCache` is a `HashMap<String, egui::TextureHandle>` keyed by project path. Populated lazily on first render of each project row.

Icon scanning for new projects runs on a background thread via `std::thread::spawn`. The background thread sends `(project_path: String, rgba_bytes: Vec<u8>, width: u32, height: u32)` over a `std::sync::mpsc` channel. The main thread checks the channel receiver each frame and allocates `TextureHandle` from the received bytes.

---

## Data Layer

### `projects.rs`

- `load_recents() -> Vec<String>` — reads `~/.config/project-picker/recents.json`; returns empty vec on missing/malformed file. Compatible with existing Python-written files.
- `save_recents(recents: &[String])` — atomic write: serialize to temp file in same directory, then `rename`. Prevents corruption on crash.
- `fuzzy_match(query: &str, text: &str) -> bool` — character subsequence match, case-insensitive. Same algorithm as Python original.

### `paths.rs`

- `get_suggestions(typed: &str) -> Vec<String>` — expands `~`, globs `typed*`, filters to directories only, tilde-collapses results, returns max 20.

### `terminal.rs`

- `open_terminal(path: &str)` — calls `uwsm-app -- ghostty --working-directory=<abs_path>` via `std::process::Command::spawn`. Non-blocking.

---

## Rendering Stack

```
egui  (immediate-mode UI)
  └─ egui-wgpu  (wgpu render backend)
       └─ wgpu  (GPU rendering, Vulkan backend on Wayland)
            └─ raw-window-handle 0.6  (wl_surface bridge)
                 └─ zwlr-layer-shell-v1 protocol
                      └─ smithay-client-toolkit (SCK)
                           └─ calloop (event loop)
                                └─ calloop-wayland-source
```

**Crate version note:** egui, egui-wgpu, and wgpu must use matching versions — egui-wgpu re-exports wgpu types. Verify exact compatible versions from `egui-wgpu/Cargo.toml` on crates.io at build time. As a starting point, use the latest stable egui release and the wgpu version it specifies. Similarly, `calloop-wayland-source` must match the calloop major version; verify on crates.io.

Key crates:
```toml
[dependencies]
egui = "0.29"                     # verify wgpu version it requires
egui-wgpu = "0.29"
wgpu = "0.20"                     # egui 0.29 requires wgpu 0.20 — re-verify if egui version changes
raw-window-handle = "0.6"
smithay-client-toolkit = "0.18"   # verify latest on crates.io
wayland-protocols = "0.31"        # for zwlr-layer-shell-v1
calloop = "0.12"
calloop-wayland-source = "0.3"    # must match calloop major version
serde = { version = "1", features = ["derive"] }
serde_json = "1"
glob = "0.3"
image = { version = "0.25", default-features = false, features = ["png", "ico", "jpeg"] }
resvg = "0.42"       # requires usvg + tiny-skia companion crates
usvg = "0.42"
tiny-skia = "0.11"
```

### Wayland surface creation bridge

SCK creates a `wl_surface` as a layer-shell surface. To attach wgpu rendering to it, a `raw-window-handle` bridge is needed since SCK does not implement `HasWindowHandle`/`HasDisplayHandle` directly:

1. Implement a local `WaylandWindowHandle` wrapper struct holding the raw `wl_surface` pointer and `wl_display` pointer.
2. Implement `HasWindowHandle` (returns `RawWindowHandle::Wayland`) and `HasDisplayHandle` (returns `RawDisplayHandle::Wayland`) for the wrapper.
3. Call `wgpu::Instance::create_surface(unsafe { ... })` with the wrapper to obtain a `wgpu::Surface`.
4. Configure the surface with the desired format, present mode, and dimensions.

### Layer-shell window configuration

| Property | Value |
|---|---|
| Protocol | `zwlr-layer-shell-v1` |
| Layer | `Overlay` |
| Anchor | `Top` only (no Left/Right anchor) |
| Surface size | `set_size(680, 0)` — explicit width, height=0 for auto |
| Exclusive zone | `-1` (does not push tiling windows) |
| Keyboard interactivity | `Exclusive` when visible, `None` when hidden |

Setting only `Top` anchor without left/right anchors centers the surface horizontally on the output. Explicit `set_size(680, 0)` is required — without it the compositor may render a 0×0 surface.

### Render loop

The render loop runs inside calloop's event loop. Frame timing is driven by `wl_surface.frame()` callbacks:

1. After each commit, register a `wl_callback` for the next frame via `wl_surface.frame()`.
2. When the frame callback fires (dispatched by calloop via `calloop-wayland-source`), call `egui::Context::run` to produce paint commands.
3. Encode and submit draw calls via `egui-wgpu`'s renderer.
4. Call `wgpu::Queue::submit(...)`.
5. Present the swap chain image.
6. Call `wl_surface.commit()`.
7. Register the next frame callback (repeat from step 1).

When the window is hidden, skip frame callback registration to avoid rendering while invisible.

---

## App State (`app.rs`)

```rust
enum Mode { Search, Add }

struct App {
    mode: Mode,
    query: String,
    recents: Vec<String>,
    filtered: Vec<String>,       // derived from recents + query each frame
    suggestions: Vec<String>,    // derived from query in Add mode
    selected_idx: Option<usize>,
    icon_cache: IconCache,
    icon_rx: Receiver<(String, Vec<u8>, u32, u32)>,  // path, rgba bytes, w, h
    visible: bool,
}
```

`filtered` and `suggestions` are recomputed every frame from `query` — no manual invalidation needed (immediate-mode pattern).

State reset on hide: `query` cleared, `selected_idx` reset to `None`, `mode` reset to `Search`. Recents list and icon cache persist for the daemon lifetime.

---

## Keyboard Navigation

| Key | Action |
|---|---|
| `↓` / `Tab` | Move selection down (wrap from entry → first row) |
| `↑` / `Shift+Tab` | Move selection up (wrap from first row → entry) |
| `Enter` | Activate selected row (or first selectable if none) |
| `Alt+Backspace` | Remove selected project from recents (Search mode) |
| `Escape` | Exit Add mode → Search mode; or hide window |

---

## Out of Scope

- Multi-monitor awareness (window appears on focused monitor via layer-shell default)
- Settings UI (open settings action row present but no-ops initially)
- Persistent icon disk cache
- Project sorting beyond recency order
