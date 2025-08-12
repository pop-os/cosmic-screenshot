// SPDX-License-Identifier: GPL-3.0-only

use crate::screenshot::{ScreenshotKind, ScreenshotOptions, ScreenshotResult, ScreenshotManager, ScreenshotError};
use crate::snipper::{Snipper, SnipperMessage, SnipperResult};
use crate::settings::SettingsManager;
use crate::error_handling::{report_error, report_success, ErrorSeverity};
use cosmic::widget;
use cosmic::iced::Rectangle;
use cosmic::dialog::file_chooser;
use cosmic_config::CosmicConfigEntry;
use std::collections::HashMap;
use image;

#[derive(Debug, Clone)]
pub enum ScreenshotMessage {
    SetScreenshotKind(ScreenshotKind),
    SetScreenshotDelay(String),
    SetScreenshotBackend(usize),
    TakeScreenshot,
    SaveScreenshot,
    ScreenshotComplete(Result<ScreenshotResult, String>),
    LaunchRegionSelection(ScreenshotResult),
    RegionSelected(cosmic::iced::Rectangle),
    RegionSelectionCancelled,
    SnipperMessage(SnipperMessage),
    BackendsLoaded(Vec<String>),
    OpenSnipperWindow(ScreenshotResult),
    SnipperWindowOpened(cosmic::iced::window::Id),
    ShowSnipperWindow,
    HideSnipperWindow,
    CloseSnipperWindow,
    SnipperWindowClosed(cosmic::iced::window::Id),
    // Path selection messages
    OpenSaveDirectoryDialog,
    SaveDirectorySelected(Option<std::path::PathBuf>),
    ToggleRememberSaveDirectory(bool),
    // Selection memory messages
    ToggleRememberSelectionArea(bool),
    // Screenshot on startup settings
    ToggleScreenshotOnStartup(bool),
    // Main window management
    MainWindowOpened(cosmic::iced::window::Id),
    // Generic window events
    WindowCloseRequested(cosmic::iced::window::Id),
    WindowClosed(cosmic::iced::window::Id),
    // Application exit
    Exit,
    // Error handling messages
    DismissErrorDialog,
    OpenErrorDialog(String, String), // (title, message) - opens new window
    ErrorDialogOpened(cosmic::iced::window::Id),
    ErrorDialogClosed(cosmic::iced::window::Id),
}

#[allow(clippy::struct_excessive_bools)]
pub struct ScreenshotWidget {
    pub screenshot_manager: ScreenshotManager,
    pub screenshot_kind: ScreenshotKind,
    pub screenshot_delay_str: String,
    pub last_screenshot: Option<ScreenshotResult>,
    pub screenshot_in_progress: bool,
    pub screenshot_options: Vec<String>,
    pub available_backends: Vec<String>,
    pub selected_backend: usize,
    pub snipper: Option<Snipper>,
    pub region_selection_mode: bool,
    pub cached_thumbnail_handle: Option<cosmic::iced::widget::image::Handle>,
    // Window optimization - reuse snipper window
    pub snipper_window_id: Option<cosmic::iced::window::Id>,
    // Path selection fields
    pub save_directory: Option<std::path::PathBuf>,
    pub remember_save_directory: bool,
    pub show_path_selection: bool,
    // Selection memory fields
    pub remember_selection_area: bool,
    pub last_selection_area: Option<cosmic::iced::Rectangle>,
    // Settings management
    pub settings_manager: SettingsManager,
    // Error dialog state
    pub error_dialog: Option<(String, String)>, // (title, message)
    pub error_dialog_window_id: Option<cosmic::iced::window::Id>,
}

