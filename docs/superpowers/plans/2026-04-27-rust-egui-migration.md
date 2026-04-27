# Rust + egui Project Picker — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the GTK4/Python project picker with a Rust daemon that renders via egui/wgpu on a Wayland layer-shell surface, toggled by a Unix socket.

**Architecture:** Single binary with `--toggle` flag for client mode. The daemon uses smithay-client-toolkit to manage a `zwlr-layer-shell-v1` surface, renders egui via a hand-rolled wgpu render loop (no eframe/winit), and exposes `/tmp/project-picker.sock` for IPC. All UI state lives in `App`; calloop drives the event loop.

**Tech Stack:** Rust, egui 0.29, egui-wgpu 0.29, wgpu 0.20, smithay-client-toolkit 0.18, calloop 0.12, wayland-client 0.31, resvg/usvg/tiny-skia for SVG icons, image crate for raster icons.

**Spec:** `docs/superpowers/specs/2026-04-27-rust-egui-project-picker-design.md`

---

## File Map

| File | Responsibility |
|---|---|
| `src/main.rs` | Arg parsing; daemon vs client branch; cold-start logic |
| `src/daemon.rs` | Wayland state, calloop event loop, render loop, Unix socket server |
| `src/app.rs` | `App` struct; all UI state, mode, selection, hide/show logic |
| `src/projects.rs` | `load_recents`, `save_recents`, `fuzzy_match` |
| `src/paths.rs` | `get_suggestions` (glob path completion) |
| `src/terminal.rs` | `open_terminal` (spawn ghostty via uwsm-app) |
| `src/icons.rs` | `IconResolver`, `IconCache`, background thread, rasterization |
| `src/assets.rs` | Bundled SVG bytes via `include_bytes!` |
| `src/ui/mod.rs` | Re-exports |
| `src/ui/theme.rs` | All color/spacing constants |
| `src/ui/search.rs` | Search bar widget |
| `src/ui/list.rs` | Section headers, project/action/suggestion rows |
| `src/ui/hints.rs` | Bottom keyboard hints bar |
| `src/assets/` | SVG files for each language icon + folder fallback |

---

## Chunk 1: Project Scaffold & Data Layer

### Task 1: Initialize Cargo project

**Files:**
- Delete: `main.py`, `style.css`
- Create: `Cargo.toml`, `src/main.rs`, all empty module stubs

- [ ] **Step 1: Remove Python files**

```bash
cd /home/ryan/projects/project-picker
rm main.py style.css
```

- [ ] **Step 2: Initialize Cargo project**

```bash
cargo init --name project-picker
```

- [ ] **Step 3: Write `Cargo.toml`**

```toml
[package]
name = "project-picker"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "project-picker"
path = "src/main.rs"

[dependencies]
# egui rendering stack
egui = "0.29"
egui-wgpu = "0.29"
wgpu = { version = "0.20", features = [] }
raw-window-handle = "0.6"

# Wayland
wayland-client = "0.31"
wayland-protocols-wlr = { version = "0.2", features = ["client"] }
smithay-client-toolkit = { version = "0.18", features = ["calloop"] }
calloop = "0.12"
calloop-wayland-source = "0.3"

# Data
serde = { version = "1", features = ["derive"] }
serde_json = "1"
glob = "0.3"

# Icons
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
ico = "0.3"          # ICO frame iteration; selects frame closest to target size
resvg = "0.42"
usvg = "0.42"
tiny-skia = "0.11"
pollster = "0.3"     # block_on for async wgpu adapter/device init

[dev-dependencies]
tempfile = "3"
```

**Note:** After writing `Cargo.toml`, run `cargo fetch` to resolve. If egui-wgpu 0.29 requires a different wgpu version, adjust `wgpu` to match exactly (check `cargo tree | grep wgpu` after fetch).

- [ ] **Step 4: Create empty module stubs**

Create these files, each containing only `// TODO`:
- `src/projects.rs`
- `src/paths.rs`
- `src/terminal.rs`
- `src/icons.rs`
- `src/assets.rs`
- `src/app.rs`
- `src/daemon.rs`
- `src/ui/mod.rs`
- `src/ui/theme.rs`
- `src/ui/search.rs`
- `src/ui/list.rs`
- `src/ui/hints.rs`

- [ ] **Step 5: Write minimal `src/main.rs`**

```rust
mod app;
mod assets;
mod daemon;
mod icons;
mod paths;
mod projects;
mod terminal;
mod ui;

fn main() {
    println!("project-picker");
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cargo check
```

Expected: compiles with warnings about unused modules.

- [ ] **Step 7: Commit**

```bash
git init
git add -A
git commit -m "chore: initialize Rust project scaffold"
```

---

### Task 2: `projects.rs` — `fuzzy_match`

**Files:**
- Modify: `src/projects.rs`

- [ ] **Step 1: Write failing tests**

In `src/projects.rs`:

```rust
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query_matches_everything() {
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn test_exact_match() {
        assert!(fuzzy_match("foo", "foo"));
    }

    #[test]
    fn test_subsequence_match() {
        assert!(fuzzy_match("pck", "project-picker"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(fuzzy_match("PCK", "project-picker"));
    }

    #[test]
    fn test_no_match() {
        assert!(!fuzzy_match("xyz", "project-picker"));
    }

    #[test]
    fn test_partial_match_fails() {
        assert!(!fuzzy_match("abc", "ab"));
    }
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test projects::tests
```

Expected: compile error or `todo!()` panic.

- [ ] **Step 3: Implement `fuzzy_match`**

```rust
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    let query = query.to_lowercase();
    let text = text.to_lowercase();
    let mut query_chars = query.chars();
    let mut current = match query_chars.next() {
        None => return true,
        Some(c) => c,
    };
    for ch in text.chars() {
        if ch == current {
            match query_chars.next() {
                None => return true,
                Some(c) => current = c,
            }
        }
    }
    false
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test projects::tests
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/projects.rs
git commit -m "feat: implement fuzzy_match with tests"
```

---

### Task 3: `projects.rs` — `load_recents` and `save_recents`

**Files:**
- Modify: `src/projects.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/projects.rs`:

```rust
use std::path::{Path, PathBuf};

pub fn recents_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/project-picker/recents.json")
}

pub fn load_recents() -> Vec<String> {
    todo!()
}

pub fn save_recents(recents: &[String]) {
    todo!()
}

// in #[cfg(test)] mod tests:

    #[test]
    fn test_load_missing_file_returns_empty() {
        let result = load_recents_from(Path::new("/nonexistent/path/recents.json"));
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        let paths = vec!["~/projects/foo".to_string(), "~/projects/bar".to_string()];
        save_recents_to(&path, &paths);
        let loaded = load_recents_from(&path);
        assert_eq!(loaded, paths);
    }

    #[test]
    fn test_load_malformed_json_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        std::fs::write(&path, b"not json").unwrap();
        let result = load_recents_from(&path);
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_save_is_atomic() {
        // save_recents_to must write to a temp file then rename
        // verify the file exists after save
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        save_recents_to(&path, &["~/foo".to_string()]);
        assert!(path.exists());
    }
```

- [ ] **Step 2: Implement the path-parameterized helpers first**

