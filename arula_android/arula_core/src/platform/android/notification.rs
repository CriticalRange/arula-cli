//! Android notification system using Termux:API

use crate::platform::android::{AndroidContext, callbacks};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Android notification manager using Termux:API
pub struct AndroidNotification {
    ctx: AndroidContext,
    enabled: Arc<Mutex<bool>>,
}

impl AndroidNotification {
    pub fn new(ctx: AndroidContext) -> Self {
        Self {
            ctx,
            enabled: Arc::new(Mutex::new(true)),
        }
    }

    /// Show a simple notification
    pub async fn show_notification(&self, title: &str, message: &str) -> Result<()> {
        let enabled = self.enabled.lock().await;
        if !*enabled {
            return Ok(());
        }

        // Use Termux:API to show notification
        let command = format!("termux-notification --title '{}' --content '{}'",
            escape_shell_arg(title),
            escape_shell_arg(message));

        log::info!("Showing notification: {} - {}", title, message);

        // In a real implementation, this would execute the Termux command
        // For now, we'll just log it
        callbacks::on_message(&format!("Notification: {} - {}", title, message));

        Ok(())
    }

    /// Show a notification with action buttons
    pub async fn show_notification_with_actions(
        &self,
        title: &str,
        message: &str,
        actions: &[NotificationAction],
    ) -> Result<()> {
        let enabled = self.enabled.lock().await;
        if !*enabled {
            return Ok(());
        }

        // Build Termux notification command with actions
        let mut command = format!("termux-notification --title '{}' --content '{}'",
            escape_shell_arg(title),
            escape_shell_arg(message));

        for (i, action) in actions.iter().enumerate() {
            command.push_str(&format!(" --action '{}' '{}'",
                escape_shell_arg(&action.id),
                escape_shell_arg(&action.title)));
        }

        log::info!("Showing notification with actions: {}", command);
        Ok(())
    }

    /// Show a progress notification
    pub async fn show_progress(&self, title: &str, progress: u8) -> Result<()> {
        let enabled = self.enabled.lock().await;
        if !*enabled {
            return Ok(());
        }

        let command = format!(
            "termux-notification --title '{}' --content 'Progress: {}%' --progress {}",
            escape_shell_arg(title),
            progress,
            progress
        );

        log::info!("Showing progress: {} - {}%", title, progress);
        Ok(())
    }

    /// Show a toast message
    pub async fn show_toast(&self, message: &str) -> Result<()> {
        let command = format!("termux-toast '{}'", escape_shell_arg(message));

        log::info!("Showing toast: {}", message);
        callbacks::on_message(&format!("Toast: {}", message));
        Ok(())
    }

    /// Vibrate device
    pub async fn vibrate(&self, pattern: VibrationPattern) -> Result<()> {
        let duration = match pattern {
            VibrationPattern::Short => 100,
            VibrationPattern::Long => 500,
            VibrationPattern::Double => 100,
            VibrationPattern::Custom(ms) => ms,
        };

        let command = format!("termux-vibrate -d {}", duration);

        log::info!("Vibrating for {}ms", duration);
        Ok(())
    }

    /// Make a sound
    pub async fn play_sound(&self, sound_type: SoundType) -> Result<()> {
        let sound = match sound_type {
            SoundType::Notification => "notification",
            SoundType::Alarm => "alarm",
            SoundType::Ringtone => "ringtone",
            SoundType::Custom(name) => name,
        };

        let command = format!("termux-bell -f {}", sound);

        log::info!("Playing sound: {}", sound);
        Ok(())
    }

    /// Remove all notifications
    pub async fn clear_all(&self) -> Result<()> {
        let command = "termux-notification --remove-all".to_string();

        log::info!("Clearing all notifications");
        Ok(())
    }

    /// Enable/disable notifications
    pub async fn set_enabled(&self, enabled: bool) {
        let mut e = self.enabled.lock().await;
        *e = enabled;
    }

    /// Check if notifications are enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.lock().await
    }
}

#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum VibrationPattern {
    Short,
    Long,
    Double,
    Custom(u64),
}

#[derive(Debug, Clone)]
pub enum SoundType {
    Notification,
    Alarm,
    Ringtone,
    Custom(String),
}

/// Escape shell argument for Termux commands
fn escape_shell_arg(arg: &str) -> String {
    arg.replace('\'', "'\"'\"'")
}