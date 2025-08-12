// SPDX-License-Identifier: GPL-3.0-only

use super::{Screengrabber, ScreenshotOptions, ScreenshotResult, ScreenshotError, ScreenshotKind};
use crate::error_handling::{report_error, ErrorSeverity};
use async_trait::async_trait;
use std::{collections::HashMap, os::unix::io::FromRawFd};
use zbus::{proxy, zvariant::{Value, OwnedValue}, Connection};
use zbus::zvariant::Fd;
use chrono::Local;
use tokio::io::AsyncReadExt;

#[proxy(
    interface = "org.kde.KWin.ScreenShot2",
    default_service = "org.kde.KWin.ScreenShot2",
    default_path = "/org/kde/KWin/ScreenShot2"
)]
trait KWinScreenShot2 {
    /// Capture a specific window
    fn capture_window(
        &self, 
        handle: &str, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Capture the active window
    fn capture_active_window(
        &self, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Capture a specific area
    fn capture_area(
        &self, 
        x: i32, 
        y: i32, 
        width: u32, 
        height: u32, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Capture a specific screen by name
    fn capture_screen(
        &self, 
        name: &str, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Capture the active screen
    fn capture_active_screen(
        &self, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Interactive capture with user selection
    fn capture_interactive(
        &self, 
        kind: u32, // 0 = window, 1 = screen
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Capture the entire workspace
    fn capture_workspace(
        &self, 
        options: HashMap<&str, &Value<'_>>, 
        pipe: Fd<'_>
    ) -> zbus::Result<HashMap<String, OwnedValue>>;

    /// Get interface version
    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
}

pub struct KWinScreengrabber {
    _private: (),
}

impl Default for KWinScreengrabber {
    fn default() -> Self {
        Self::new()
    }
}

impl KWinScreengrabber {
    #[must_use] 
    pub fn new() -> Self {
        Self { _private: () }
    }
    
    
    async fn read_image_from_pipe(pipe_fd: i32) -> Result<Vec<u8>, ScreenshotError> {
        let file = unsafe { std::fs::File::from_raw_fd(pipe_fd) };
        let mut async_file = tokio::fs::File::from_std(file);
        let mut buffer = Vec::new();
        async_file.read_to_end(&mut buffer).await?;
        Ok(buffer)
    }
    
    async fn try_screenshot(&self, proxy: &KWinScreenShot2Proxy<'_>, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // Create pipe using libc directly to avoid ownership issues
        let mut pipe_fds: [std::os::raw::c_int; 2] = [0; 2];
        let result = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };
        if result != 0 {
            return Err(ScreenshotError::Io(std::io::Error::last_os_error()));
        }
        
        let read_fd = pipe_fds[0];
        let write_fd = pipe_fds[1];
        
        // Create OwnedFd from write_fd to pass to KWin
        let owned_write_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(write_fd) };
        let fd = Fd::from(owned_write_fd);
        
        let mut kwin_options = HashMap::new();
        kwin_options.insert("include-cursor", &Value::Bool(false));
        kwin_options.insert("native-resolution", &Value::Bool(true));
        
        let result = match options.kind {
            ScreenshotKind::AllScreens => {
                proxy.capture_workspace(kwin_options, fd).await?
            }
            ScreenshotKind::ScreenUnderCursor => {
                proxy.capture_active_screen(kwin_options, fd).await?
            }
            ScreenshotKind::WindowUnderCursor => {
                proxy.capture_active_window(kwin_options, fd).await?
            }
            ScreenshotKind::SelectScreen => {
                proxy.capture_interactive(1, kwin_options, fd).await? // 1 = screen
            }
            ScreenshotKind::RectangularRegion => {
                proxy.capture_interactive(0, kwin_options, fd).await? // 0 = window (closest to area selection)
            }
        };
        
        // Write end is automatically closed when fd is dropped after the call
        // Now read from read end - KWin has finished writing
        let image_data = Self::read_image_from_pipe(read_fd).await?;
        
        // Close read end manually
        unsafe { libc::close(read_fd) };
        
        // KWin returns raw pixel data with metadata, not standard image formats
        self.save_screenshot_data(image_data, result, options).await
    }
    
    async fn fallback_to_interactive(&self, proxy: &KWinScreenShot2Proxy<'_>, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // Create pipe using libc directly to avoid ownership issues
        let mut pipe_fds: [std::os::raw::c_int; 2] = [0; 2];
        let result = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };
        if result != 0 {
            return Err(ScreenshotError::Io(std::io::Error::last_os_error()));
        }
        
        let read_fd = pipe_fds[0];
        let write_fd = pipe_fds[1];
        
        // Create OwnedFd from write_fd to pass to KWin
        let owned_write_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(write_fd) };
        let fd = Fd::from(owned_write_fd);
        
        let mut kwin_options = HashMap::new();
        kwin_options.insert("include-cursor", &Value::Bool(false));
        kwin_options.insert("include-decoration", &Value::Bool(true));
        kwin_options.insert("include-shadow", &Value::Bool(true));
        kwin_options.insert("native-resolution", &Value::Bool(true));
        
        // Use interactive capture - this should work even without explicit authorization
        // NOTE: Interactive mode cannot capture AllScreens - it requires user selection
        let interactive_kind = match options.kind {
            ScreenshotKind::AllScreens => {
                // Interactive mode can't do AllScreens - return error
                return Err(ScreenshotError::KWin("Interactive mode does not support AllScreens capture".to_string()));
            }
            ScreenshotKind::WindowUnderCursor => 0, // window selection
            _ => 1, // screen selection for screen/region types
        };
        
        let result = proxy.capture_interactive(interactive_kind, kwin_options, fd).await?;
        
        // Extract image metadata from the result
        println!("KWin result metadata: {result:?}");
        
        // Write end is automatically closed when fd is dropped after the call
        // Now read from read end - KWin has finished writing
        let image_data = Self::read_image_from_pipe(read_fd).await?;
        
        // Debug: Check image data length and format
        println!("KWin fallback_to_interactive: Received {} bytes", image_data.len());
        if image_data.len() >= 8 {
            println!("First 8 bytes: {:?}", &image_data[0..8]);
        }
        
        // Close read end manually
        unsafe { libc::close(read_fd) };
        
        // Process the raw image data with metadata from KWin
        self.save_screenshot_data(image_data, result, options).await
    }
    
    #[allow(clippy::unused_async)]
    async fn save_screenshot_data(&self, image_data: Vec<u8>, metadata: std::collections::HashMap<String, zbus::zvariant::OwnedValue>, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // Extract image metadata from KWin response
        let width = metadata.get("width")
            .and_then(|v| v.downcast_ref::<u32>().ok())
            .ok_or_else(|| ScreenshotError::Portal("Missing width in KWin response".to_string()))?;
        
        let height = metadata.get("height")
            .and_then(|v| v.downcast_ref::<u32>().ok())
            .ok_or_else(|| ScreenshotError::Portal("Missing height in KWin response".to_string()))?;
        
        // Convert raw image data based on QImage format
        let format = metadata.get("format")
            .and_then(|v| v.downcast_ref::<u32>().ok())
            .ok_or_else(|| ScreenshotError::Portal("Missing format in KWin response".to_string()))?;
        
        println!("KWin image format: {format}");
        
        let img = match format {
            5 => {
                // QImage::Format_ARGB32 (0xAARRGGBB) - need to convert ARGB to RGBA
                let mut rgba_data = Vec::with_capacity(image_data.len());
                for chunk in image_data.chunks_exact(4) {
                    let argb = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let a = ((argb >> 24) & 0xff) as u8;
                    let r = ((argb >> 16) & 0xff) as u8;
                    let g = ((argb >> 8) & 0xff) as u8;
                    let b = (argb & 0xff) as u8;
                    rgba_data.extend_from_slice(&[r, g, b, a]);
                }
                
                match image::RgbaImage::from_raw(width, height, rgba_data) {
                    Some(rgba_img) => image::DynamicImage::ImageRgba8(rgba_img),
                    None => return Err(ScreenshotError::Portal("Failed to create RGBA image from ARGB data".to_string())),
                }
            }
            6 => {
                // QImage::Format_ARGB32_Premultiplied - similar to Format_ARGB32 but pre-multiplied alpha
                let mut rgba_data = Vec::with_capacity(image_data.len());
                for chunk in image_data.chunks_exact(4) {
                    let argb = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let a = ((argb >> 24) & 0xff) as u8;
                    let r = ((argb >> 16) & 0xff) as u8;
                    let g = ((argb >> 8) & 0xff) as u8;
                    let b = (argb & 0xff) as u8;
                    rgba_data.extend_from_slice(&[r, g, b, a]);
                }
                
                match image::RgbaImage::from_raw(width, height, rgba_data) {
                    Some(rgba_img) => image::DynamicImage::ImageRgba8(rgba_img),
                    None => return Err(ScreenshotError::Portal("Failed to create RGBA image from premultiplied ARGB data".to_string())),
                }
            }
            13 => {
                // QImage::Format_RGB888 - RGB format, need to convert to RGBA
                let mut rgba_data = Vec::with_capacity(image_data.len() * 4 / 3);
                for chunk in image_data.chunks_exact(3) {
                    rgba_data.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]); // Add alpha=255
                }
                
                match image::RgbaImage::from_raw(width, height, rgba_data) {
                    Some(rgba_img) => image::DynamicImage::ImageRgba8(rgba_img),
                    None => return Err(ScreenshotError::Portal("Failed to create RGBA image from RGB data".to_string())),
                }
            }
            _ => {
                // Unknown format - try as RGBA as fallback
                println!("Unknown QImage format {format}, trying as RGBA");
                match image::RgbaImage::from_raw(width, height, image_data) {
                    Some(rgba_img) => image::DynamicImage::ImageRgba8(rgba_img),
                    None => return Err(ScreenshotError::Portal(format!("Failed to create image from unknown format {format}"))),
                }
            }
        };
        
        let final_path = if let Some(save_dir) = &options.save_dir {
            let date = Local::now();
            let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
            let path = save_dir.join(filename);
            img.save(&path)?;
            Some(path)
        } else {
            let temp_dir = std::env::temp_dir();
            let date = Local::now();
            let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
            let path = temp_dir.join(filename);
            img.save(&path)?;
            Some(path)
        };
        
        // Generate thumbnail from the converted image
        let thumbnail = img.thumbnail(320, 240);
        let mut thumbnail_data = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut thumbnail_data);
        thumbnail.write_to(&mut cursor, image::ImageFormat::Png)?;
        