```rust
pub fn load_recents_from(path: &Path) -> Vec<String> {
    let Ok(data) = std::fs::read(path) else { return vec![] };
    let Ok(parsed) = serde_json::from_slice::<Vec<serde_json::Value>>(&data) else { return vec![] };
    parsed.into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

pub fn save_recents_to(path: &Path, recents: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(recents).unwrap();
    std::fs::write(&tmp, &data).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

pub fn load_recents() -> Vec<String> {
    load_recents_from(&recents_path())
}

pub fn save_recents(recents: &[String]) {
    save_recents_to(&recents_path(), recents);
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test projects::tests
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/projects.rs
git commit -m "feat: implement load_recents and save_recents with atomic write"
```

---

### Task 4: `paths.rs` — `get_suggestions`

**Files:**
- Modify: `src/paths.rs`

- [ ] **Step 1: Write failing tests**

```rust
pub fn get_suggestions(typed: &str) -> Vec<String> {
    todo!()
}

pub fn tilde_collapse(path: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input_returns_empty() {
        assert_eq!(get_suggestions(""), vec![] as Vec<String>);
    }

    #[test]
    fn test_tilde_collapse() {
        let home = std::env::var("HOME").unwrap();
        let full = format!("{}/projects", home);
        assert_eq!(tilde_collapse(&full), "~/projects");
    }

    #[test]
    fn test_suggestions_from_real_tmp() {
        // /tmp always exists and has subdirs we can test against
        let results = get_suggestions("/tmp/");
        // Should be vec of strings starting with ~/  or /
        for r in &results {
            assert!(r.starts_with('/') || r.starts_with('~'));
        }
        assert!(results.len() <= 20);
    }
}
```

- [ ] **Step 2: Implement**

```rust
pub fn tilde_collapse(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}{}", home, &path[1..])
    } else if path == "~" {
        std::env::var("HOME").unwrap_or_default()
    } else {
        path.to_string()
    }
}

pub fn get_suggestions(typed: &str) -> Vec<String> {
    if typed.is_empty() {
        return vec![];
    }
    let expanded = expand_tilde(typed);
    // If user typed a trailing slash, glob inside that dir; otherwise prefix-match siblings
    let pattern = if typed.ends_with('/') {
        format!("{}*", expanded)
    } else {
        format!("{}*/", expanded)  // trailing slash ensures only dirs match
    };
    let mut matches: Vec<String> = glob::glob(&pattern)
        .unwrap_or_else(|_| glob::glob("/dev/null").unwrap())
        .filter_map(|entry| entry.ok())
        .filter(|p| p.is_dir())
        .map(|p| tilde_collapse(p.to_string_lossy().as_ref()))
        .take(20)
        .collect();
    matches.sort();
    matches
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test paths::tests
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/paths.rs
git commit -m "feat: implement path completion with tilde expansion"
```

---

### Task 5: `terminal.rs`

**Files:**
- Modify: `src/terminal.rs`

- [ ] **Step 1: Implement `open_terminal`**

```rust
use std::process::Command;

pub fn open_terminal(path: &str) {
    let abs = expand_tilde(path);
    let _ = Command::new("uwsm-app")
        .args(["--", "ghostty", &format!("--working-directory={}", abs)])
        .spawn();
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}{}", home, &path[1..])
    } else {
        path.to_string()
    }
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/terminal.rs
git commit -m "feat: implement open_terminal via uwsm-app + ghostty"
```

---

## Chunk 2: Icon System

### Task 6: Bundle SVG assets

**Files:**
- Create: `src/assets/` directory with SVG files
- Modify: `src/assets.rs`

- [ ] **Step 1: Create asset directory and download/write SVG icons**

Create `src/assets/` and add these SVG files. Use simple, minimal SVGs (colored shapes with letters) if you don't have logo SVGs available. The important thing is they must be valid SVGs at ≤1KB each.

Required files:
- `src/assets/rust.svg`
- `src/assets/javascript.svg`
- `src/assets/typescript.svg`
- `src/assets/python.svg`
- `src/assets/go.svg`
- `src/assets/ruby.svg`
- `src/assets/java.svg`
- `src/assets/cpp.svg`
- `src/assets/folder.svg`

Example minimal SVG (substitute icon-specific colors/letters):
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
  <circle cx="12" cy="12" r="11" fill="#DEA584"/>
  <text x="12" y="16" font-family="monospace" font-size="11" text-anchor="middle" fill="white">Rs</text>
</svg>
```

For production quality, source icons from Simple Icons (https://simpleicons.org) — download the SVGs for Rust, JavaScript, TypeScript, Python, Go, Ruby, Java, C++, and a generic folder.

- [ ] **Step 2: Write `src/assets.rs`**

```rust
pub const RUST_SVG: &[u8] = include_bytes!("assets/rust.svg");
pub const JAVASCRIPT_SVG: &[u8] = include_bytes!("assets/javascript.svg");
pub const TYPESCRIPT_SVG: &[u8] = include_bytes!("assets/typescript.svg");
pub const PYTHON_SVG: &[u8] = include_bytes!("assets/python.svg");
pub const GO_SVG: &[u8] = include_bytes!("assets/go.svg");
pub const RUBY_SVG: &[u8] = include_bytes!("assets/ruby.svg");
pub const JAVA_SVG: &[u8] = include_bytes!("assets/java.svg");
pub const CPP_SVG: &[u8] = include_bytes!("assets/cpp.svg");
pub const FOLDER_SVG: &[u8] = include_bytes!("assets/folder.svg");
```

- [ ] **Step 3: Verify compile**

```bash
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src/assets/ src/assets.rs
git commit -m "feat: add bundled SVG icon assets"
```

---

### Task 7: `icons.rs` — stack detection

**Files:**
- Modify: `src/icons.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum IconKind {
    BundledSvg(&'static [u8]),
    ImageFile(std::path::PathBuf),
    Folder,
}

pub fn detect_icon_kind(project_path: &str) -> IconKind {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detects_rust_project() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), b"[package]").unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::BundledSvg(crate::assets::RUST_SVG));
    }

    #[test]
    fn test_detects_node_project() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), b"{}").unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::BundledSvg(crate::assets::JAVASCRIPT_SVG));
    }

    #[test]
    fn test_image_file_takes_priority() {
        let dir = tempdir().unwrap();
        // Write a minimal 1x1 PNG
        fs::write(dir.path().join("Cargo.toml"), b"[package]").unwrap();
        let png_path = dir.path().join("icon.png");
        fs::write(&png_path, include_bytes!("../tests/fixtures/1x1.png")).unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::ImageFile(png_path));
    }

    #[test]
    fn test_fallback_to_folder() {
        let dir = tempdir().unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::Folder);
    }
}
```

- [ ] **Step 2: Create test fixture**

Run these commands exactly to generate `tests/fixtures/1x1.png` (a valid 1×1 RGB PNG):

```bash
mkdir -p /home/ryan/projects/project-picker/tests/fixtures
python3 - <<'EOF'
import struct, zlib

sig = b'\x89PNG\r\n\x1a\n'

def chunk(tag, data):
    crc = zlib.crc32(tag + data) & 0xffffffff
    return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', crc)

ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 2, 0, 0, 0))  # 1x1 RGB8
# Scanline: filter byte 0 + R G B = \x00\xff\xff\xff
idat = chunk(b'IDAT', zlib.compress(b'\x00\xff\xff\xff'))
iend = chunk(b'IEND', b'')

with open('tests/fixtures/1x1.png', 'wb') as f:
    f.write(sig + ihdr + idat + iend)
print("Written tests/fixtures/1x1.png")
EOF
```

Verify it was created:
```bash
file tests/fixtures/1x1.png
```
Expected: `PNG image data, 1 x 1, 8-bit/color RGB, non-interlaced`

- [ ] **Step 3: Implement `detect_icon_kind`**

```rust
use std::path::Path;
use crate::assets;

