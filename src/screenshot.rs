// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error_handling::{report_error, ErrorSeverity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenshotKind {
    AllScreens,
    ScreenUnderCursor,
    WindowUnderCursor,
    SelectScreen,
    RectangularRegion,
}

impl Default for ScreenshotKind {
    fn default() -> Self {
        Self::AllScreens
    }
}

impl std::fmt::Display for ScreenshotKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllScreens => write!(f, "All screens"),
            Self::ScreenUnderCursor => write!(f, "Screen under cursor"),
            Self::WindowUnderCursor => write!(f, "Window under cursor"),
            Self::SelectScreen => write!(f, "Select screen"),
            Self::RectangularRegion => write!(f, "Rectangular region"),
        }
    }
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ScreenshotOptions {
    pub kind: ScreenshotKind,
    pub delay_ms: u32,
    pub save_to_clipboard: bool,
    pub save_dir: Option<PathBuf>,
}


#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    pub path: Option<PathBuf>,
    pub saved_to_clipboard: bool,
    pub thumbnail_data: Vec<u8>,
    pub full_image_data: Vec<u8>, // Full resolution image data for region selection
}

#[derive(thiserror::Error, Debug)]
pub enum ScreenshotError {
    #[error("Portal error: {0}")]
    Portal(String),
    #[error("KWin error: {0}")]
    KWin(String),
    #[error("DBus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Screengrabber not available")]
    NotAvailable,
    #[error("Operation cancelled")]
    Cancelled,
}

#[async_trait]
pub trait Screengrabber: Send + Sync {
    async fn is_available(&self) -> bool;
    
    async fn take_screenshot(&self, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError>;
    
    fn name(&self) -> &'static str;
    
    fn supports_kind(&self, kind: ScreenshotKind) -> bool;
}

pub mod freedesktop_portal;
pub mod kwin_screenshot2;

#[cfg(target_os = "windows")]
pub mod windows_native;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod xorg_native;

#[derive(Clone)]
pub struct ScreenshotManager {
    grabbers: std::sync::Arc<Vec<Box<dyn Screengrabber>>>,
}

impl Default for ScreenshotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenshotManager {
    #[must_use] 
    pub fn new() -> Self {
        // Add platform-specific screengrabbers in order of preference
        // Prefer KWin for better screen-specific capture support
        let grabbers: Vec<Box<dyn Screengrabber>> = vec![
            Box::new(kwin_screenshot2::KWinScreengrabber::new()),
            Box::new(freedesktop_portal::PortalScreengrabber::new()),
            #[cfg(target_os = "windows")]
            Box::new(windows_native::WindowsScreengrabber::new()),
            #[cfg(all(unix, not(target_os = "macos")))]
            Box::new(xorg_native::XorgScreengrabber::new()),
        ];
        
        Self { grabbers: std::sync::Arc::new(grabbers) }
    }
    
    /// Takes a screenshot using a specific backend
    ///
    /// # Errors
    /// Returns `ScreenshotError` if no compatible backend is found, backend fails, or region selection is attempted
    pub async fn take_screenshot_with_backend(&self, options: &ScreenshotOptions, backend_name: Option<&str>) -> Result<ScreenshotResult, ScreenshotError> {
        // Note: RectangularRegion should not reach this method
        // CLI launches GUI, D-Bus rejects it, GUI/library use snipper directly
        if options.kind == ScreenshotKind::RectangularRegion {
            return Err(ScreenshotError::Portal("Rectangular region selection should use get_screenshot_for_region_selection() and snipper instead".to_string()));
        }
        
        // If specific backend is requested, try to find it
        if let Some(backend_name) = backend_name {
            for grabber in self.grabbers.iter() {
                if grabber.name().to_lowercase().contains(&backend_name.to_lowercase()) && 
                   grabber.is_available().await && 
                   grabber.supports_kind(options.kind) {
                    return grabber.take_screenshot(options).await;
                }
            }
            return Err(ScreenshotError::Portal(format!("Backend '{backend_name}' not found or not available")));
        }
        
        // Auto mode - try backends with fallback
        let mut last_error = None;
        for grabber in self.grabbers.iter() {
            if grabber.is_available().await && grabber.supports_kind(options.kind) {
                match grabber.take_screenshot(options).await {
                    Ok(result) => return Ok(result),
                    Err(err) => {
                        report_error(ErrorSeverity::Warning, "Backend Fallback", &format!("Backend {} failed: {}, trying next backend...", grabber.name(), err));
                        last_error = Some(err);
                        // Continue to next backend
                    }
                }
            }
        }
        
        // If we get here, all backends failed or none were available
        Err(last_error.unwrap_or(ScreenshotError::NotAvailable))
    }
    