        // Store full resolution image data for region selection
        let mut full_image_data = Vec::new();
        let mut cursor_full = std::io::Cursor::new(&mut full_image_data);
        img.write_to(&mut cursor_full, image::ImageFormat::Png)?;
        
        Ok(ScreenshotResult {
            path: final_path,
            saved_to_clipboard: options.save_to_clipboard,
            thumbnail_data,
            full_image_data,
        })
    }
    
}

#[async_trait]
impl Screengrabber for KWinScreengrabber {
    async fn is_available(&self) -> bool {
        match Connection::session().await {
            Ok(connection) => {
                (KWinScreenShot2Proxy::new(&connection).await).is_ok()
            }
            Err(_) => false,
        }
    }
    
    async fn take_screenshot(&self, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        let connection = Connection::session().await?;
        let proxy = KWinScreenShot2Proxy::new(&connection).await?;
        
        // Add delay if specified
        if options.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(u64::from(options.delay_ms))).await;
        }
        
        // Try the normal screenshot method first
        match self.try_screenshot(&proxy, options).await {
            Ok(result) => Ok(result),
            Err(ScreenshotError::DBus(zbus_error)) => {
                let error_msg = zbus_error.to_string();
                if error_msg.contains("NoAuthorized") || error_msg.contains("org.kde.KWin.ScreenShot2.Error.NoAuthorized") {
                    // Show authorization error message
                    report_error(
                        ErrorSeverity::Warning,
                        "KWin Authorization",
                        "Screenshot permission not granted. The application needs the org.kde.KWin.ScreenShot2 interface listed in X-KDE-DBUS-Restricted-Interfaces. Falling back to interactive mode..."
                    );
                    
                    // Fall back to interactive mode
                    self.fallback_to_interactive(&proxy, options).await
                } else {
                    Err(ScreenshotError::DBus(zbus_error))
                }
            }
            Err(err) => Err(err),
        }
    }
    
    fn name(&self) -> &'static str {
        "KWin ScreenShot2"
    }
    
    fn supports_kind(&self, _kind: ScreenshotKind) -> bool {
        // KWin supports all screenshot kinds when properly authorized
        // NOTE: If falling back to interactive mode, AllScreens is not supported
        // but we can't detect authorization state here
        true
    }
}