// SPDX-License-Identifier: GPL-3.0-only

//! Universal error handling for cosmic-screenshot
//! 
//! This module provides centralized error reporting that adapts to the application mode:
//! - GUI mode: Shows error dialogs and warning notifications
//! - CLI/D-Bus mode: Uses eprintln! for console output
//! - Service mode: Uses structured logging

use std::sync::atomic::{AtomicBool, Ordering};
use crate::notifications::{show_system_notification, notifications_available, NotificationType};

/// Global flag to track if we're running in GUI mode
static GUI_MODE: AtomicBool = AtomicBool::new(false);

/// Error severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    /// Critical errors that prevent the application from functioning
    Error,
    /// Warnings about fallbacks or degraded functionality
    Warning,
    /// Informational messages about state changes
    Info,
}

/// Set whether the application is running in GUI mode
pub fn set_gui_mode(enabled: bool) {
    GUI_MODE.store(enabled, Ordering::Relaxed);
}

/// Check if the application is running in GUI mode
pub fn is_gui_mode() -> bool {
    GUI_MODE.load(Ordering::Relaxed)
}

/// Channel for sending GUI error messages
static GUI_ERROR_SENDER: std::sync::OnceLock<std::sync::mpsc::Sender<(ErrorSeverity, String, String)>> = std::sync::OnceLock::new();

/// Set up GUI error message channel
pub fn setup_gui_channel() -> std::sync::mpsc::Receiver<(ErrorSeverity, String, String)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let _ = GUI_ERROR_SENDER.set(sender);
    receiver
}

/// Universal error reporting function
/// 
/// This function handles error reporting across different application modes:
/// - In GUI mode, it sends messages to show error dialogs or notifications
/// - In CLI/service mode, it uses eprintln! for console output
pub fn report_error(severity: ErrorSeverity, title: &str, message: &str) {
    if is_gui_mode() {
        match severity {
            ErrorSeverity::Error => {
                // Errors should show dialogs in GUI mode
                if let Some(sender) = GUI_ERROR_SENDER.get() {
                    let _ = sender.send((severity.clone(), title.to_string(), message.to_string()));
                } else {
                    eprintln!("ERROR: {title}: {message}");
                }
            }
            ErrorSeverity::Warning | ErrorSeverity::Info => {
                // Warnings and info should use system notifications
                let notification_type = match severity {
                    ErrorSeverity::Warning => NotificationType::Warning,
                    ErrorSeverity::Info => NotificationType::Info,
                    ErrorSeverity::Error => unreachable!(),
                };
                
                // Try to show system notification
                let title_clone = title.to_string();
                let message_clone = message.to_string();
                tokio::spawn(async move {
                    // First check if notifications are available
                    if notifications_available().await {
                        match show_system_notification(notification_type, &title_clone, &message_clone).await {
                            Ok(_) => {
                                // Notification shown successfully
                                return;
                            }
                            Err(e) => {
                                // Log the notification error but continue to fallback
                                eprintln!("Notification failed: {e}");
                            }
                        }
                    }
                    
                    // Fall back to console if notifications aren't available or fail
                    match severity {
                        ErrorSeverity::Warning => {
                            eprintln!("WARNING: {title_clone}: {message_clone}");
                        }
                        ErrorSeverity::Info => {
                            eprintln!("INFO: {title_clone}: {message_clone}");
                        }
                        ErrorSeverity::Error => unreachable!(),
                    }
                });
            }
        }
    } else {
        // In CLI/service mode, use standard error output
        match severity {
            ErrorSeverity::Error => {
                eprintln!("ERROR: {title}: {message}");
            }
            ErrorSeverity::Warning => {
                eprintln!("WARNING: {title}: {message}");
            }
            ErrorSeverity::Info => {
                eprintln!("INFO: {title}: {message}");
            }
        }
    }
}

/// Convenience macros for common error reporting patterns
#[macro_export]
macro_rules! report_error {
    ($title:expr, $msg:expr) => {
        $crate::error_handling::report_error(
            $crate::error_handling::ErrorSeverity::Error,
            $title,
            $msg,
        )
    };
}

#[macro_export]
macro_rules! report_warning {
    ($title:expr, $msg:expr) => {
        $crate::error_handling::report_error(
            $crate::error_handling::ErrorSeverity::Warning,
            $title,
            $msg,
        )
    };
}

#[macro_export]
macro_rules! report_info {
    ($title:expr, $msg:expr) => {
        $crate::error_handling::report_error(
            $crate::error_handling::ErrorSeverity::Info,
            $title,
            $msg,
        )
    };
}

/// Format error for display (used in GUI dialogs)
#[must_use] 
pub fn format_error_message(title: &str, message: &str) -> String {
    format!("{title}\n\n{message}")
}

/// Check if an error should trigger a dialog in GUI mode
#[must_use] 
pub fn should_show_dialog(severity: &ErrorSeverity) -> bool {
    match severity {
        ErrorSeverity::Error => true,
        ErrorSeverity::Warning | ErrorSeverity::Info => false, // Warnings should use notifications
    }
}

/// Show a success notification (convenience function)
pub fn report_success(title: &str, message: &str) {
    if is_gui_mode() {
        let notification_type = NotificationType::Success;
        let title_clone = title.to_string();
        let message_clone = message.to_string();
        
        tokio::spawn(async move {
            match show_system_notification(notification_type, &title_clone, &message_clone).await {
                Ok(_) => {
                    // Success notification shown
                }
                Err(_) => {
                    // Fall back to console for success messages
                    println!("SUCCESS: {title_clone}: {message_clone}");
                }
            }
        });
    } else {
        println!("SUCCESS: {title}: {message}");
    }
}

/// Check if system notifications are available and working
pub async fn check_notification_support() -> bool {
    notifications_available().await
}