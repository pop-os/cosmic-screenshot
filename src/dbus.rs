// SPDX-License-Identifier: GPL-3.0-only

use crate::screenshot::{ScreenshotKind, ScreenshotManager, ScreenshotOptions};
use crate::settings::APP_ID;
use std::collections::HashMap;
use std::path::PathBuf;
use zbus::{connection, fdo, interface, zvariant::Value, Connection};

pub struct ScreenshotService {
    manager: ScreenshotManager,
}

impl ScreenshotService {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            manager: ScreenshotManager::new(),
        }
    }
}

impl Default for ScreenshotService {
    fn default() -> Self {
        Self::new()
    }
}

#[interface(name = "com.system76.CosmicScreenshot")]
impl ScreenshotService {
    /// Take a screenshot with the specified options
    ///
    /// # Arguments
    /// * `kind` - Screenshot type: "all", "screen", "window", "select", "region"
    /// * `delay_ms` - Delay in milliseconds before taking screenshot
    /// * `save_to_clipboard` - Whether to save to clipboard
    /// * `save_dir` - Optional directory to save screenshot
    ///
    /// # Returns
    /// A dictionary containing:
    /// * `path` - File path if saved to file (optional)
    /// * `saved_to_clipboard` - Boolean indicating if saved to clipboard
    /// * `thumbnail_data` - PNG thumbnail data as bytes
    /// * `full_image_data` - Full resolution PNG image data as bytes
    async fn take_screenshot(
        &self,
        kind: &str,
        delay_ms: u32,
        save_to_clipboard: bool,
        save_dir: String,
    ) -> fdo::Result<HashMap<String, Value<'static>>> {
        let screenshot_kind = match kind {
            "screen" => ScreenshotKind::ScreenUnderCursor,
            "window" => ScreenshotKind::WindowUnderCursor,
            "select" => ScreenshotKind::SelectScreen,
            "region" => ScreenshotKind::RectangularRegion,
            _ => ScreenshotKind::AllScreens,
        };