pub fn detect_icon_kind(project_path: &str) -> IconKind {
    let root = Path::new(project_path);

    // 1. Check for image files first
    let image_candidates = [
        "icon.png", "logo.png", "logo.svg", "favicon.ico",
        ".github/logo.png", ".github/LOGO.png",
    ];
    for name in &image_candidates {
        let candidate = root.join(name);
        if candidate.exists() {
            return IconKind::ImageFile(candidate);
        }
    }

    // 2. Stack detection via marker files
    let markers: &[(&str, &[u8])] = &[
        ("Cargo.toml",       assets::RUST_SVG),
        ("tsconfig.json",    assets::TYPESCRIPT_SVG),
        ("package.json",     assets::JAVASCRIPT_SVG),
        ("go.mod",           assets::GO_SVG),
        ("pyproject.toml",   assets::PYTHON_SVG),
        ("requirements.txt", assets::PYTHON_SVG),
        ("Gemfile",          assets::RUBY_SVG),
        ("pom.xml",          assets::JAVA_SVG),
        ("build.gradle",     assets::JAVA_SVG),
        ("CMakeLists.txt",   assets::CPP_SVG),
    ];
    for (marker, svg) in markers {
        if root.join(marker).exists() {
            return IconKind::BundledSvg(svg);
        }
    }
    // Check for .py files (glob would be overkill; check extension on dir entries)
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("py") {
                return IconKind::BundledSvg(assets::PYTHON_SVG);
            }
        }
    }

    // 3. Fallback
    IconKind::Folder
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test icons::tests
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/icons.rs tests/
git commit -m "feat: implement icon stack detection with tests"
```

---

### Task 8: `icons.rs` — SVG rasterization

**Files:**
- Modify: `src/icons.rs`

- [ ] **Step 1: Write test for SVG rasterization (no egui dependency)**

```rust
pub fn rasterize_svg(svg_bytes: &[u8], size: u32) -> Option<(Vec<u8>, u32, u32)> {
    todo!()
}

// in tests:
    #[test]
    fn test_rasterize_bundled_svg() {
        let (bytes, w, h) = rasterize_svg(assets::FOLDER_SVG, 20).unwrap();
        assert_eq!(w, 20);
        assert_eq!(h, 20);
        assert_eq!(bytes.len() as u32, w * h * 4); // RGBA
    }
```

- [ ] **Step 2: Implement `rasterize_svg`**

```rust
pub fn rasterize_svg(svg_bytes: &[u8], size: u32) -> Option<(Vec<u8>, u32, u32)> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some((pixmap.data().to_vec(), size, size))
}
```

- [ ] **Step 3: Run test**

```bash
cargo test icons::tests::test_rasterize_bundled_svg
```

- [ ] **Step 4: Commit**

```bash
git add src/icons.rs
git commit -m "feat: implement SVG rasterization via usvg/tiny-skia"
```

---

### Task 9: `icons.rs` — image loading + `IconCache` + background thread

**Files:**
- Modify: `src/icons.rs`

- [ ] **Step 1: Implement image file loading**

```rust
pub fn load_image_file(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if ext == "svg" {
        let bytes = std::fs::read(path).ok()?;
        return rasterize_svg(&bytes, 20);
    }
    if ext == "ico" {
        return load_ico_closest_to(path, 20);
    }
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

fn load_ico_closest_to(path: &std::path::Path, target: u32) -> Option<(Vec<u8>, u32, u32)> {
    // Use the `ico` crate to iterate all frames and pick the one closest to `target` px.
    let file = std::fs::File::open(path).ok()?;
    let dir = ico::IconDir::read(std::io::BufReader::new(file)).ok()?;
    let entry = dir.entries().iter()
        .min_by_key(|e| (e.width() as i32 - target as i32).unsigned_abs())?;
    let image = entry.decode().ok()?;
    let w = image.width();
    let h = image.height();
    // ico::IconImage stores RGBA pixels directly
    Some((image.rgba_data().to_vec(), w, h))
}
```

- [ ] **Step 2: Define `IconCache` and the channel message type**

```rust
use std::sync::mpsc::{self, Receiver, Sender};
use std::collections::HashMap;

pub struct IconLoadResult {
    pub project_path: String,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// IconCache maps project_path → index into a slab of loaded textures.
// The actual TextureHandle lives in app.rs since it requires an egui context.
// icons.rs just produces the raw RGBA pixels; app.rs converts them.
pub struct IconResolver {
    /// Set of paths already dispatched to background thread (avoids duplicate scans)
    pub pending: std::collections::HashSet<String>,
    pub tx: Sender<IconLoadResult>,
    pub rx: Receiver<IconLoadResult>,
}

impl IconResolver {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { pending: Default::default(), tx, rx }
    }

    /// Kick off background resolution for a project path if not already pending/resolved.
    pub fn request(&mut self, project_path: &str) {
        if self.pending.contains(project_path) {
            return;
        }
        self.pending.insert(project_path.to_string());
        let path = project_path.to_string();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let rgba = resolve_to_rgba(&path);
            if let Some((rgba, width, height)) = rgba {
                let _ = tx.send(IconLoadResult { project_path: path, rgba, width, height });
            }
        });
    }
}

fn resolve_to_rgba(project_path: &str) -> Option<(Vec<u8>, u32, u32)> {
    match detect_icon_kind(project_path) {
        IconKind::ImageFile(p) => load_image_file(&p),
        IconKind::BundledSvg(svg) => rasterize_svg(svg, 20),
        IconKind::Folder => rasterize_svg(assets::FOLDER_SVG, 20),
    }
}
```

- [ ] **Step 3: Pre-rasterize bundled SVGs at startup**

```rust
/// Returns RGBA bytes for each bundled SVG at 20px, in the same order as `ALL_BUNDLED`.
/// Call once at daemon startup before the event loop.
pub fn rasterize_all_bundled() -> HashMap<*const u8, (Vec<u8>, u32, u32)> {
    let all: &[&[u8]] = &[
        assets::RUST_SVG, assets::JAVASCRIPT_SVG, assets::TYPESCRIPT_SVG,
        assets::PYTHON_SVG, assets::GO_SVG, assets::RUBY_SVG,
        assets::JAVA_SVG, assets::CPP_SVG, assets::FOLDER_SVG,
    ];
    all.iter()
        .filter_map(|svg| rasterize_svg(svg, 20).map(|r| (svg.as_ptr(), r)))
        .collect()
}
```

- [ ] **Step 4: Verify compile**

```bash
cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src/icons.rs
git commit -m "feat: implement IconResolver with background thread and image loading"
```

---

## Chunk 3: Wayland Foundation

### Task 10: `daemon.rs` — Wayland connection and globals

**Files:**
- Modify: `src/daemon.rs`

This chunk is the most technically complex part. Read it fully before starting.

- [ ] **Step 1: Add required SCK imports and define the `State` skeleton**

`src/daemon.rs`:

```rust
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers, keysyms},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{
        KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        LayerSurfaceConfigure,
    },
};
use wayland_client::protocol::wl_callback;
use std::os::unix::net::UnixListener;

pub struct State {
    // Wayland protocol state (SCK managed)
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,

    // Our layer surface
    layer_surface: LayerSurface,
    wl_surface: wl_surface::WlSurface,

    // wgpu rendering
    device: wgpu::Device,
    queue: wgpu::Queue,
    wgpu_surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    egui_renderer: egui_wgpu::Renderer,

