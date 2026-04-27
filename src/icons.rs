use std::path::Path;
use crate::assets;

#[derive(Debug, Clone, PartialEq)]
pub enum IconKind {
    BundledSvg(&'static [u8]),
    ImageFile(std::path::PathBuf),
    Folder,
}

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
    // Check for .py files
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

pub fn rasterize_svg(svg_bytes: &[u8], size: u32) -> Option<(Vec<u8>, u32, u32)> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some((pixmap.data().to_vec(), size, size))
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

    #[test]
    fn test_rasterize_bundled_svg() {
        let (bytes, w, h) = rasterize_svg(crate::assets::FOLDER_SVG, 20).unwrap();
        assert_eq!(w, 20);
        assert_eq!(h, 20);
        assert_eq!(bytes.len() as u32, w * h * 4); // RGBA
    }
}
