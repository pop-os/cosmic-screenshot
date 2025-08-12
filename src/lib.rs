// SPDX-License-Identifier: GPL-3.0-only

//! Screenshot functionality for COSMIC desktop
//! 
//! This crate provides screenshot capabilities for COSMIC desktop applications
//! with support for multiple backends and a D-Bus interface for external programs.

pub mod screenshot;

// Re-export main types for easier usage
pub use screenshot::{
    ScreenshotKind, ScreenshotOptions, ScreenshotResult, ScreenshotError,
    Screengrabber, ScreenshotManager
};

// Re-export snipper types for library integration
pub use snipper::{
    Snipper, SnipperMessage, SnipperResult, SnipperState
};

pub mod ui;
pub mod dbus;
pub mod snipper;
pub mod app;
pub mod settings;
pub mod error_handling;
pub mod notifications;

/// The current version of the cosmic-screenshot library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");