    // egui
    egui_ctx: egui::Context,
    pending_input: egui::RawInput,
    current_modifiers: egui::Modifiers,
    pointer_pos: egui::Pos2,

    // App logic
    app: crate::app::App,

    // Control
    needs_redraw: bool,
    configured: bool,          // layer surface has been configured at least once
    pending_toggle: bool,      // set by socket handler; processed in idle callback
    loop_handle: LoopHandle<'static, Self>,
    qh: QueueHandle<State>,    // stored so idle callback can call show/hide
}
```

- [ ] **Step 2: Implement SCK trait boilerplate**

These are required by SCK's macro system. Add after the State definition:

```rust
impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _new_factor: i32) {}
    fn transform_changed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _new_transform: wl_output::Transform) {}
    fn frame(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _time: u32) {
        self.needs_redraw = true;
    }
    fn surface_enter(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _output: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface, _output: &wl_output::WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}
}

impl LayerShellHandler for State {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        std::process::exit(0);
    }
    fn configure(&mut self, _conn: &Connection, qh: &QueueHandle<Self>,
        layer: &LayerSurface, configure: LayerSurfaceConfigure, _serial: u32) {
        let (width, height) = configure.new_size;
        let width = if width == 0 { 680 } else { width };
        let height = if height == 0 { 40 } else { height }; // initial min height
        self.resize_surface(width, height, qh);
        self.configured = true;
        self.needs_redraw = true;
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState, SeatState];
}
```

- [ ] **Step 3: Implement keyboard handler**

```rust
impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _conn: &Connection, qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Keyboard && self.seat_state.get_keyboard(qh, &seat, None).is_ok() {}
        if capability == Capability::Pointer && self.seat_state.get_pointer(qh, &seat).is_ok() {}
    }
    fn remove_capability(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat, _capability: Capability) {}
    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}
}

impl KeyboardHandler for State {
    fn enter(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard, _surface: &wl_surface::WlSurface,
        _serial: u32, _raw: &[u32], _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}
    fn leave(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard, _surface: &wl_surface::WlSurface, _serial: u32) {}

    fn press_key(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard, _serial: u32, event: KeyEvent) {
        self.handle_key(event, true);
        self.needs_redraw = true;
    }
    fn release_key(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard, _serial: u32, event: KeyEvent) {
        self.handle_key(event, false);
    }
    fn update_modifiers(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard, _serial: u32, modifiers: Modifiers, _layout: u32) {
        self.current_modifiers = egui::Modifiers {
            alt: modifiers.alt,
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            mac_cmd: false,
            command: modifiers.ctrl,
        };
    }
}

impl State {
    fn handle_key(&mut self, event: KeyEvent, pressed: bool) {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        // Push text event for printable chars on press only
        if pressed {
            if let Some(text) = &event.utf8 {
                if !text.chars().all(|c| c.is_control()) {
                    self.pending_input.events.push(egui::Event::Text(text.clone()));
                }
            }
        }
        // Push key event for navigational/special keys
        if let Some(key) = keysym_to_egui(event.keysym) {
            self.pending_input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: event.repeat,
                modifiers: self.current_modifiers,
            });
        }
    }
}

fn keysym_to_egui(sym: smithay_client_toolkit::seat::keyboard::Keysym) -> Option<egui::Key> {
    use smithay_client_toolkit::seat::keyboard::keysyms;
    match sym.raw() {
        keysyms::KEY_Up        => Some(egui::Key::ArrowUp),
        keysyms::KEY_Down      => Some(egui::Key::ArrowDown),
        keysyms::KEY_Left      => Some(egui::Key::ArrowLeft),
        keysyms::KEY_Right     => Some(egui::Key::ArrowRight),
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => Some(egui::Key::Enter),
        keysyms::KEY_Escape    => Some(egui::Key::Escape),
        keysyms::KEY_BackSpace => Some(egui::Key::Backspace),
        keysyms::KEY_Tab       => Some(egui::Key::Tab),
        keysyms::KEY_ISO_Left_Tab => Some(egui::Key::Tab), // Shift+Tab arrives as ISO_Left_Tab
        _                      => None,
    }
}
```

- [ ] **Step 4: Implement pointer handler**

```rust
impl PointerHandler for State {
    fn pointer_frame(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for event in events {
            let pos = egui::pos2(event.position.0 as f32, event.position.1 as f32);
            match event.kind {
                PointerEventKind::Motion { .. } => {
                    self.pointer_pos = pos;
                    self.pending_input.events.push(egui::Event::PointerMoved(pos));
                    self.needs_redraw = true;
                }
                PointerEventKind::Press { button, .. } => {
                    let btn = wayland_button_to_egui(button);
                    self.pending_input.events.push(egui::Event::PointerButton {
                        pos, button: btn, pressed: true, modifiers: self.current_modifiers,
                    });
                    self.needs_redraw = true;
                }
                PointerEventKind::Release { button, .. } => {
                    let btn = wayland_button_to_egui(button);
                    self.pending_input.events.push(egui::Event::PointerButton {
                        pos, button: btn, pressed: false, modifiers: self.current_modifiers,
                    });
                }
                PointerEventKind::Leave { .. } => {
                    self.pending_input.events.push(egui::Event::PointerGone);
                }
                _ => {}
            }
        }
    }
}

fn wayland_button_to_egui(button: u32) -> egui::PointerButton {
    match button {
        0x110 => egui::PointerButton::Primary,   // BTN_LEFT
        0x111 => egui::PointerButton::Secondary, // BTN_RIGHT
        0x112 => egui::PointerButton::Middle,    // BTN_MIDDLE
        _     => egui::PointerButton::Primary,
    }
}
```

- [ ] **Step 5: Add SCK delegate macros at the bottom of `daemon.rs`**

SCK requires these macros for its trait dispatch system. Without them the crate will not compile.

```rust
smithay_client_toolkit::delegate_compositor!(State);
smithay_client_toolkit::delegate_output!(State);
smithay_client_toolkit::delegate_seat!(State);
smithay_client_toolkit::delegate_keyboard!(State);
smithay_client_toolkit::delegate_pointer!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_registry!(State);
```

- [ ] **Step 6: Verify compile**

```bash
cargo check
```

- [ ] **Step 7: Commit**

```bash
git add src/daemon.rs
git commit -m "feat: Wayland globals, SCK handlers, keyboard/pointer input, delegate macros"
```

---

### Task 11: `daemon.rs` — layer-shell surface + wgpu bridge

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Implement the wgpu surface bridge wrapper**

```rust
use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use std::ptr::NonNull;

struct WaylandSurfaceHandle {
    surface: *mut std::ffi::c_void,   // wl_surface raw pointer
    display: *mut std::ffi::c_void,   // wl_display raw pointer
}

unsafe impl Send for WaylandSurfaceHandle {}
unsafe impl Sync for WaylandSurfaceHandle {}

impl HasWindowHandle for WaylandSurfaceHandle {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let handle = WaylandWindowHandle::new(NonNull::new(self.surface).unwrap());
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle)) })
    }
}

