// SPDX-License-Identifier: GPL-3.0-only

use super::{Screengrabber, ScreenshotOptions, ScreenshotResult, ScreenshotError, ScreenshotKind};
use ashpd::desktop::screenshot::Screenshot;
use async_trait::async_trait;
use std::{fs, path::PathBuf, os::unix::fs::MetadataExt};
use chrono::Local;

pub struct PortalScreengrabber {
    _private: (),
}

impl Default for PortalScreengrabber {
    fn default() -> Self {
        Self::new()
    }
}

impl PortalScreengrabber {
    #[must_use] 
    pub fn new() -> Self {
        Self { _private: () }
    }
    
    fn generate_thumbnail(image_path: &PathBuf) -> Result<Vec<u8>, ScreenshotError> {
        let img = image::open(image_path)?;
        
        // Calculate thumbnail size maintaining aspect ratio, targeting 360p
        let (orig_width, orig_height) = (img.width(), img.height());
        #[allow(clippy::cast_precision_loss)]
        let aspect_ratio = orig_width as f32 / orig_height as f32;
        
        let (thumb_width, thumb_height) = if orig_height <= 360 {
            // Already smaller than 360p, use original size
            (orig_width, orig_height)
        } else {
            // Scale down to 360p height, preserve aspect ratio
            let height = 360u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let width = (360.0 * aspect_ratio) as u32;
            (width, height)
        };
        
        let thumbnail = img.thumbnail(thumb_width, thumb_height);
        
        let mut thumbnail_data = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut thumbnail_data);
        thumbnail.write_to(&mut cursor, image::ImageFormat::Png)?;
        
        Ok(thumbnail_data)
    }
}

#[async_trait]
impl Screengrabber for PortalScreengrabber {
    async fn is_available(&self) -> bool {
        // Try to connect to the portal
        (Screenshot::request().send().await).is_ok()
    }
    
    async fn take_screenshot(&self, options: &ScreenshotOptions) -> Result<ScreenshotResult, ScreenshotError> {
        // Add delay if specified
        if options.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(u64::from(options.delay_ms))).await;
        }
        
        let mut request = Screenshot::request();
        
        // Configure the screenshot based on kind
        // NOTE: Freedesktop Portal capabilities:
        // - Non-interactive: Full workspace screenshot only
        // - Interactive: User selection dialog for screen/region/window
        match options.kind {
            ScreenshotKind::AllScreens => {
                // Non-interactive mode captures the entire workspace
                request = request.interactive(false);
            }
            ScreenshotKind::ScreenUnderCursor |
            ScreenshotKind::SelectScreen | 
            ScreenshotKind::RectangularRegion |
            ScreenshotKind::WindowUnderCursor => {
                // Interactive mode lets user choose screen, region, or window
                // The portal will show a selection dialog where user can pick any type
                request = request.interactive(true).modal(true);
            }
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| ScreenshotError::Portal(e.to_string()))?
            .response()
            .map_err(|e| ScreenshotError::Portal(e.to_string()))?;
        
        let uri = response.uri();
        
        match uri.scheme() {
            "file" => {
                let temp_path = PathBuf::from(uri.path());
                let final_path = if let Some(save_dir) = &options.save_dir {
                    let date = Local::now();
                    let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                    let path = save_dir.join(filename);
                    
                    // Move or copy the file
                    if fs::metadata(save_dir)?.dev() == fs::metadata(&temp_path)?.dev() {
                        fs::rename(&temp_path, &path)?;
                    } else {
                        fs::copy(&temp_path, &path)?;
                        fs::remove_file(&temp_path)?;
                    }
                    
                    Some(path)
                } else {
                    Some(temp_path)
                };
                
                let thumbnail_data = if let Some(ref path) = final_path {
                    Self::generate_thumbnail(path)?
                } else {
                    Vec::new()
                };
                
                let full_image_data = if let Some(ref path) = final_path {
                    fs::read(path)?
                } else {
                    Vec::new()
                };
                
                Ok(ScreenshotResult {
                    path: final_path,
                    saved_to_clipboard: false,
                    thumbnail_data,
                    full_image_data,
                })
            }
            "clipboard" => {
                Ok(ScreenshotResult {
                    path: None,
                    saved_to_clipboard: true,
                    thumbnail_data: Vec::new(), // Can't generate thumbnail from clipboard
                    full_image_data: Vec::new(), // Can't get full image from clipboard
                })
            }
            scheme => Err(ScreenshotError::Portal(format!("Unsupported scheme: {scheme}"))),
        }
    }
    
    fn name(&self) -> &'static str {
        "Freedesktop Portal"
    }
    
    fn supports_kind(&self, kind: ScreenshotKind) -> bool {
        // Portal supports all screenshot kinds via interactive mode
        match kind {
            ScreenshotKind::AllScreens | 
            ScreenshotKind::ScreenUnderCursor |
            ScreenshotKind::SelectScreen | 
            ScreenshotKind::RectangularRegion | 
            ScreenshotKind::WindowUnderCursor => true, // All screenshot types supported via portal
        }
    }
}