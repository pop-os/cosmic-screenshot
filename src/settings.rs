// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Rectangle;
use cosmic_config::{Config, CosmicConfigEntry, ConfigGet, ConfigSet};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_ID: &str = "com.system76.CosmicScreenshot";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotSettings {
    /// Whether to take screenshot immediately on startup using last settings
    pub screenshot_on_startup: bool,
    /// Last used screenshot kind (All screens, Current screen, etc.)
    pub last_screenshot_kind: String,
    /// Last used screenshot delay in seconds
    pub last_screenshot_delay: u32,
    /// Last used backend index
    pub last_selected_backend: usize,
    /// Whether to remember the save directory
    pub remember_save_directory: bool,
    /// Last used save directory
    pub last_save_directory: Option<PathBuf>,
    /// Whether to remember selection area across sessions
    pub remember_selection_area: bool,
    /// Last selection rectangle (for region screenshots)
    pub last_selection_area: Option<SelectionArea>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rectangle> for SelectionArea {
    fn from(rect: Rectangle) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl From<SelectionArea> for Rectangle {
    fn from(area: SelectionArea) -> Self {
        Rectangle {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        }
    }
}

impl Default for ScreenshotSettings {
    fn default() -> Self {
        Self {
            screenshot_on_startup: false,
            last_screenshot_kind: "All screens".to_string(),
            last_screenshot_delay: 0,
            last_selected_backend: 0,
            remember_save_directory: true,
            last_save_directory: dirs::picture_dir(),
            remember_selection_area: false,
            last_selection_area: None,
        }
    }
}

impl CosmicConfigEntry for ScreenshotSettings {
    const VERSION: u64 = 1;

    fn write_entry(&self, config: &Config) -> Result<(), cosmic_config::Error> {
        config.set("screenshot_on_startup", self.screenshot_on_startup)?;
        config.set("last_screenshot_kind", &self.last_screenshot_kind)?;
        config.set("last_screenshot_delay", self.last_screenshot_delay)?;
        config.set("last_selected_backend", self.last_selected_backend)?;
        config.set("remember_save_directory", self.remember_save_directory)?;
        config.set("last_save_directory", &self.last_save_directory)?;
        config.set("remember_selection_area", self.remember_selection_area)?;
        config.set("last_selection_area", &self.last_selection_area)?;
        Ok(())
    }

    fn get_entry(config: &Config) -> Result<Self, (Vec<cosmic_config::Error>, Self)> {
        let mut errors = Vec::new();
        let default = Self::default();

        let screenshot_on_startup = config.get("screenshot_on_startup")
            .unwrap_or_else(|e| { errors.push(e); default.screenshot_on_startup });
        
        let last_screenshot_kind = config.get("last_screenshot_kind")
            .unwrap_or_else(|e| { errors.push(e); default.last_screenshot_kind.clone() });
        
        let last_screenshot_delay = config.get("last_screenshot_delay")
            .unwrap_or_else(|e| { errors.push(e); default.last_screenshot_delay });
        
        let last_selected_backend = config.get("last_selected_backend")
            .unwrap_or_else(|e| { errors.push(e); default.last_selected_backend });
        
        let remember_save_directory = config.get("remember_save_directory")
            .unwrap_or_else(|e| { errors.push(e); default.remember_save_directory });
        
        let last_save_directory = config.get("last_save_directory")
            .unwrap_or_else(|e| { errors.push(e); default.last_save_directory.clone() });
        
        let remember_selection_area = config.get("remember_selection_area")
            .unwrap_or_else(|e| { errors.push(e); default.remember_selection_area });
        
        let last_selection_area = config.get("last_selection_area")
            .unwrap_or_else(|e| { errors.push(e); default.last_selection_area.clone() });

        let settings = Self {
            screenshot_on_startup,
            last_screenshot_kind,
            last_screenshot_delay,
            last_selected_backend,
            remember_save_directory,
            last_save_directory,
            remember_selection_area,
            last_selection_area,
        };

        if errors.is_empty() {
            Ok(settings)
        } else {
            Err((errors, settings))
        }
    }

    fn update_keys<T>(&mut self, config: &Config, _keys: &[T]) -> (Vec<cosmic_config::Error>, Vec<&'static str>)
    where
        T: AsRef<str>
    {
        // For simple config updates, we just reload all settings
        match Self::get_entry(config) {
            Ok(new_settings) => {
                *self = new_settings;
                (vec![], vec![])
            }
            Err((errors, new_settings)) => {
                *self = new_settings;
                (errors, vec![])
            }
        }
    }
}

pub struct SettingsManager {
    pub config: Config,
    pub settings: ScreenshotSettings,
}

impl SettingsManager {
    #[allow(clippy::missing_errors_doc)]
    pub fn new() -> Result<Self, cosmic_config::Error> {
        let config = Config::new(APP_ID, ScreenshotSettings::VERSION)?;
        let settings = ScreenshotSettings::get_entry(&config).unwrap_or_default();
        
        Ok(Self { config, settings })
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn save(&self) -> Result<(), cosmic_config::Error> {
        self.settings.write_entry(&self.config)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn update_screenshot_settings(
        &mut self,
        kind: &str,
        delay: u32,
        backend_index: usize,
    ) -> Result<(), cosmic_config::Error> {
        self.settings.last_screenshot_kind = kind.to_string();
        self.settings.last_screenshot_delay = delay;
        self.settings.last_selected_backend = backend_index;
        self.save()
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn update_save_directory(
        &mut self,
        directory: Option<PathBuf>,
    ) -> Result<(), cosmic_config::Error> {
        self.settings.last_save_directory = directory;
        self.save()
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn update_selection_area(
        &mut self,
        area: Option<Rectangle>,
    ) -> Result<(), cosmic_config::Error> {
        self.settings.last_selection_area = area.map(SelectionArea::from);
        self.save()
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn set_screenshot_on_startup(&mut self, enabled: bool) -> Result<(), cosmic_config::Error> {
        self.settings.screenshot_on_startup = enabled;
        self.save()
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn set_remember_save_directory(&mut self, remember: bool) -> Result<(), cosmic_config::Error> {
        self.settings.remember_save_directory = remember;
        self.save()
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn set_remember_selection_area(&mut self, remember: bool) -> Result<(), cosmic_config::Error> {
        self.settings.remember_selection_area = remember;
        self.save()
    }
}