impl HasDisplayHandle for WaylandSurfaceHandle {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let handle = WaylandDisplayHandle::new(NonNull::new(self.display).unwrap());
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(handle)) })
    }
}
```

- [ ] **Step 2: Implement `State::init`**

`init` takes an already-created `EventLoop` and `QueueHandle` (created in `run_daemon`) so there is exactly one event loop in the process.

```rust
impl State {
    /// Called from run_daemon after creating the EventLoop.
    /// Returns the fully initialized State; does NOT create a new EventLoop.
    pub fn init(
        loop_handle: LoopHandle<'static, Self>,
        qh: QueueHandle<Self>,
        conn: Connection,
        globals: smithay_client_toolkit::reexports::client::globals::GlobalList,
    ) -> Self {

        let compositor_state = CompositorState::bind(&globals, &qh)
            .expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, &qh)
            .expect("zwlr_layer_shell_v1 not available — is your compositor Hyprland/wlroots?");
        let seat_state = SeatState::new(&globals, &qh);
        let output_state = OutputState::new(&globals, &qh);
        let registry_state = RegistryState::new(&globals);

        // Create wl_surface and layer surface
        let wl_surface = compositor_state.create_surface(&qh);
        let layer_surface = layer_shell.create_layer_surface(
            &qh, wl_surface.clone(), Layer::Overlay, Some("project-picker"), None,
        );
        layer_surface.set_anchor(smithay_client_toolkit::shell::wlr_layer::Anchor::TOP);
        layer_surface.set_size(680, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        wl_surface.commit();

        // Build wgpu surface using the bridge
        let display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;
        let surface_ptr = wl_surface.id().as_ptr() as *mut std::ffi::c_void;
        let handle = WaylandSurfaceHandle { surface: surface_ptr, display: display_ptr };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let wgpu_surface = unsafe { instance.create_surface(&handle) }.expect("Failed to create wgpu surface");

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&wgpu_surface),
                ..Default::default()
            }).await.expect("No compatible GPU adapter");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None)
                .await.expect("Failed to get device");
            (adapter, device, queue)
        });

        let caps = wgpu_surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 680,
            height: 480,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        wgpu_surface.configure(&device, &surface_config);

        let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        let egui_ctx = egui::Context::default();

        // Apply dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(0x1c, 0x1c, 0x1c);
        egui_ctx.set_visuals(visuals);

        let app = crate::app::App::new();

        State {
            registry_state, seat_state, output_state, compositor_state, layer_shell,
            layer_surface, wl_surface,
            device, queue, wgpu_surface, surface_config, egui_renderer,
            egui_ctx,
            pending_input: egui::RawInput::default(),
            current_modifiers: egui::Modifiers::default(),
            pointer_pos: egui::Pos2::ZERO,
            app,
            needs_redraw: false,
            configured: false,
            pending_toggle: false,
            loop_handle,
            qh,
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32, _qh: &QueueHandle<Self>) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.wgpu_surface.configure(&self.device, &self.surface_config);
    }
}
```

Add `pollster` to Cargo.toml: `pollster = "0.3"`

- [ ] **Step 3: Verify compile**

```bash
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs Cargo.toml
git commit -m "feat: wgpu surface bridge + layer-shell surface init"
```

---

### Task 12: `daemon.rs` — render loop + show/hide

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Implement the per-frame render function**

```rust
impl State {
    pub fn render_frame(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured || !self.app.visible {
            return;
        }

        let surface_texture = match self.wgpu_surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                self.wgpu_surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(_) => return,
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let width = self.surface_config.width;
        let height = self.surface_config.height;

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: 1.0,
        };

        // Collect pending input, clear for next frame
        let mut raw_input = std::mem::take(&mut self.pending_input);
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        ));

        // Build and process egui frame
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            self.app.ui(ctx);
        });

        // Handle any app-level actions produced during the frame
        self.handle_app_output();

        // Encode and submit
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // Upload texture deltas
        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &primitives, &screen_descriptor);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0x1c as f64 / 255.0,
                            g: 0x1c as f64 / 255.0,
                            b: 0x1c as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.egui_renderer.render(&mut render_pass, &primitives, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        self.wl_surface.commit();

        // Request next frame callback (drives vsync)
        self.wl_surface.frame(qh, ());
        self.needs_redraw = false;
    }

    pub fn show(&mut self, qh: &QueueHandle<Self>) {
        self.layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.wl_surface.commit();
        self.app.on_show();
        self.wl_surface.frame(qh, ()); // kick off render loop
        self.needs_redraw = true;
    }

    pub fn hide(&mut self) {
        self.layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        self.wl_surface.commit();
        self.app.on_hide();
    }

    fn handle_app_output(&mut self) {
        let actions = self.app.drain_actions();
        for action in actions {
            match action {
                crate::app::AppAction::Hide => self.hide(),
                crate::app::AppAction::OpenTerminal(path) => crate::terminal::open_terminal(&path),
                crate::app::AppAction::RemoveProject(_) => {} // handled in-place by app
            }
        }
    }
}

// Dispatch wl_callback (frame timing)
impl smithay_client_toolkit::reexports::client::Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(state: &mut Self, _proxy: &wl_callback::WlCallback,
        event: wl_callback::Event, _data: &(), _conn: &Connection, _qh: &QueueHandle<Self>) {
        if let wl_callback::Event::Done { .. } = event {
            state.needs_redraw = true;
        }
    }
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/daemon.rs
git commit -m "feat: render loop, show/hide with keyboard interactivity toggle"
```

---

## Chunk 4: egui App & UI

### Task 13: `ui/theme.rs` — constants

**Files:**
- Modify: `src/ui/theme.rs`

- [ ] **Step 1: Write all theme constants**

```rust
use egui::Color32;

pub const BG:             Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);
pub const ROW_HOVER:      Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
pub const ROW_SELECTED:   Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const SECTION_HEADER: Color32 = Color32::from_rgb(0x6b, 0x6b, 0x6b);
pub const TEXT_PRIMARY:   Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
pub const TEXT_MUTED:     Color32 = Color32::from_rgb(0x6b, 0x6b, 0x6b);
pub const KBD_BG:         Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const KBD_TEXT:       Color32 = Color32::from_rgb(0xc8, 0xc8, 0xc8);
pub const ACCENT:         Color32 = Color32::from_rgb(0x5c, 0x8f, 0xff);
pub const SEPARATOR:      Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);

pub const WINDOW_WIDTH:   f32 = 680.0;
pub const WINDOW_MAX_H:   f32 = 480.0;

pub const ROW_H_PROJECT:  f32 = 48.0;
pub const ROW_H_ACTION:   f32 = 40.0;
pub const ROW_H_HEADER:   f32 = 28.0;
pub const ICON_SIZE:      f32 = 20.0;

pub const FONT_TITLE:     f32 = 14.0;
pub const FONT_SUBTITLE:  f32 = 12.0;
pub const FONT_SECTION:   f32 = 11.0;
```

- [ ] **Step 2: Update `src/ui/mod.rs`**

```rust
pub mod hints;
pub mod list;
pub mod search;
pub mod theme;
```

- [ ] **Step 3: Verify compile**

```bash
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src/ui/
git commit -m "feat: UI theme constants"
```

---

### Task 14: `app.rs` — `App` struct and state machine

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Define `App` and `Mode`**

```rust
use std::sync::mpsc::Receiver;
use std::collections::HashMap;
use egui::TextureHandle;
use crate::icons::{IconLoadResult, IconResolver};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode { Search, Add }

/// Actions produced during a frame that require daemon-level responses.
#[derive(Debug)]
pub enum AppAction {
    Hide,
    OpenTerminal(String),
    RemoveProject(String),
}