        let options = ScreenshotOptions {
            kind: screenshot_kind,
            delay_ms,
            save_to_clipboard,
            save_dir: if save_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(save_dir))
            },
        };

        // For region selection, we cannot run cosmic::app::run from within an async context
        // Instead, return an error asking the user to use the CLI
        if screenshot_kind == ScreenshotKind::RectangularRegion {
            return Err(fdo::Error::Failed(
                "Region selection via D-Bus is not supported. Please use 'cosmic-screenshot take --kind region' from the command line.".to_string()
            ));
        }
        
        match self.manager.take_screenshot(&options).await {
            Ok(result) => {
                let mut response = HashMap::new();

                if let Some(path) = result.path {
                    response.insert(
                        "path".to_string(),
                        Value::Str(path.to_string_lossy().to_string().into()),
                    );
                }

                response.insert(
                    "saved_to_clipboard".to_string(),
                    Value::Bool(result.saved_to_clipboard),
                );
                response.insert(
                    "thumbnail_data".to_string(),
                    Value::Array(result.thumbnail_data.into()),
                );
                response.insert(
                    "full_image_data".to_string(),
                    Value::Array(result.full_image_data.into()),
                );

                Ok(response)
            }
            Err(err) => Err(fdo::Error::Failed(format!("Screenshot failed: {err}"))),
        }
    }

    /// Get available screenshot backends
    ///
    /// # Returns
    /// Array of available screenshot backend names
    async fn get_available_backends(&self) -> fdo::Result<Vec<String>> {
        Ok(self.manager.get_available_grabbers().await)
    }

    /// Take a screenshot with backend selection
    ///
    /// # Arguments
    /// * `kind` - Screenshot type: "all", "screen", "window", "select", "region"
    /// * `delay_ms` - Delay in milliseconds before taking screenshot
    /// * `save_to_clipboard` - Whether to save to clipboard
    /// * `save_dir` - Optional directory to save screenshot
    /// * `backend` - Backend to use: "auto", "kwin", "freedesktop", etc.
    ///
    /// # Returns
    /// A dictionary containing:
    /// * `path` - File path if saved to file (optional)
    /// * `saved_to_clipboard` - Boolean indicating if saved to clipboard
    /// * `thumbnail_data` - PNG thumbnail data as bytes
    /// * `full_image_data` - Full resolution PNG image data as bytes
    async fn take_screenshot_with_backend(
        &self,
        kind: &str,
        delay_ms: u32,
        save_to_clipboard: bool,
        save_dir: String,
        backend: String,
    ) -> fdo::Result<HashMap<String, Value<'static>>> {
        let screenshot_kind = match kind {
            "screen" => ScreenshotKind::ScreenUnderCursor,
            "window" => ScreenshotKind::WindowUnderCursor,
            "select" => ScreenshotKind::SelectScreen,
            "region" => ScreenshotKind::RectangularRegion,
            _ => ScreenshotKind::AllScreens,
        };

        let options = ScreenshotOptions {
            kind: screenshot_kind,
            delay_ms,
            save_to_clipboard,
            save_dir: if save_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(save_dir))
            },
        };

        // For region selection, we cannot run cosmic::app::run from within an async context
        // Instead, return an error asking the user to use the CLI
        if screenshot_kind == ScreenshotKind::RectangularRegion {
            return Err(fdo::Error::Failed(
                "Region selection via D-Bus is not supported. Please use 'cosmic-screenshot take --kind region' from the command line.".to_string()
            ));
        }
        
        let backend_name = if backend == "auto" { None } else { Some(backend.as_str()) };
        match self.manager.take_screenshot_with_backend(&options, backend_name).await {
            Ok(result) => {
                let mut response = HashMap::new();

                if let Some(path) = result.path {
                    response.insert(
                        "path".to_string(),
                        Value::Str(path.to_string_lossy().to_string().into()),
                    );
                }

                response.insert(
                    "saved_to_clipboard".to_string(),
                    Value::Bool(result.saved_to_clipboard),
                );
                response.insert(
                    "thumbnail_data".to_string(),
                    Value::Array(result.thumbnail_data.into()),
                );
                response.insert(
                    "full_image_data".to_string(),
                    Value::Array(result.full_image_data.into()),
                );

                Ok(response)
            }
            Err(err) => Err(fdo::Error::Failed(format!("Screenshot failed: {err}"))),
        }
    }

    /// Check if a specific screenshot kind is supported
    ///
    /// # Arguments
    /// * `kind` - Screenshot type to check
    ///
    /// # Returns
    /// Boolean indicating if the kind is supported
    async fn supports_kind(&self, kind: &str) -> fdo::Result<bool> {
        let screenshot_kind = match kind {
            "all" => ScreenshotKind::AllScreens,
            "screen" => ScreenshotKind::ScreenUnderCursor,
            "window" => ScreenshotKind::WindowUnderCursor,
            "select" => ScreenshotKind::SelectScreen,
            "region" => ScreenshotKind::RectangularRegion,
            _ => return Ok(false),
        };

        if let Some(grabber) = self.manager.get_available_grabber().await {
            Ok(grabber.supports_kind(screenshot_kind))
        } else {
            Ok(false)
        }
    }

    /// Check if a specific screenshot kind is supported by a backend
    ///
    /// # Arguments
    /// * `kind` - Screenshot type to check
    /// * `backend` - Backend name to check
    ///
    /// # Returns
    /// Boolean indicating if the kind is supported by the backend
    async fn supports_kind_with_backend(&self, kind: &str, backend: &str) -> fdo::Result<bool> {
        let screenshot_kind = match kind {
            "all" => ScreenshotKind::AllScreens,
            "screen" => ScreenshotKind::ScreenUnderCursor,
            "window" => ScreenshotKind::WindowUnderCursor,
            "select" => ScreenshotKind::SelectScreen,
            "region" => ScreenshotKind::RectangularRegion,
            _ => return Ok(false),
        };

        Ok(self.manager.supports_kind_with_backend(screenshot_kind, backend).await)
    }

    /// Get detailed backend capabilities
    ///
    /// # Returns
    /// A dictionary where keys are backend names and values are arrays of supported screenshot kinds
    async fn get_backend_capabilities(&self) -> fdo::Result<HashMap<String, Vec<String>>> {
        let capabilities = self.manager.get_backend_capabilities().await;
        let mut result = HashMap::new();
        
        for (backend, kinds) in capabilities {
            let kind_strings: Vec<String> = kinds.into_iter().map(|kind| {
                match kind {
                    crate::screenshot::ScreenshotKind::AllScreens => "all".to_string(),
                    crate::screenshot::ScreenshotKind::ScreenUnderCursor => "screen".to_string(),
                    crate::screenshot::ScreenshotKind::WindowUnderCursor => "window".to_string(),
                    crate::screenshot::ScreenshotKind::SelectScreen => "select".to_string(),
                    crate::screenshot::ScreenshotKind::RectangularRegion => "region".to_string(),
                }
            }).collect();
            result.insert(backend, kind_strings);
        }
        
        Ok(result)
    }
}

pub struct ScreenshotServiceInterface {
    connection: Connection,
}

impl ScreenshotServiceInterface {
    /// Create a new D-Bus service interface
    #[allow(clippy::missing_errors_doc)]
    pub async fn new() -> zbus::Result<Self> {
        let service = ScreenshotService::new();
        let object_path = format!("/{}", APP_ID.replace('.', "/"));
        let connection = connection::Builder::session()?
            .name(APP_ID)?
            .serve_at(object_path.as_str(), service)?
            .build()
            .await?;

        Ok(Self { connection })
    }

    /// Run the D-Bus service
    #[allow(clippy::missing_errors_doc)]
    pub async fn run(&self) -> zbus::Result<()> {
        // Wait for termination signals for graceful shutdown
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        
        println!("D-Bus service running. Press Ctrl+C to stop.");
        
        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down gracefully...");
            },
            _ = sigint.recv() => {
                println!("Received SIGINT (Ctrl+C), shutting down gracefully...");
            },
        }
        
        Ok(())
    }

    /// Get the D-Bus connection
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
