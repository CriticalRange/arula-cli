//! Termux:API integration for Android-specific features

use crate::platform::android::{AndroidContext, AndroidCommandExecutor, AndroidNotification};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Termux:API wrapper providing access to Android features
pub struct TermuxApi {
    ctx: AndroidContext,
    command_executor: Arc<AndroidCommandExecutor>,
    notification: Arc<AndroidNotification>,
}

impl TermuxApi {
    pub fn new(ctx: AndroidContext) -> Self {
        let command_executor = Arc::new(AndroidCommandExecutor::new(ctx.clone()));
        let notification = Arc::new(AndroidNotification::new(ctx.clone()));

        Self {
            ctx,
            command_executor,
            notification,
        }
    }

    // Battery Information
    pub async fn get_battery_info(&self) -> Result<BatteryInfo> {
        let output = self.command_executor
            .execute_termux_api("battery-status", &[])
            .await?;

        // Parse battery status JSON
        let info: BatteryInfo = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse battery info: {}", e))?;

        Ok(info)
    }

    // Location Services
    pub async fn get_location(&self) -> Result<LocationInfo> {
        let output = self.command_executor
            .execute_termux_api("location", &["-p", "gps"])
            .await?;

        let info: LocationInfo = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse location: {}", e))?;

        Ok(info)
    }

    // Camera Information
    pub async fn get_camera_info(&self) -> Result<Vec<CameraInfo>> {
        let output = self.command_executor
            .execute_termux_api("camera-info", &[])
            .await?;

        let cameras: Vec<CameraInfo> = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse camera info: {}", e))?;

        Ok(cameras)
    }

    // Take Photo
    pub async fn take_photo(&self, camera_id: &str, output_file: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("camera-photo", &["-c", camera_id, output_file])
            .await?;

        Ok(())
    }

    // Microphone Recording
    pub async fn start_recording(&self, output_file: &str, limit_seconds: Option<u32>) -> Result<()> {
        let mut args = vec!["-f", output_file];
        if let Some(limit) = limit_seconds {
            args.push("-l");
            args.push(&limit.to_string());
        }

        self.command_executor
            .execute_termux_api("microphone-record", &args)
            .await?;

        Ok(())
    }

    pub async fn stop_recording(&self) -> Result<()> {
        self.command_executor
            .execute_termux_api("microphone-record", &["-q"])
            .await?;

        Ok(())
    }

    // SMS Functions
    pub async fn list_sms(&self, limit: Option<u32>) -> Result<Vec<SmsMessage>> {
        let mut args = vec!["-l"];
        if let Some(limit) = limit {
            args.push(&limit.to_string());
        }

        let output = self.command_executor
            .execute_termux_api("sms-list", &args)
            .await?;

        let messages: Vec<SmsMessage> = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse SMS list: {}", e))?;

        Ok(messages)
    }

    pub async fn send_sms(&self, number: &str, message: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("sms-send", &["-n", number, message])
            .await?;

        Ok(())
    }

    // Telephony
    pub async fn make_call(&self, number: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("telephony-call", &["-n", number])
            .await?;

        Ok(())
    }

    pub async fn get_call_log(&self, limit: Option<u32>) -> Result<Vec<CallLogEntry>> {
        let mut args = vec!["-l"];
        if let Some(limit) = limit {
            args.push(&limit.to_string());
        }

        let output = self.command_executor
            .execute_termux_api("telephony-calllog", &args)
            .await?;

        let entries: Vec<CallLogEntry> = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse call log: {}", e))?;

        Ok(entries)
    }

    // WiFi Information
    pub async fn get_wifi_info(&self) -> Result<WifiInfo> {
        let output = self.command_executor
            .execute_termux_api("wifi-connectioninfo", &[])
            .await?;

        let info: WifiInfo = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse WiFi info: {}", e))?;

        Ok(info)
    }

    // Sensor Information
    pub async fn get_sensor_info(&self, sensor_type: &str) -> Result<SensorData> {
        let output = self.command_executor
            .execute_termux_api("sensor", &["-s", sensor_type])
            .await?;

        let data: SensorData = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse sensor data: {}", e))?;

        Ok(data)
    }

    pub async fn list_sensors(&self) -> Result<Vec<SensorInfo>> {
        let output = self.command_executor
            .execute_termux_api("sensor", &["-l"])
            .await?;

        let sensors: Vec<SensorInfo> = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse sensor list: {}", e))?;

        Ok(sensors)
    }

    // Clipboard
    pub async fn set_clipboard(&self, text: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("clipboard-set", &[text])
            .await?;

        Ok(())
    }

    pub async fn get_clipboard(&self) -> Result<String> {
        let output = self.command_executor
            .execute_termux_api("clipboard-get", &[])
            .await?;

        Ok(output.trim().to_string())
    }