    pub async fn get_available_grabbers(&self) -> Vec<String> {
        let mut available = Vec::new();
        for grabber in self.grabbers.iter() {
            if grabber.is_available().await {
                available.push(grabber.name().to_string());
            }
        }
        available
    }
    
    pub async fn get_available_grabber(&self) -> Option<&Box<dyn Screengrabber>> {
        for grabber in self.grabbers.iter() {
            if grabber.is_available().await {
                return Some(grabber);
            }
        }
        None
    }
    
    pub async fn supports_kind_with_backend(&self, kind: ScreenshotKind, backend_name: &str) -> bool {
        for grabber in self.grabbers.iter() {
            if grabber.name().to_lowercase().contains(&backend_name.to_lowercase()) && 
               grabber.is_available().await {
                return grabber.supports_kind(kind);
            }
        }
        false
    }
    
    pub async fn get_grabber_by_name(&self, backend_name: &str) -> Option<&Box<dyn Screengrabber>> {
        for grabber in self.grabbers.iter() {
            if grabber.name().to_lowercase().contains(&backend_name.to_lowercase()) && 
               grabber.is_available().await {
                return Some(grabber);
            }
        }
        None
    }
    
    pub async fn get_backend_capabilities(&self) -> std::collections::HashMap<String, Vec<ScreenshotKind>> {
        let mut capabilities = std::collections::HashMap::new();
        
        for grabber in self.grabbers.iter() {
            if grabber.is_available().await {
                let mut supported_kinds = Vec::new();
                
                for kind in [
                    ScreenshotKind::AllScreens,
                    ScreenshotKind::ScreenUnderCursor,
                    ScreenshotKind::WindowUnderCursor,
                    ScreenshotKind::SelectScreen,
                    ScreenshotKind::RectangularRegion,
                ] {
                    if grabber.supports_kind(kind) {
                        supported_kinds.push(kind);
                    }
                }
                
                capabilities.insert(grabber.name().to_string(), supported_kinds);
            }
        }
        
        capabilities
    }
    
    /// Takes a screenshot using automatic backend selection
    ///
    /// # Errors
    /// Returns `ScreenshotError` if no backends are available, all backends fail, or region selection is attempted
    pub async fn take_screenshot(&self, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // Note: RectangularRegion should not reach this method
        // CLI launches GUI, D-Bus rejects it, GUI/library use snipper directly  
        if options.kind == ScreenshotKind::RectangularRegion {
            return Err(ScreenshotError::Portal("Rectangular region selection should use get_screenshot_for_region_selection() and snipper instead".to_string()));
        }
        
        // For all other screenshot types, try backends with fallback
        let mut last_error = None;
        for grabber in self.grabbers.iter() {
            if grabber.is_available().await && grabber.supports_kind(options.kind) {
                match grabber.take_screenshot(options).await {
                    Ok(result) => return Ok(result),
                    Err(err) => {
                        report_error(ErrorSeverity::Warning, "Backend Fallback", &format!("Backend {} failed: {}, trying next backend...", grabber.name(), err));
                        last_error = Some(err);
                        // Continue to next backend
                    }
                }
            }
        }
        
        // If we get here, all backends failed or none were available
        Err(last_error.unwrap_or(ScreenshotError::NotAvailable))
    }
    
    /// Get screenshot data for interactive region selection
    /// Returns the full screenshot data and metadata needed to create a Snipper
    ///
    /// # Errors
    /// Returns `ScreenshotError` if screenshot capture fails or image processing fails
    pub async fn get_screenshot_for_region_selection(&self) -> Result<(std::collections::HashMap<String, Vec<u8>>, cosmic::iced::Rectangle), ScreenshotError> {
        // Take current screen screenshot for region selection
        let options = ScreenshotOptions {
            kind: ScreenshotKind::ScreenUnderCursor,
            delay_ms: 0,
            save_to_clipboard: false,
            save_dir: None,
        };
        
        let result = self.take_screenshot(&options).await?;
        
        // Create screen images map (using "primary" as key for compatibility)
        let mut screen_images = std::collections::HashMap::new();
        screen_images.insert("primary".to_string(), result.full_image_data);
        
        // Get screen bounds (for now, assume full screen - this could be improved)
        // TODO: Get actual screen dimensions from the backend
        let screen_bounds = cosmic::iced::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1920.0, // Default fallback - should get actual screen size
            height: 1080.0,
        };
        
        Ok((screen_images, screen_bounds))
    }
}