pub struct App {
    pub visible: bool,
    pub mode: Mode,
    pub query: String,
    pub selected_idx: Option<usize>,
    pub recents: Vec<String>,
    pub icon_resolver: IconResolver,
    pub icon_textures: HashMap<String, TextureHandle>,
    pub pending_actions: Vec<AppAction>,
    pub focus_search: bool,  // request keyboard focus on search entry next frame
}

impl App {
    pub fn new() -> Self {
        let recents = crate::projects::load_recents();
        App {
            visible: false,
            mode: Mode::Search,
            query: String::new(),
            selected_idx: None,
            recents,
            icon_resolver: IconResolver::new(),
            icon_textures: HashMap::new(),
            pending_actions: Vec::new(),
            focus_search: false,
        }
    }

    pub fn on_show(&mut self) {
        self.query.clear();
        self.selected_idx = None;
        self.mode = Mode::Search;
        self.visible = true;
        self.focus_search = true; // consumed by ui() on next frame
    }

    pub fn on_hide(&mut self) {
        self.visible = false;
    }

    /// Called from daemon's handle_app_output each frame.
    pub fn drain_actions(&mut self) -> Vec<AppAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Poll icon resolver for newly loaded textures. Call once per frame.
    pub fn poll_icons(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.icon_resolver.rx.try_recv() {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.rgba,
            );
            let handle = ctx.load_texture(
                &result.project_path,
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.icon_textures.insert(result.project_path, handle);
        }
    }

    pub fn filtered_recents(&self) -> Vec<&str> {
        self.recents.iter()
            .filter(|p| crate::projects::fuzzy_match(&self.query, p))
            .map(String::as_str)
            .collect()
    }

    pub fn suggestions(&self) -> Vec<String> {
        crate::paths::get_suggestions(&self.query)
    }

    pub fn open_project(&mut self, path: &str) {
        let path = path.to_string();
        self.recents.retain(|p| p != &path);
        self.recents.insert(0, path.clone());
        crate::projects::save_recents(&self.recents);
        self.pending_actions.push(AppAction::OpenTerminal(path));
        self.pending_actions.push(AppAction::Hide);
    }

    pub fn remove_project(&mut self, path: &str) {
        self.recents.retain(|p| p != path);
        crate::projects::save_recents(&self.recents);
    }
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: App struct with state machine, actions, icon polling"
```

---

### Task 15: `ui/search.rs` — search bar

**Files:**
- Modify: `src/ui/search.rs`

- [ ] **Step 1: Implement search bar**

```rust
use egui::{Ui, TextEdit, Color32, Vec2};
use crate::ui::theme;

pub const SEARCH_ID: &str = "project_picker_search";

/// Returns true if the query changed. If `request_focus` is true, grabs keyboard focus.
pub fn search_bar(ui: &mut Ui, query: &mut String, placeholder: &str, request_focus: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        // Magnifying glass icon (Unicode fallback — replace with bundled SVG image if desired)
        ui.label(egui::RichText::new("⌕").size(18.0).color(theme::TEXT_MUTED));
        ui.add_space(8.0);
        let response = ui.add(
            TextEdit::singleline(query)
                .id(egui::Id::new(SEARCH_ID))
                .hint_text(egui::RichText::new(placeholder).color(theme::TEXT_MUTED))
                .frame(false)
                .desired_width(f32::INFINITY)
                .font(egui::FontId::proportional(theme::FONT_TITLE))
                .text_color(theme::TEXT_PRIMARY)
                .cursor_color(theme::ACCENT),
        );
        if request_focus {
            response.request_focus();
        }
        changed = response.changed();
    });
    changed
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/search.rs
git commit -m "feat: search bar widget"
```

---

### Task 16: `ui/list.rs` — all row types

**Files:**
- Modify: `src/ui/list.rs`

- [ ] **Step 1: Implement section header**

```rust
use egui::{Ui, Response, Color32, Rect, Vec2, Pos2};
use crate::ui::theme;

pub fn section_header(ui: &mut Ui, label: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(label.to_uppercase())
                .size(theme::FONT_SECTION)
                .color(theme::SECTION_HEADER),
        );
    });
    ui.add_space(4.0);
}
```

- [ ] **Step 2: Implement `row_background` helper**

```rust
/// Draws a clickable row background, returns response. `selected` highlights the row.
fn row_background(ui: &mut Ui, height: f32, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    let bg = if selected {
        theme::ROW_SELECTED
    } else if response.hovered() {
        theme::ROW_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    response
}
```

- [ ] **Step 3: Implement action row**

```rust
/// Returns true if clicked.
pub fn action_row(ui: &mut Ui, icon: &str, label: &str, selected: bool) -> bool {
    let response = row_background(ui, theme::ROW_H_ACTION, selected);
    // Draw content over the background
    let mut child = ui.child_ui(response.rect, egui::Layout::left_to_right(egui::Align::Center), None);
    child.add_space(16.0);
    child.label(egui::RichText::new(icon).size(theme::ICON_SIZE).color(theme::TEXT_MUTED));
    child.add_space(12.0);
    child.label(egui::RichText::new(label).size(theme::FONT_TITLE).color(theme::TEXT_PRIMARY));
    // Chevron on right
    let right_ui_rect = Rect::from_min_size(
        Pos2::new(response.rect.right() - 24.0, response.rect.top()),
        Vec2::new(24.0, theme::ROW_H_ACTION),
    );
    let mut right_ui = ui.child_ui(right_ui_rect, egui::Layout::right_to_left(egui::Align::Center), None);
    right_ui.label(egui::RichText::new("›").size(16.0).color(theme::TEXT_MUTED));

    response.clicked()
}
```

- [ ] **Step 4: Implement project row**

```rust
/// Returns true if clicked. `icon_texture` is optional resolved icon.
pub fn project_row(
    ui: &mut Ui,
    path: &str,
    icon: Option<&egui::TextureHandle>,
    selected: bool,
) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    let response = row_background(ui, theme::ROW_H_PROJECT, selected);
    let mut child = ui.child_ui(response.rect, egui::Layout::left_to_right(egui::Align::Center), None);
    child.add_space(16.0);

    // Icon
    if let Some(tex) = icon {
        child.image(egui::load::SizedTexture::new(tex.id(), [theme::ICON_SIZE, theme::ICON_SIZE]));
    } else {
        child.label(egui::RichText::new("📁").size(theme::ICON_SIZE));
    }
    child.add_space(12.0);

    // Text column
    child.vertical(|ui| {
        ui.label(
            egui::RichText::new(name).size(theme::FONT_TITLE).strong().color(theme::TEXT_PRIMARY),
        );
        ui.label(
            egui::RichText::new(path).size(theme::FONT_SUBTITLE).color(theme::TEXT_MUTED),
        );
    });

    response.clicked()
}
```

- [ ] **Step 5: Implement suggestion row**

```rust
pub fn suggestion_row(ui: &mut Ui, path: &str, selected: bool) -> bool {
    let response = row_background(ui, theme::ROW_H_ACTION, selected);
    let mut child = ui.child_ui(response.rect, egui::Layout::left_to_right(egui::Align::Center), None);
    child.add_space(16.0);
    child.label(egui::RichText::new("📁").size(theme::ICON_SIZE).color(theme::TEXT_MUTED));
    child.add_space(12.0);
    child.label(egui::RichText::new(path).size(theme::FONT_TITLE).color(theme::TEXT_PRIMARY));
    response.clicked()
}
```

- [ ] **Step 6: Verify compile**

```bash
cargo check
```

- [ ] **Step 7: Commit**

```bash
git add src/ui/list.rs
git commit -m "feat: section header, action/project/suggestion row widgets"
```

---

### Task 17: `ui/hints.rs` — keyboard hints bar

**Files:**
- Modify: `src/ui/hints.rs`

- [ ] **Step 1: Implement hints bar**

```rust
use egui::{Ui, Color32};
use crate::ui::theme;

