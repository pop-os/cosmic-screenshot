// SPDX-License-Identifier: GPL-3.0-only

//! System notification support using freedesktop notification standard

use zbus::{proxy, Connection};
use std::collections::HashMap;

/// Notification urgency levels
#[derive(Debug, Clone, Copy)]
pub enum NotificationUrgency {
    Low = 0,
    Normal = 1,
    Critical = 2,
}

/// Notification types with appropriate urgency and icons
#[derive(Debug, Clone, Copy)]
pub enum NotificationType {
    Info,
    Warning,
    Error,
    Success,
}

impl NotificationType {
    #[must_use] 
    pub fn urgency(&self) -> NotificationUrgency {
        match self {
            NotificationType::Info | NotificationType::Success => NotificationUrgency::Low,
            NotificationType::Warning => NotificationUrgency::Normal,
            NotificationType::Error => NotificationUrgency::Critical,
        }
    }
    
    #[must_use] 
    pub fn icon(&self) -> &'static str {
        match self {
            NotificationType::Info => "dialog-information",
            NotificationType::Success => "emblem-default",
            NotificationType::Warning => "dialog-warning",
            NotificationType::Error => "dialog-error",
        }
    }
}

/// Freedesktop Notifications D-Bus proxy
#[allow(clippy::too_many_arguments)]
#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    /// Show a notification
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
    
    /// Close a notification
    fn close_notification(&self, id: u32) -> zbus::Result<()>;
    
    /// Get server capabilities
    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;
    
    /// Get server information
    fn get_server_information(&self) -> zbus::Result<(String, String, String, String)>;
}

/// System notification manager
pub struct NotificationManager {
    connection: Option<Connection>,
}

impl NotificationManager {
    /// Create a new notification manager
    pub async fn new() -> Self {
        let connection = Connection::session().await.ok();
        Self { connection }
    }
    
    /// Check if system notifications are available
    #[must_use] 
    pub fn is_available(&self) -> bool {
        self.connection.is_some()
    }
    
    /// Show a system notification
    #[allow(clippy::missing_errors_doc)]
    pub async fn show_notification(
        &self,
        notification_type: NotificationType,
        title: &str,
        message: &str,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        if let Some(ref connection) = self.connection {
            let proxy = NotificationsProxy::new(connection).await?;
            
            let mut hints = HashMap::new();
            hints.insert("urgency", zbus::zvariant::Value::U8(notification_type.urgency() as u8));
            
            let notification_id = proxy.notify(
                "COSMIC Screenshot", // app_name
                0, // replaces_id (0 for new notification)
                notification_type.icon(), // app_icon
                title, // summary
                message, // body
                vec![], // actions (empty for simple notifications)
                hints, // hints
                5000, // expire_timeout (5 seconds)
            ).await?;
            
            Ok(notification_id)
        } else {
            Err("No D-Bus connection available for notifications".into())
        }
    }
    
    /// Close a notification by ID
    #[allow(clippy::missing_errors_doc)]
    pub async fn close_notification(&self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref connection) = self.connection {
            let proxy = NotificationsProxy::new(connection).await?;
            proxy.close_notification(id).await?;
            Ok(())
        } else {
            Err("No D-Bus connection available for notifications".into())
        }
    }
    
    /// Get notification server capabilities
    #[allow(clippy::missing_errors_doc)]
    pub async fn get_capabilities(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if let Some(ref connection) = self.connection {
            let proxy = NotificationsProxy::new(connection).await?;
            let capabilities = proxy.get_capabilities().await?;
            Ok(capabilities)
        } else {
            Err("No D-Bus connection available for notifications".into())
        }
    }
}

/// Global notification manager instance
static NOTIFICATION_MANAGER: std::sync::OnceLock<tokio::sync::Mutex<NotificationManager>> = std::sync::OnceLock::new();

/// Initialize the global notification manager
pub async fn init_notification_manager() {
    let manager = NotificationManager::new().await;
    let _ = NOTIFICATION_MANAGER.set(tokio::sync::Mutex::new(manager));
}

/// Show a system notification (convenience function)
#[allow(clippy::missing_errors_doc)]
pub async fn show_system_notification(
    notification_type: NotificationType,
    title: &str,
    message: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    if let Some(manager_mutex) = NOTIFICATION_MANAGER.get() {
        let manager = manager_mutex.lock().await;
        manager.show_notification(notification_type, title, message).await
    } else {
        Err("Notification manager not initialized".into())
    }
}

/// Check if system notifications are available (convenience function)
pub async fn notifications_available() -> bool {
    if let Some(manager_mutex) = NOTIFICATION_MANAGER.get() {
        let manager = manager_mutex.lock().await;
        manager.is_available()
    } else {
        false
    }
}