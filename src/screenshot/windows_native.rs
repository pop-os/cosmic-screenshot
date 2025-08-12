// SPDX-License-Identifier: GPL-3.0-only

#[cfg(target_os = "windows")]
use super::{Screengrabber, ScreenshotOptions, ScreenshotResult, ScreenshotError, ScreenshotKind};
#[cfg(target_os = "windows")]
use async_trait::async_trait;

#[cfg(target_os = "windows")]
pub struct WindowsScreengrabber {
    _private: (),
}

#[cfg(target_os = "windows")]
impl WindowsScreengrabber {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl Screengrabber for WindowsScreengrabber {
    async fn is_available(&self) -> bool {
        // On Windows, we can always use the native API
        true
    }
    
    async fn take_screenshot(&self, _options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // TODO: Implement using Windows API (user32.dll, gdi32.dll)
        Err(ScreenshotError::NotAvailable)
    }
    
    fn name(&self) -> &'static str {
        "Windows Native"
    }
    
    fn supports_kind(&self, _kind: ScreenshotKind) -> bool {
        // Windows native API supports most kinds
        true
    }
}

#[cfg(not(target_os = "windows"))]
pub struct WindowsScreengrabber;

#[cfg(not(target_os = "windows"))]
impl WindowsScreengrabber {
    pub fn new() -> Self {
        Self
    }
}