//! Statik desktop application (egui). The binary in `main.rs` only boots the window; everything
//! else lives here so it can be tested headless (`tests/`).
pub mod app;
pub mod data_view;
pub mod dialogs;
pub mod edit;
pub mod editor;
pub mod icon;
pub mod jobs;
pub mod menu;
pub mod output;
pub mod state;
pub mod syntax;
pub mod variable_view;

pub use app::StatikApp;
pub use jobs::SaveKind;
pub use state::{
    AppState, Command, DialogKind, Doc, Message, OutputEntry, ProcOutput, run_procedure,
};