impl Default for ScreenshotWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenshotWidget {
    /// Creates a new `ScreenshotWidget`
    ///
    /// # Panics
    /// Panics if unable to create a `cosmic_config::Config` instance
    pub fn new() -> Self {
        use crate::settings::{ScreenshotSettings, APP_ID};
        
        // Initialize settings manager
        let settings_manager = SettingsManager::new().unwrap_or_else(|e| {
            report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to load settings: {e}"));
            // Create a fallback with default settings
            let config = cosmic_config::Config::new(APP_ID, ScreenshotSettings::VERSION)
                .unwrap_or_else(|_| panic!("Unable to create config"));
            SettingsManager { config, settings: ScreenshotSettings::default() }
        });

        // Check if we're in CLI mode and read CLI options
        let (screenshot_kind, screenshot_delay_str, save_directory) = if std::env::var("CLI_MODE_REGION").is_ok() {
            let delay = std::env::var("CLI_DELAY").ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let output_dir = std::env::var("CLI_OUTPUT_DIR").ok()
                .and_then(|s| std::path::PathBuf::from(s).canonicalize().ok())
                .or_else(dirs::picture_dir);
            
            (ScreenshotKind::RectangularRegion, delay.to_string(), output_dir)
        } else {
            // Use settings for non-CLI mode
            let kind = Self::kind_from_string(&settings_manager.settings.last_screenshot_kind);
            let delay_str = settings_manager.settings.last_screenshot_delay.to_string();
            let save_dir = if settings_manager.settings.remember_save_directory {
                settings_manager.settings.last_save_directory.clone()
            } else {
                dirs::picture_dir()
            };
            (kind, delay_str, save_dir)
        };

        Self {
            screenshot_manager: ScreenshotManager::new(),
            screenshot_kind,
            screenshot_delay_str,
            last_screenshot: None,
            screenshot_in_progress: false,
            screenshot_options: vec![
                "All screens".to_string(),
                "Screen under cursor".to_string(),
                "Window under cursor".to_string(),
                "Select screen".to_string(),
                "Rectangular region".to_string(),
            ],
            available_backends: vec!["Auto".to_string()],
            selected_backend: settings_manager.settings.last_selected_backend,
            snipper: None,
            region_selection_mode: false,
            cached_thumbnail_handle: None,
            // Initialize path selection
            save_directory,
            remember_save_directory: settings_manager.settings.remember_save_directory,
            show_path_selection: false,
            // Initialize selection memory
            remember_selection_area: settings_manager.settings.remember_selection_area,
            last_selection_area: settings_manager.settings.last_selection_area.clone().map(Into::into),
            // Window optimization
            snipper_window_id: None,
            // Settings management
            settings_manager,
            // Error dialog state
            error_dialog: None,
            error_dialog_window_id: None,
        }
    }

    fn kind_from_string(kind_str: &str) -> ScreenshotKind {
        match kind_str {
            "All screens" => ScreenshotKind::AllScreens,
            "Screen under cursor" => ScreenshotKind::ScreenUnderCursor,
            "Window under cursor" => ScreenshotKind::WindowUnderCursor,
            "Select screen" => ScreenshotKind::SelectScreen,
            "Rectangular region" => ScreenshotKind::RectangularRegion,
            _ => ScreenshotKind::default(),
        }
    }

    fn kind_to_string(kind: ScreenshotKind) -> String {
        match kind {
            ScreenshotKind::AllScreens => "All screens".to_string(),
            ScreenshotKind::ScreenUnderCursor => "Screen under cursor".to_string(),
            ScreenshotKind::WindowUnderCursor => "Window under cursor".to_string(),
            ScreenshotKind::SelectScreen => "Select screen".to_string(),
            ScreenshotKind::RectangularRegion => "Rectangular region".to_string(),
        }
    }
    
    fn update_thumbnail_cache(&mut self) {
        self.cached_thumbnail_handle = if let Some(ref screenshot) = self.last_screenshot {
            if let Ok(img) = image::load_from_memory(&screenshot.thumbnail_data) {
                let rgba_img = img.to_rgba8();
                let (width, height) = rgba_img.dimensions();
                Some(cosmic::iced::widget::image::Handle::from_rgba(
                    width,
                    height,
                    rgba_img.into_raw()
                ))
            } else {
                None
            }
        } else {
            None
        };
    }
    
    pub fn init() -> cosmic::Task<ScreenshotMessage> {
        let manager = ScreenshotManager::new();
        
        // Check if we're in CLI region mode
        if std::env::var("CLI_MODE_REGION").is_ok() {
            // Start region selection immediately in CLI mode
            cosmic::Task::batch([
                cosmic::Task::perform(
                    async move {
                        let mut backends = manager.get_available_grabbers().await;
                        backends.insert(0, "Auto".to_string());
                        ScreenshotMessage::BackendsLoaded(backends)
                    },
                    |msg| msg,
                ),
                cosmic::Task::done(ScreenshotMessage::TakeScreenshot),
            ])
        } else {
            // Check if screenshot on startup is enabled
            let settings_manager = SettingsManager::new().ok();
            let screenshot_on_startup = settings_manager
                .as_ref()
                .is_some_and(|sm| sm.settings.screenshot_on_startup);
            
            if screenshot_on_startup {
                // Load backends and take screenshot automatically
                cosmic::Task::batch([
                    cosmic::Task::perform(
                        async move {
                            let mut backends = manager.get_available_grabbers().await;
                            backends.insert(0, "Auto".to_string());
                            ScreenshotMessage::BackendsLoaded(backends)
                        },
                        |msg| msg,
                    ),
                    cosmic::Task::done(ScreenshotMessage::TakeScreenshot),
                ])
            } else {
                // Normal startup - just load backends
                cosmic::Task::perform(
                    async move {
                        let mut backends = manager.get_available_grabbers().await;
                        backends.insert(0, "Auto".to_string());
                        ScreenshotMessage::BackendsLoaded(backends)
                    },
                    |msg| msg,
                )
            }
        }
    }
    