pub fn hints_bar(ui: &mut Ui) {
    ui.add(egui::Separator::default().horizontal().spacing(0.0));
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        kbd(ui, "↑");
        kbd(ui, "↓");
        hint(ui, " Navigate");
        ui.add_space(16.0);
        kbd(ui, "Enter");
        hint(ui, " Select");
        ui.add_space(16.0);
        kbd(ui, "Alt+⌫");
        hint(ui, " Remove");
        ui.add_space(16.0);
        kbd(ui, "Esc");
        hint(ui, " Close");
    });
}

fn kbd(ui: &mut Ui, label: &str) {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_string(), egui::FontId::proportional(11.0), theme::KBD_TEXT));
    let padding = egui::Vec2::new(6.0, 3.0);
    let desired = galley.size() + padding * 2.0;
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, theme::KBD_BG);
    ui.painter().galley(rect.min + padding, galley, theme::KBD_TEXT);
    ui.add_space(4.0);
}

fn hint(ui: &mut Ui, label: &str) {
    ui.label(egui::RichText::new(label).size(11.0).color(theme::TEXT_MUTED));
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/hints.rs
git commit -m "feat: keyboard hints bar with kbd badges"
```

---

### Task 18: `app.rs` — `ui()` method with keyboard navigation

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Implement the main `ui()` method**

```rust
impl App {
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.poll_icons(ctx);

        // Request icons for all recents that don't have one yet
        for path in &self.recents {
            if !self.icon_textures.contains_key(path) {
                self.icon_resolver.request(path);
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(crate::ui::theme::BG))
            .show(ctx, |ui| {
                // Apply global style
                ui.style_mut().spacing.item_spacing = egui::Vec2::new(0.0, 0.0);
                ui.style_mut().visuals.selection.bg_fill = crate::ui::theme::ACCENT;

                // Search bar
                let placeholder = match self.mode {
                    Mode::Search => "Search projects...",
                    Mode::Add    => "Type directory path...",
                };
                let grab = self.focus_search;
                self.focus_search = false;
                crate::ui::search::search_bar(ui, &mut self.query, placeholder, grab);

                ui.add(egui::Separator::default().horizontal().spacing(0.0));

                // List area
                egui::ScrollArea::vertical()
                    .max_height(crate::ui::theme::WINDOW_MAX_H - 80.0)
                    .show(ui, |ui| {
                        self.render_list(ui);
                    });

                // Hints bar
                crate::ui::hints::hints_bar(ui);
            });

        self.handle_keyboard(ctx);
    }

    fn render_list(&mut self, ui: &mut egui::Ui) {
        match self.mode {
            Mode::Search => self.render_search_list(ui),
            Mode::Add    => self.render_add_list(ui),
        }
    }

    fn render_search_list(&mut self, ui: &mut egui::Ui) {
        let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();

        if self.query.is_empty() {
            crate::ui::list::section_header(ui, "Actions");
            let add_selected = self.selected_idx == Some(0);
            if crate::ui::list::action_row(ui, "⊕", "Add project", add_selected) {
                self.enter_add_mode();
            }
        }

        if !filtered.is_empty() {
            let offset = if self.query.is_empty() { 1 } else { 0 }; // offset for action rows
            crate::ui::list::section_header(ui, "Recent Projects");
            for (i, path) in filtered.iter().enumerate() {
                let selected = self.selected_idx == Some(i + offset);
                let icon = self.icon_textures.get(path.as_str());
                if crate::ui::list::project_row(ui, path, icon, selected) {
                    self.open_project(path);
                }
            }
        }
    }

    fn render_add_list(&mut self, ui: &mut egui::Ui) {
        let suggestions = self.suggestions();
        for (i, path) in suggestions.iter().enumerate() {
            let selected = self.selected_idx == Some(i);
            if crate::ui::list::suggestion_row(ui, path, selected) {
                self.add_and_open(path);
            }
        }
    }

    fn enter_add_mode(&mut self) {
        self.mode = Mode::Add;
        self.query.clear();
        self.selected_idx = None;
    }

    fn add_and_open(&mut self, path: &str) {
        let abs = crate::paths::expand_tilde(path);
        if std::path::Path::new(&abs).is_dir() {
            self.open_project(path);
        }
    }
}
```

- [ ] **Step 2: Implement keyboard navigation**

```rust
impl App {
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // Count selectable items in current view
        let selectable_count = self.selectable_count();

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        match key {
                            egui::Key::Escape => {
                                if self.mode == Mode::Add {
                                    self.mode = Mode::Search;
                                    self.query.clear();
                                    self.selected_idx = None;
                                } else {
                                    self.pending_actions.push(AppAction::Hide);
                                }
                            }
                            egui::Key::Enter => {
                                self.activate_selected();
                            }
                            egui::Key::ArrowDown | egui::Key::Tab => {
                                if modifiers.shift && *key == egui::Key::Tab {
                                    self.move_selection(-1, selectable_count);
                                } else {
                                    self.move_selection(1, selectable_count);
                                }
                            }
                            egui::Key::ArrowUp => {
                                self.move_selection(-1, selectable_count);
                            }
                            egui::Key::Backspace if modifiers.alt => {
                                if let Some(idx) = self.selected_idx {
                                    if self.mode == Mode::Search {
                                        let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                                        let offset = if self.query.is_empty() { 1 } else { 0 };
                                        if idx >= offset {
                                            if let Some(path) = filtered.get(idx - offset) {
                                                let path = path.clone();
                                                self.remove_project(&path);
                                                self.selected_idx = None;
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn selectable_count(&self) -> usize {
        match self.mode {
            Mode::Search => {
                let n = self.filtered_recents().len();
                if self.query.is_empty() { n + 1 } else { n } // +1 for Add action
            }
            Mode::Add => self.suggestions().len(),
        }
    }

    fn move_selection(&mut self, delta: i32, count: usize) {
        if count == 0 { return; }
        let current = self.selected_idx.unwrap_or(usize::MAX) as i32;
        let next = current + delta;
        if next < 0 {
            self.selected_idx = None; // go back to search entry
        } else {
            self.selected_idx = Some((next as usize).min(count - 1));
        }
    }

    fn activate_selected(&mut self) {
        let selectable = self.selectable_count();
        let idx = match self.selected_idx {
            Some(i) => i,
            None => {
                // Enter with no selection → activate first item
                if selectable > 0 { 0 } else { return; }
            }
        };

        match self.mode {
            Mode::Search => {
                let offset = if self.query.is_empty() { 1 } else { 0 };
                if idx == 0 && self.query.is_empty() {
                    self.enter_add_mode();
                } else {
                    let filtered = self.filtered_recents().iter().map(|s| s.to_string()).collect::<Vec<_>>();
                    if let Some(path) = filtered.get(idx.saturating_sub(offset)) {
                        let path = path.clone();
                        self.open_project(&path);
                    }
                }
            }
            Mode::Add => {
                let suggestions = self.suggestions();
                if let Some(path) = suggestions.get(idx) {
                    let path = path.clone();
                    self.add_and_open(&path);
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify compile**

```bash
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: app UI method, list rendering, keyboard navigation"
```

---

## Chunk 5: IPC + Integration

### Task 19: `daemon.rs` — Unix socket server

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Implement socket listener insertion into calloop**

```rust
use calloop::generic::Generic;
use calloop::Interest;
use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};

const SOCKET_PATH: &str = "/tmp/project-picker.sock";

impl State {
    pub fn setup_socket(loop_handle: &LoopHandle<'static, Self>) {
        // Remove stale socket
        let _ = std::fs::remove_file(SOCKET_PATH);
        let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind Unix socket");
        listener.set_nonblocking(true).unwrap();

        let source = Generic::new(listener, Interest::READ, calloop::Mode::Level);
        loop_handle.insert_source(source, |_, listener, state| {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = String::new();
                    let _ = stream.read_to_string(&mut buf);
                    for line in buf.lines() {
                        state.handle_ipc_command(line.trim());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => eprintln!("Socket accept error: {}", e),
            }
            Ok(calloop::PostAction::Continue)
        }).expect("Failed to insert socket source");
    }

    fn handle_ipc_command(&mut self, cmd: &str) {
        // Set a flag; the idle callback in run_daemon processes it with access to qh.
        match cmd {
            "toggle" => { self.pending_toggle = true; }
            _ => {}
        }
    }
}
```

Add `pending_toggle: bool` to the `State` struct.

- [ ] **Step 2: Handle pending_toggle in the event loop idle callback**

The idle callback is the closure passed to `event_loop.run(...)`. Update `run_daemon` (Task 20) to handle it there.

- [ ] **Step 3: Verify compile**

```bash
cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs
git commit -m "feat: Unix socket IPC server with calloop integration"
```

---

### Task 20: `daemon.rs` — `run_daemon` and `handle_app_output`

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Implement `run_daemon`**

There is exactly one `EventLoop` in the process, created here and passed into `State::init`.
`qh` comes from the Wayland event queue and is stored in `State` (added in Task 11), so the
idle callback can call `show`/`hide` without holding a separate reference.

```rust
pub fn run_daemon() {
    // 1. Create the single event loop for this process
    let mut event_loop: EventLoop<'static, State> = EventLoop::new().expect("event loop");
    let loop_handle = event_loop.handle();

    // 2. Connect to Wayland and get globals
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn).expect("Failed to get Wayland globals");
    let qh = event_queue.handle();

    // 3. Insert Wayland source into calloop — clone conn first; State::init also needs it
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .expect("Failed to insert Wayland source");

    // 4. Build state (uses the globals and qh we just created)
    let mut state = State::init(loop_handle.clone(), qh, conn, globals);

    // 5. Attach Unix socket source
    State::setup_socket(&loop_handle);

    // 6. Run — idle callback handles pending toggle and rendering
    event_loop.run(None, &mut state, |state| {
        if state.pending_toggle {
            state.pending_toggle = false;
            if state.app.visible {
                state.hide();
            } else {
                let qh = state.qh.clone();
                state.show(&qh);
            }
        }

        if state.needs_redraw && state.app.visible {
            let qh = state.qh.clone();
            state.render_frame(&qh);
        }
    }).expect("Event loop error");
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/daemon.rs
git commit -m "feat: run_daemon event loop with single EventLoop and toggle handling"
```

---

### Task 21: `main.rs` — CLI entry point with `--toggle`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement full `main.rs`**

```rust
mod app;
mod assets;
mod daemon;
mod icons;
mod paths;
mod projects;
mod terminal;
mod ui;

use std::io::Write;
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/project-picker.sock";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let toggle = args.iter().any(|a| a == "--toggle");

    if toggle {
        match send_toggle() {
            Ok(()) => return,
            Err(_) => {
                // Daemon not running — start it in background, then send toggle
                start_daemon_background();
                // Wait for socket to appear (max 2s)
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if send_toggle().is_ok() {
                        return;
                    }
                }
                eprintln!("project-picker: daemon did not start in time");
                std::process::exit(1);
            }
        }
    } else {
        // Start daemon in foreground
        daemon::run_daemon();
    }
}

fn send_toggle() -> std::io::Result<()> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    stream.write_all(b"toggle\n")?;
    Ok(())
}

fn start_daemon_background() {
    let exe = std::env::current_exe().expect("Cannot find current executable");
    std::process::Command::new(exe)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build
```

Expected: binary produced at `target/debug/project-picker`.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: CLI entry point with --toggle and cold-start daemon"
```

---

### Task 22: Smoke test and Hyprland integration

**Files:**
- No code changes — this is a manual integration task

- [ ] **Step 1: Build release binary**

```bash
cargo build --release
```

Expected: binary at `target/release/project-picker`. No errors.

- [ ] **Step 2: Start daemon and test toggle**

Terminal 1:
```bash
./target/release/project-picker
```

Terminal 2:
```bash
./target/release/project-picker --toggle
```

Expected: window appears on screen. Run `--toggle` again → window hides.

- [ ] **Step 3: Test project selection**

With window open:
- Type a project name → filtered list appears
- Press Down to select → row highlights
- Press Enter → ghostty opens in project directory, window hides

- [ ] **Step 4: Test add-project mode**

With window open:
- Press Down to select "Add project", Enter
- Type `~/` → path suggestions appear
- Select a directory → it's added to recents, terminal opens

- [ ] **Step 5: Configure Hyprland autostart**

Add to `~/.config/hypr/hyprland.conf`:
```
exec-once = /home/ryan/projects/project-picker/target/release/project-picker
```

- [ ] **Step 6: Configure Hyprland keybind**

Add to `~/.config/hypr/hyprland.conf`:
```
bind = SUPER, P, exec, /home/ryan/projects/project-picker/target/release/project-picker --toggle
```

Adjust the keybind to your preference (`SUPER, P` = Super+P).

- [ ] **Step 7: Test cold-start keybind**

Kill the daemon if running, then press the keybind. Expected: daemon starts and window appears within ~1 second.

- [ ] **Step 8: Final commit**

```bash
git add ~/.config/hypr/hyprland.conf
git commit -m "chore: add Hyprland exec-once and keybind for project-picker"
```

---

## Notes for Implementers

**On crate version conflicts:** Run `cargo tree | grep wgpu` after initial `cargo fetch`. If egui-wgpu and your `wgpu` line resolve to different versions, add `wgpu = { version = "X.Y" }` matching exactly what `egui-wgpu` uses.

**On `QueueHandle` in the event loop idle callback:** `QueueHandle<State>` is `Clone + Send`. Store it in `State` during `init` so it's accessible in the idle closure.

**On SCK macro requirements:** SCK uses `delegate_*!` macros. At the bottom of `daemon.rs`, add:
```rust
smithay_client_toolkit::delegate_compositor!(State);
smithay_client_toolkit::delegate_output!(State);
smithay_client_toolkit::delegate_seat!(State);
smithay_client_toolkit::delegate_keyboard!(State);
smithay_client_toolkit::delegate_pointer!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_registry!(State);
```

**On the `egui_ctx.run()` call:** In egui 0.29, the method is `Context::run(raw_input, add_contents) -> FullOutput`. The `add_contents` closure receives a `&Context` ref.

**On window height:** The layer-shell is configured with `set_size(680, 0)`. The compositor will send a configure event with the available height. In `LayerShellHandler::configure`, use the compositor-provided height or fall back to `WINDOW_MAX_H`. egui's `CentralPanel` will use the full configured height. To make the window shrink-to-content, you need to set `set_size(680, actual_content_height)` after each frame — measure egui's `min_rect` from the response and call `layer_surface.set_size`.
