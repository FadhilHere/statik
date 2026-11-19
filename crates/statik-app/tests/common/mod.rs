//! Shared test support.
//!
//! `egui_kittest` renders with wgpu (snapshots, `ViewportCommand::Screenshot`). Rendering
//! from several test threads at once crashed the process now and then (0xc0000005), so every
//! test that builds a `Harness` holds [`render_lock`] for its whole run. Cargo runs the test
//! binaries one after another, so a lock per binary serialises all rendering.
#![allow(dead_code)]
use std::sync::{Mutex, MutexGuard};

static RENDER: Mutex<()> = Mutex::new(());

/// Hold this for the whole test (a failed test poisons nothing for the others).
pub fn render_lock() -> MutexGuard<'static, ()> {
    RENDER.lock().unwrap_or_else(|e| e.into_inner())
}