    /// Updates the widget state based on the given message
    ///
    /// # Panics
    /// May panic if primary display screen images are not available
    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, message: ScreenshotMessage) -> cosmic::Task<ScreenshotMessage> {
        match message {
            ScreenshotMessage::MainWindowOpened(_) => {
                // OS-level window open events are handled by main app, not the widget
                // This is used for CLI mode logic, not snipper setup
                cosmic::Task::none()
            }
            ScreenshotMessage::SetScreenshotKind(kind) => {
                self.screenshot_kind = kind;
                // Save to settings
                let delay = self.screenshot_delay_str.parse().unwrap_or(0);
                if let Err(e) = self.settings_manager.update_screenshot_settings(
                    &Self::kind_to_string(kind),
                    delay,
                    self.selected_backend
                ) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save screenshot settings: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::SetScreenshotDelay(delay_str) => {
                self.screenshot_delay_str.clone_from(&delay_str);
                // Save to settings
                let delay = delay_str.parse().unwrap_or(0);
                if let Err(e) = self.settings_manager.update_screenshot_settings(
                    &Self::kind_to_string(self.screenshot_kind),
                    delay,
                    self.selected_backend
                ) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save screenshot settings: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::SetScreenshotBackend(index) => {
                self.selected_backend = index;
                // Save to settings
                let delay = self.screenshot_delay_str.parse().unwrap_or(0);
                if let Err(e) = self.settings_manager.update_screenshot_settings(
                    &Self::kind_to_string(self.screenshot_kind),
                    delay,
                    index
                ) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save screenshot settings: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::BackendsLoaded(backends) => {
                self.available_backends = backends;
                cosmic::Task::none()
            }
            ScreenshotMessage::TakeScreenshot => {
                println!("TakeScreenshot triggered with kind: {:?}", self.screenshot_kind);
                self.screenshot_in_progress = true;
                
                // Handle region screenshots differently using the correct API
                if self.screenshot_kind == ScreenshotKind::RectangularRegion {
                    let manager = self.screenshot_manager.clone();
                    return cosmic::Task::perform(
                        async move {
                            println!("Using get_screenshot_for_region_selection for rectangular region");
                            match manager.get_screenshot_for_region_selection().await {
                                Ok((screen_images, _screen_bounds)) => {
                                    // Create a ScreenshotResult from the region selection data
                                    // Use the primary screen image
                                    let image_data = screen_images.get("primary").unwrap().clone();
                                    let result = ScreenshotResult {
                                        path: None, // No file saved yet
                                        saved_to_clipboard: false,
                                        full_image_data: image_data.clone(),
                                        thumbnail_data: image_data, // Will be updated after region selection
                                    };
                                    ScreenshotMessage::ScreenshotComplete(Ok(result))
                                },
                                Err(err) => ScreenshotMessage::ScreenshotComplete(Err(err.to_string())),
                            }
                        },
                        |msg| msg,
                    );
                }
                
                // For non-region screenshots, use the regular API
                let delay_ms = self.screenshot_delay_str.parse::<u32>().unwrap_or(0);
                let options = ScreenshotOptions {
                    kind: self.screenshot_kind,
                    delay_ms,
                    save_to_clipboard: false,
                    save_dir: None,
                };
                
                let manager = self.screenshot_manager.clone();
                let backend_name = if self.selected_backend == 0 {
                    None
                } else {
                    Some(self.available_backends[self.selected_backend].clone())
                };
                
                cosmic::Task::perform(
                    async move {
                        println!("About to take screenshot with options: {:?}, backend: {:?}", options.kind, backend_name);
                        match manager.take_screenshot_with_backend(&options, backend_name.as_deref()).await {
                            Ok(result) => ScreenshotMessage::ScreenshotComplete(Ok(result)),
                            Err(err) => ScreenshotMessage::ScreenshotComplete(Err(err.to_string())),
                        }
                    },
                    |msg| msg,
                )
            }
            ScreenshotMessage::SaveScreenshot => {
                if let Some(ref screenshot) = self.last_screenshot {
                    let default_dir = std::path::PathBuf::from(".");
                    let save_dir = self.save_directory.as_ref()
                        .unwrap_or(&default_dir);
                    
                    let filename = format!("Screenshot_{}.png", chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S"));
                    let full_path = save_dir.join(&filename);
                    
                    // For regular screenshots, save full image data
                    // For region-cropped screenshots (path=None), save the cropped thumbnail_data
                    let data_to_save = if screenshot.path.is_some() {
                        // Regular screenshot - use full resolution data
                        &screenshot.full_image_data
                    } else {
                        // Cropped screenshot - thumbnail_data contains the cropped result
                        &screenshot.thumbnail_data
                    };
                    
                    match std::fs::write(&full_path, data_to_save) {
                        Ok(()) => {
                            println!("Screenshot saved as: {}", full_path.display());
                            report_success("Screenshot Saved", &format!("Screenshot saved to {}", full_path.display()));
                        }
                        Err(err) => report_error(ErrorSeverity::Error, "Save Failed", &format!("Failed to save screenshot: {err}")),
                    }
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::ScreenshotComplete(result) => {
                println!("ScreenshotComplete triggered");
                self.screenshot_in_progress = false;
                match result {
                    Ok(screenshot) => {
                        println!("Screenshot succeeded, current kind: {:?}", self.screenshot_kind);
                        // If this was for a rectangular region, launch the snipper
                        if self.screenshot_kind == ScreenshotKind::RectangularRegion {
                            println!("Launching region selection!");
                            return cosmic::Task::perform(
                                async move { ScreenshotMessage::LaunchRegionSelection(screenshot) },
                                |msg| msg,
                            );
                        }
                        self.last_screenshot = Some(screenshot);
                        self.update_thumbnail_cache();
                    }
                    Err(err) => {
                        report_error(ErrorSeverity::Error, "Screenshot Failed", &err);
                    }
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::LaunchRegionSelection(screenshot) => {
                println!("LaunchRegionSelection triggered - opening fullscreen snipper window");
                cosmic::Task::perform(
                    async move { ScreenshotMessage::OpenSnipperWindow(screenshot) },
                    |msg| msg,
                )
            }
            ScreenshotMessage::OpenSnipperWindow(screenshot) => {
                println!("Opening fullscreen snipper window");
                
                // Get actual screenshot dimensions from the FULL image data (not thumbnail!)
                let screen_bounds = if let Ok(img) = image::load_from_memory(&screenshot.full_image_data) {
                    println!("[PERF] Full screenshot dimensions: {}x{}", img.width(), img.height());
                    Rectangle::new(
                        cosmic::iced::Point::ORIGIN, 
                        #[allow(clippy::cast_precision_loss)]
                        cosmic::iced::Size::new(img.width() as f32, img.height() as f32)
                    )
                } else {
                    println!("[PERF] Failed to load full screenshot image, using default dimensions");
                    Rectangle::new(
                        cosmic::iced::Point::ORIGIN, 
                        cosmic::iced::Size::new(1920.0, 1080.0)
                    )
                };
                
                // Create snipper with the full screenshot data
                let mut screen_images = HashMap::new();
                screen_images.insert("primary".to_string(), screenshot.full_image_data.clone());
                
                // Create snipper or update existing one with new screenshot data
                if let Some(ref mut snipper) = self.snipper {
                    println!("[PERF] Updating existing snipper with new screenshot");
                    // Pass remembered selection if enabled
                    let remembered_selection = if self.remember_selection_area {
                        self.last_selection_area
                    } else {
                        None
                    };
                    snipper.update_screenshot_with_memory(screen_images, screen_bounds, remembered_selection);
                } else {
                    println!("[PERF] Creating new snipper - was None");
                    // Use remembered selection if enabled
                    if self.remember_selection_area && self.last_selection_area.is_some() {
                        self.snipper = Some(Snipper::new_with_memory(screen_images, screen_bounds, self.last_selection_area));
                        println!("Created snipper with remembered selection: {:?}", self.last_selection_area);
                    } else {
                        self.snipper = Some(Snipper::new(screen_images, screen_bounds));
                    }
                }
                self.last_screenshot = Some(screenshot);
                
                // Check if we already have a snipper window to reuse
                if let Some(window_id) = self.snipper_window_id {
                    println!("[PERF] Reusing existing snipper window: {window_id:?}");
                    // Show the existing window
                    cosmic::Task::perform(
                        async move { ScreenshotMessage::ShowSnipperWindow },
                        |msg| msg,
                    )
                } else {
                    println!("[PERF] Creating new snipper window");
                    // Create new window
                    let (window_id, open_window) = cosmic::iced::window::open(cosmic::iced::window::Settings {
                        size: cosmic::iced::Size::new(1920.0, 1080.0), // Will be made fullscreen
                        decorations: false,
                        transparent: true,
                        ..Default::default()
                    });
                    
                    // Send SnipperWindowOpened immediately to set up application state
                    // This is separate from OS window events handled by MainWindowOpened
                    open_window.map(move |_| ScreenshotMessage::SnipperWindowOpened(window_id))
                }
            }
            ScreenshotMessage::SnipperWindowOpened(window_id) => {
                // Handle application-level snipper window setup (sent immediately on window creation)
                println!("Snipper window opened: {window_id:?}");
                self.snipper_window_id = Some(window_id);
                self.region_selection_mode = true;
                // Make window fullscreen and maximize
                cosmic::iced::window::maximize(window_id, true)
                    .map(|(): ()| ScreenshotMessage::BackendsLoaded(vec![]))
            }
            ScreenshotMessage::ShowSnipperWindow => {
                if let Some(window_id) = self.snipper_window_id {
                    println!("[PERF] Showing existing snipper window: {window_id:?}");
                    self.region_selection_mode = true;
                    // Show and maximize the window
                    cosmic::Task::batch([
                        cosmic::iced::window::maximize(window_id, true).map(|(): ()| ScreenshotMessage::BackendsLoaded(vec![])),
                        // You could also add window::show() here if the window was completely hidden
                    ])
                } else {
                    println!("[PERF] No snipper window to show");
                    cosmic::Task::none()
                }
            }
            ScreenshotMessage::HideSnipperWindow => {
                if let Some(window_id) = self.snipper_window_id {
                    println!("[PERF] Hiding snipper window: {window_id:?}");
                    self.region_selection_mode = false;
                    // Minimize the window instead of closing it
                    cosmic::iced::window::maximize(window_id, false)
                        .map(|(): ()| ScreenshotMessage::BackendsLoaded(vec![]))
                } else {
                    println!("[PERF] No snipper window to hide");
                    self.region_selection_mode = false;
                    cosmic::Task::none()
                }
            }
            ScreenshotMessage::CloseSnipperWindow => {
                println!("CloseSnipperWindow received - this should be handled by main app");
                // This message should be handled by the main app, not here
                // The main app should close the actual window
                cosmic::Task::none()
            }
            ScreenshotMessage::SnipperWindowClosed(window_id) => {
                println!("Snipper window closed: {window_id:?}");
                self.region_selection_mode = false;
                // Keep the snipper cached for reuse instead of destroying it
                println!("[PERF] NOT destroying snipper - keeping for reuse");
                cosmic::Task::none()
            }
            ScreenshotMessage::RegionSelected(region) => {
                println!("RegionSelected received: {region:?}");
                
                // Remember the selection area if enabled
                if self.remember_selection_area {
                    self.last_selection_area = Some(region);
                    println!("Remembered selection area: {region:?}");
                    // Save to settings
                    if let Err(e) = self.settings_manager.update_selection_area(Some(region)) {
                        report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save selection area: {e}"));
                    }
                }
                
                // Crop the screenshot to the selected region
                if let Some(ref screenshot) = self.last_screenshot {
                    let cropped_screenshot = Self::crop_screenshot_to_region(screenshot, region);
                    match cropped_screenshot {
                        Ok(cropped) => {
                            // In CLI mode, save the screenshot and exit
                            if std::env::var("CLI_MODE_REGION").is_ok() {
                                // Apply CLI options: clipboard and file saving
                                let save_to_clipboard = std::env::var("CLI_CLIPBOARD").is_ok();
                                let output_dir = std::env::var("CLI_OUTPUT_DIR").ok()
                                    .and_then(|s| std::path::PathBuf::from(s).canonicalize().ok())
                                    .or_else(|| self.save_directory.clone())
                                    .or_else(dirs::picture_dir)
                                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                                
                                let filename = format!("screenshot_{}.png", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
                                let full_path = output_dir.join(&filename);
                                
                                // Save to clipboard if requested
                                if save_to_clipboard {
                                    // TODO: Implement clipboard saving
                                    println!("Clipboard saving not yet implemented");
                                }
                                
                                // Save to file
                                match std::fs::write(&full_path, &cropped.thumbnail_data) {
                                    Ok(()) => {
                                        println!("Screenshot saved to: {}", full_path.display());
                                        report_success("Region Screenshot", &format!("Screenshot saved to {}", full_path.display()));
                                        // Exit the application gracefully
                                        return cosmic::Task::perform(async {}, |()| {
                                            ScreenshotMessage::Exit
                                        });
                                    }
                                    Err(err) => {
                                        report_error(ErrorSeverity::Error, "Save Failed", &format!("Failed to save screenshot: {err}"));
                                        return cosmic::Task::perform(async {}, |()| {
                                            ScreenshotMessage::Exit
                                        });
                                    }
                                }
                            }
                            // Regular GUI mode - update UI
                            self.last_screenshot = Some(cropped);
                            self.update_thumbnail_cache();
                            {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let width = region.width as u32;
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let height = region.height as u32;
                                println!("Region cropped successfully: {width}x{height}");
                            }
                            self.region_selection_mode = false;
                            
                            // Keep snipper cached for reuse
                            println!("[PERF] RegionSelected - keeping snipper cached");
                            // Hide the snipper window instead of closing it
                            return cosmic::Task::perform(
                                async move { ScreenshotMessage::HideSnipperWindow },
                                |msg| msg,
                            );
                        }
                        Err(err) => {
                            report_error(ErrorSeverity::Error, "Crop Failed", &format!("Failed to crop screenshot: {err}"));
                            if std::env::var("CLI_MODE_REGION").is_ok() {
                                return cosmic::Task::perform(async {}, |()| {
                                    ScreenshotMessage::Exit
                                });
                            }
                        }
                    }
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::RegionSelectionCancelled => {
                println!("Region selection cancelled - hiding snipper window");
                self.region_selection_mode = false;
                
                // In CLI mode, exit when cancelled
                if std::env::var("CLI_MODE_REGION").is_ok() {
                    println!("CLI mode region selection cancelled, exiting...");
                    return cosmic::Task::perform(async {}, |()| {
                        ScreenshotMessage::Exit
                    });
                }
                
                // Keep snipper cached for reuse
                println!("[PERF] RegionSelectionCancelled - keeping snipper cached");
                cosmic::Task::perform(
                    async move { ScreenshotMessage::HideSnipperWindow },
                    |msg| msg,
                )
            }
            ScreenshotMessage::SnipperMessage(snipper_msg) => {
                if let Some(ref mut snipper) = self.snipper {
                    if let Some(result) = snipper.update(snipper_msg) {
                        match result {
                            SnipperResult::Selected(region) => {
                                println!("Region selected - closing snipper window");
                                return cosmic::Task::perform(
                                    async move { ScreenshotMessage::RegionSelected(region) },
                                    |msg| msg,
                                );
                            }
                            SnipperResult::Cancelled => {
                                println!("Snipper cancelled - closing snipper window");
                                return cosmic::Task::perform(
                                    async move { ScreenshotMessage::RegionSelectionCancelled },
                                    |msg| msg,
                                );
                            }
                        }
                    }
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::OpenSaveDirectoryDialog => {
                println!("Opening save directory dialog");
                // Use COSMIC's native file chooser for directory selection
                cosmic::Task::perform(
                    async move {
                        let dialog = file_chooser::open::Dialog::new()
                            .title("Choose Save Directory");
                        
                        match dialog.open_folder().await {
                            Ok(response) => {
                                // Convert URL to PathBuf
                                if let Ok(path) = response.url().to_file_path() {
                                    ScreenshotMessage::SaveDirectorySelected(Some(path))
                                } else {
                                    ScreenshotMessage::SaveDirectorySelected(None)
                                }
                            }
                            Err(file_chooser::Error::Cancelled) => {
                                println!("Directory selection cancelled");
                                ScreenshotMessage::SaveDirectorySelected(None)
                            }
                            Err(err) => {
                                report_error(ErrorSeverity::Warning, "Directory Selection", &format!("Directory selection error: {err}"));
                                ScreenshotMessage::SaveDirectorySelected(None)
                            }
                        }
                    },
                    |msg| msg,
                )
            }
            ScreenshotMessage::SaveDirectorySelected(path) => {
                println!("Save directory selected: {path:?}");
                self.save_directory.clone_from(&path);
                self.show_path_selection = false;
                // Save to settings
                if let Err(e) = self.settings_manager.update_save_directory(path) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save directory setting: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::ToggleRememberSaveDirectory(remember) => {
                self.remember_save_directory = remember;
                if let Err(e) = self.settings_manager.set_remember_save_directory(remember) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save remember save directory setting: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::ToggleRememberSelectionArea(remember) => {
                self.remember_selection_area = remember;
                if !remember {
                    // Clear remembered selection if disabled
                    self.last_selection_area = None;
                }
                if let Err(e) = self.settings_manager.set_remember_selection_area(remember) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save remember selection area setting: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::ToggleScreenshotOnStartup(enabled) => {
                if let Err(e) = self.settings_manager.set_screenshot_on_startup(enabled) {
                    report_error(ErrorSeverity::Warning, "Settings Error", &format!("Failed to save screenshot on startup setting: {e}"));
                }
                cosmic::Task::none()
            }
            ScreenshotMessage::Exit => {
                // This is handled by the main app, just return none here
                cosmic::Task::none()
            }
            ScreenshotMessage::WindowCloseRequested(_) | ScreenshotMessage::WindowClosed(_) => {
                // Generic window events are handled by the main app
                cosmic::Task::none()
            }
            ScreenshotMessage::DismissErrorDialog => {
                self.error_dialog = None;
                cosmic::Task::none()
            }
            ScreenshotMessage::OpenErrorDialog(title, message) => {
                self.error_dialog = Some((title, message));
                // Open new error dialog window
                cosmic::Task::perform(
                    async move { ScreenshotMessage::ErrorDialogOpened(cosmic::iced::window::Id::unique()) },
                    |msg| msg,
                )
            }
            ScreenshotMessage::ErrorDialogOpened(window_id) => {
                self.error_dialog_window_id = Some(window_id);
                cosmic::Task::none()
            }
            ScreenshotMessage::ErrorDialogClosed(window_id) => {
                if Some(window_id) == self.error_dialog_window_id {
                    self.error_dialog_window_id = None;
                    self.error_dialog = None;
                }
                cosmic::Task::none()
            }
        }
    }
    
    fn get_screenshot_kind_index(&self) -> usize {
        match self.screenshot_kind {
            ScreenshotKind::AllScreens => 0,
            ScreenshotKind::ScreenUnderCursor => 1,
            ScreenshotKind::WindowUnderCursor => 2,
            ScreenshotKind::SelectScreen => 3,
            ScreenshotKind::RectangularRegion => 4,
        }
    }
    
    fn crop_screenshot_to_region(screenshot: &ScreenshotResult, region: Rectangle) -> Result<ScreenshotResult, ScreenshotError> {
        // Load the image from full resolution data for accurate cropping
        let img = image::load_from_memory(&screenshot.full_image_data)
            .map_err(ScreenshotError::Image)?;
        
        // Ensure region is within image bounds
        #[allow(clippy::cast_precision_loss)]
        let img_width = img.width() as f32;
        #[allow(clippy::cast_precision_loss)]
        let img_height = img.height() as f32;
        
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let crop_x = region.x.max(0.0).min(img_width) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let crop_y = region.y.max(0.0).min(img_height) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let crop_width = region.width.min(img_width - crop_x as f32) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let crop_height = region.height.min(img_height - crop_y as f32) as u32;
        
        if crop_width == 0 || crop_height == 0 {
            return Err(ScreenshotError::Portal("Invalid crop region".to_string()));
        }
        
        // Crop the image
        let cropped = img.crop_imm(crop_x, crop_y, crop_width, crop_height);
        
        // Convert back to bytes
        let mut buffer = Vec::new();
        cropped.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
            .map_err(ScreenshotError::Image)?;
        
        // Create new screenshot result with cropped data
        Ok(ScreenshotResult {
            path: None, // Remove path so saving uses the cropped thumbnail_data
            saved_to_clipboard: screenshot.saved_to_clipboard,
            thumbnail_data: buffer.clone(),
            full_image_data: buffer, // For cropped result, full and thumbnail data are the same
        })
    }
    
    #[allow(clippy::too_many_lines)]
    pub fn view(&self) -> cosmic::Element<'_, ScreenshotMessage> {
        // Use COSMIC theme spacing for consistency
        let spacing = cosmic::theme::active().cosmic().spacing;
        
        // Header section
        let header = widget::container(
            cosmic::widget::text::title4("COSMIC Screenshot")
        )
        .padding(spacing.space_m);
        
        // Thumbnail preview section - adaptive sizing for 360p thumbnails
        let thumbnail_section = if let Some(ref image_handle) = self.cached_thumbnail_handle {
            widget::container(
                cosmic::widget::image(image_handle.clone())
                    .content_fit(cosmic::iced::ContentFit::ScaleDown)
            )
            .width(cosmic::iced::Length::Fixed(640.0))
            .height(cosmic::iced::Length::Fixed(360.0))
            .padding(2) // Add padding to keep image away from border
            .style(|theme: &cosmic::Theme| cosmic::widget::container::Style {
                border: cosmic::iced::Border {
                    width: 1.0,
                    color: theme.cosmic().accent.base.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
        } else {
            widget::container(
                widget::column()
                    .push(widget::text("No screenshot"))
                    .push(widget::text("taken yet"))
                    .align_x(cosmic::iced::Alignment::Center)
                    .spacing(spacing.space_xxs)
            )
            .width(cosmic::iced::Length::Fixed(640.0))
            .height(cosmic::iced::Length::Fixed(360.0))
            .padding(2) // Add padding to keep text away from border
            .center_x(cosmic::iced::Length::Fill)
            .center_y(cosmic::iced::Length::Fill)
            .style(|theme: &cosmic::Theme| cosmic::widget::container::Style {
                border: cosmic::iced::Border {
                    width: 1.0,
                    color: theme.cosmic().bg_divider().into(),
                    ..Default::default()
                },
                ..Default::default()
            })
        };
        
        // Controls section - improved layout
        let controls_section = widget::column()
            .push(
                cosmic::widget::text::caption("Screenshot Type:")
            )
            .push(
                widget::dropdown(&self.screenshot_options, Some(self.get_screenshot_kind_index()), |index| {
                    match index {
                        1 => ScreenshotMessage::SetScreenshotKind(ScreenshotKind::ScreenUnderCursor),
                        2 => ScreenshotMessage::SetScreenshotKind(ScreenshotKind::WindowUnderCursor),
                        3 => ScreenshotMessage::SetScreenshotKind(ScreenshotKind::SelectScreen),
                        4 => ScreenshotMessage::SetScreenshotKind(ScreenshotKind::RectangularRegion),
                        _ => ScreenshotMessage::SetScreenshotKind(ScreenshotKind::AllScreens),
                    }
                })
                .width(cosmic::iced::Length::Fixed(250.0))
            )
            .push(
                cosmic::widget::text::caption("Delay (milliseconds):")
            )
            .push(
                widget::text_input("0", &self.screenshot_delay_str)
                    .on_input(ScreenshotMessage::SetScreenshotDelay)
                    .width(cosmic::iced::Length::Fixed(120.0))
            )
            .push(
                cosmic::widget::text::caption("Backend:")
            )
            .push(
                widget::dropdown(&self.available_backends, Some(self.selected_backend), ScreenshotMessage::SetScreenshotBackend)
                    .width(cosmic::iced::Length::Fixed(250.0))
            )
            .push(
                cosmic::widget::text::caption("Save Directory:")
            )
            .push(
                widget::row()
                    .push(
                        cosmic::widget::text::body(
                            self.save_directory
                                .as_ref().map_or_else(|| "No directory selected".to_string(), |p| p.display().to_string())
                        )
                        .width(cosmic::iced::Length::Fill)
                    )
                    .push(
                        widget::button::standard("Browse...")
                            .on_press(ScreenshotMessage::OpenSaveDirectoryDialog)
                    )
                    .spacing(spacing.space_xs)
                    .width(cosmic::iced::Length::Fixed(250.0))
            )
            .push(
                widget::checkbox("Remember save directory", self.remember_save_directory)
                    .on_toggle(ScreenshotMessage::ToggleRememberSaveDirectory)
            )
            .push(
                widget::checkbox("Remember selection area", self.remember_selection_area)
                    .on_toggle(ScreenshotMessage::ToggleRememberSelectionArea)
            )
            .push(
                widget::checkbox("Take screenshot on startup", self.settings_manager.settings.screenshot_on_startup)
                    .on_toggle(ScreenshotMessage::ToggleScreenshotOnStartup)
            )
            .spacing(spacing.space_xs);
        
        // Action buttons section
        let actions_section = widget::column()
            .push(
                widget::button::standard(if self.screenshot_in_progress {
                    "Taking Screenshot..."
                } else {
                    "Take Screenshot"
                })
                .on_press_maybe(if self.screenshot_in_progress { 
                    None 
                } else { 
                    Some(ScreenshotMessage::TakeScreenshot) 
                })
            )
            .push_maybe(if self.last_screenshot.is_some() {
                Some(
                    widget::button::standard("Save Screenshot")
                        .on_press(ScreenshotMessage::SaveScreenshot)
                )
            } else {
                None
            })
            .spacing(spacing.space_xs);
        
        // Main content layout - following szhrmk's row-based layout
        let main_content = widget::row()
            .push(thumbnail_section)
            .push(
                widget::column()
                    .push(controls_section)
                    .push(actions_section)
                    .spacing(spacing.space_m)
                    .padding(spacing.space_s)
            )
            .spacing(spacing.space_m)
            .align_y(cosmic::iced::Alignment::Start);
        
        // Complete layout with proper COSMIC styling
        widget::column()
            .push(header)
            .push(main_content)
            .spacing(spacing.space_s)
            .padding(spacing.space_m)
            .into()
    }
}