    // Share
    pub async fn share_text(&self, title: &str, text: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("share", &[title, text])
            .await?;

        Ok(())
    }

    pub async fn share_file(&self, title: &str, file_path: &str) -> Result<()> {
        self.command_executor
            .execute_termux_api("share", &[title, "-a", "file", file_path])
            .await?;

        Ok(())
    }

    // Dialog
    pub async fn show_dialog(&self, dialog: DialogOptions) -> Result<DialogResult> {
        let mut args = vec!["-t", &dialog.title, "-i", &dialog.message];

        if let Some(ok) = dialog.ok_button {
            args.push("-p");
            args.push(&ok);
        }

        if let Some(cancel) = dialog.cancel_button {
            args.push("-n");
            args.push(&cancel);
        }

        let output = self.command_executor
            .execute_termux_api("dialog", &args)
            .await?;

        let result = if output.trim() == "yes" {
            DialogResult::Ok
        } else {
            DialogResult::Cancel
        };

        Ok(result)
    }

    // Spinner
    pub async fn show_spinner(&self, title: &str) -> Result<SpinnerHandle> {
        self.command_executor
            .execute_termux_api("spinner", &["start", title])
            .await?;

        Ok(SpinnerHandle {
            command_executor: self.command_executor.clone(),
        })
    }

    // Volume
    pub async fn get_volume(&self, stream: VolumeStream) -> Result<u8> {
        let stream_name = match stream {
            VolumeStream::Music => "music",
            VolumeStream::System => "system",
            VolumeStream::Ring => "ring",
            VolumeStream::Alarm => "alarm",
            VolumeStream::Notification => "notification",
        };

        let output = self.command_executor
            .execute_termux_api("volume", &[stream_name])
            .await?;

        output.trim().parse::<u8>()
            .map_err(|e| anyhow::anyhow!("Failed to parse volume: {}", e))
    }

    pub async fn set_volume(&self, stream: VolumeStream, volume: u8) -> Result<()> {
        let stream_name = match stream {
            VolumeStream::Music => "music",
            VolumeStream::System => "system",
            VolumeStream::Ring => "ring",
            VolumeStream::Alarm => "alarm",
            VolumeStream::Notification => "notification",
        };

        self.command_executor
            .execute_termux_api("volume", &[stream_name, &volume.to_string()])
            .await?;

        Ok(())
    }

    // Brightness
    pub async fn get_brightness(&self) -> Result<u8> {
        let output = self.command_executor
            .execute_termux_api("brightness", &[])
            .await?;

        output.trim().parse::<u8>()
            .map_err(|e| anyhow::anyhow!("Failed to parse brightness: {}", e))
    }

    pub async fn set_brightness(&self, brightness: u8) -> Result<()> {
        self.command_executor
            .execute_termux_api("brightness", &[&brightness.to_string()])
            .await?;

        Ok(())
    }
}

// Data structures for Termux:API responses

#[derive(Debug, Deserialize, Serialize)]
pub struct BatteryInfo {
    pub percentage: u8,
    pub status: String, // charging, discharging, full, etc.
    pub health: String,
    pub power_source: String,
    pub temperature: Option<f32>,
    pub voltage: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LocationInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: Option<f32>,
    pub bearing: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub facing: String, // back, front, external
    pub focal_lengths: Vec<f32>,
    pub jpeg_output_sizes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SmsMessage {
    pub number: String,
    pub text: String,
    pub received_date: String,
    pub type_: String, // inbox, sent, etc.
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CallLogEntry {
    pub number: String,
    pub name: Option<String>,
    pub type_: String, // incoming, outgoing, missed
    pub date: String,
    pub duration: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WifiInfo {
    pub ssid: String,
    pub bssid: String,
    pub ip: String,
    pub id: String,
    pub frequency: u32,
    pub link_speed: u32,
    pub mac_address: String,
    pub rssi: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SensorData {
    pub sensor_type: String,
    pub values: Vec<f32>,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SensorInfo {
    pub name: String,
    pub type_: String,
    pub vendor: String,
    pub maximum_range: f32,
    pub resolution: f32,
    pub power: f32,
}

#[derive(Debug, Clone)]
pub struct DialogOptions {
    pub title: String,
    pub message: String,
    pub ok_button: Option<String>,
    pub cancel_button: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DialogResult {
    Ok,
    Cancel,
}

pub struct SpinnerHandle {
    command_executor: Arc<AndroidCommandExecutor>,
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        let executor = self.command_executor.clone();
        tokio::spawn(async move {
            let _ = executor.execute_termux_api("spinner", &["stop"]).await;
        });
    }
}

pub enum VolumeStream {
    Music,
    System,
    Ring,
    Alarm,
    Notification,
}