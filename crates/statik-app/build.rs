//! Embeds the application icon (and version info) into `statik-app.exe` on Windows with
//! `winresource`, which needs a resource compiler: `rc.exe` from the Windows SDK, or the one
//! named by the `RC` environment variable (`tools/release.sh` sets it on this laptop). Without
//! one the build still succeeds and only prints a warning; the window icon is set at runtime
//! (`src/icon.rs`) either way.
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.png");
    println!("cargo:rerun-if-env-changed=RC");
    // Only release builds get the resource (debug and test builds stay quick and quiet).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("PROFILE").as_deref() != Ok("release")
    {
        return;
    }
    if let Err(e) = embed_icon() {
        println!("cargo:warning=no embedded icon: {e}");
    }
}

/// `assets/icon.png` (256x256) wrapped in a one-image `.ico` (PNG-compressed entry, Vista+).
fn write_ico(out: &Path) -> Result<PathBuf, String> {
    let png = std::fs::read("assets/icon.png").map_err(|e| format!("assets/icon.png: {e}"))?;
    let mut ico = Vec::with_capacity(png.len() + 22);
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]); // reserved, type = icon, 1 image
    // 256x256 (0 = 256), no palette, reserved, 1 plane, 32 bpp, size, offset 22.
    ico.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&png);
    let path = out.join("statik.ico");
    std::fs::write(&path, ico).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(windows)]
fn embed_icon() -> Result<(), String> {
    let out = PathBuf::from(std::env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let ico = write_ico(&out)?;
    let mut res = winresource::WindowsResource::new();
    if let Some(rc) = std::env::var_os("RC").map(PathBuf::from)
        && let Some(dir) = rc.parent()
    {
        res.set_toolkit_path(&dir.to_string_lossy());
    }
    res.set_icon(&ico.to_string_lossy())
        .set("ProductName", "Statik")
        .set("FileDescription", "Statik")
        .set("LegalCopyright", "");
    res.compile().map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn embed_icon() -> Result<(), String> {
    let _ = write_ico;
    Err("icon embedding is only done on a Windows host".into())
}
