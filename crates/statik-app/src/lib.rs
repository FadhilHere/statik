//! Statik desktop application (egui). The binary in `main.rs` only boots the window; everything
//! else lives here so it can be tested headless (`tests/`).
pub mod app;
pub mod dialogs;
pub mod icon;
pub mod jobs;
pub mod menu;
pub mod state;

pub use app::StatikApp;
pub use state::{AppState, Command, DialogKind, Doc, Message, OutputEntry, run_procedure};
