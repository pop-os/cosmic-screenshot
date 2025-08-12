// SPDX-License-Identifier: GPL-3.0-only

#[cfg(all(unix, not(target_os = "macos")))]
use super::{Screengrabber, ScreenshotOptions, ScreenshotResult, ScreenshotError, ScreenshotKind};
#[cfg(all(unix, not(target_os = "macos")))]
use async_trait::async_trait;

#[cfg(all(unix, not(target_os = "macos")))]
pub struct XorgScreengrabber {
    _private: (),
}

#[cfg(all(unix, not(target_os = "macos")))]
impl Default for XorgScreengrabber {
    fn default() -> Self {
        Self::new()
    }
}

impl XorgScreengrabber {
    #[must_use] 
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[async_trait]
impl Screengrabber for XorgScreengrabber {
    async fn is_available(&self) -> bool {
        // Check if we're running under X11
        std::env::var("DISPLAY").is_ok() && std::env::var("WAYLAND_DISPLAY").is_err()
    }
    
    async fn take_screenshot(&self, _options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // TODO: Implement using X11 API (libX11, libXext)
        Err(ScreenshotError::NotAvailable)
    }
    
    fn name(&self) -> &'static str {
        "X11 Native"
    }
    
    fn supports_kind(&self, _kind: ScreenshotKind) -> bool {
        // X11 supports most screenshot kinds
        true
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub struct XorgScreengrabber;

#[cfg(not(all(unix, not(target_os = "macos"))))]
impl XorgScreengrabber {
    pub fn new() -> Self {
        Self
    }
}