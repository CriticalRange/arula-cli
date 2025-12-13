#![allow(dead_code)]
#![allow(private_interfaces)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub mod platform;

// Export for JNI
pub use platform::android::*;

// Initialize logging for Android
#[no_mangle]
pub extern "C" fn rust_initialize(config_json: *const c_char) -> bool {
    if config_json.is_null() {
        return false;
    }

    let config_str = unsafe { CStr::from_ptr(config_json).to_str().unwrap_or("{}") };

    // Initialize android logger
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("ArulaCore"),
    );

    log::info!("Arula Android Core initialized with config: {}", config_str);
    true
}

#[no_mangle]
pub extern "C" fn rust_send_message(message: *const c_char) {
    if message.is_null() {
        return;
    }

    let msg_str = unsafe { CStr::from_ptr(message).to_str().unwrap_or("") };
    log::info!("Sending message: {}", msg_str);

    // Simulate processing
    rust_on_message("This is a test response from Arula Android Core!");
}

#[no_mangle]
pub extern "C" fn rust_set_config(config_json: *const c_char) {
    if config_json.is_null() {
        return;
    }

    let config_str = unsafe { CStr::from_ptr(config_json).to_str().unwrap_or("{}") };
    log::info!("Setting config: {}", config_str);
}

#[no_mangle]
pub extern "C" fn rust_get_config() -> *const c_char {
    let config = r#"{"active_provider":"openai","model":"gpt-4"}"#;
    match CString::new(config) {
        Ok(cstring) => cstring.into_raw(),
        Err(_) => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn rust_cleanup() {
    log::info!("Arula Android Core cleanup");
}

#[no_mangle]
pub extern "C" fn rust_set_java_callback(_env: *mut jni::JNIEnv, _callback: jni::objects::JObject) {
    log::info!("Java callback set");
}

// Callback functions to Java
extern "C" {
    fn rust_on_message(message: *const c_char);
    fn rust_on_stream_chunk(chunk: *const c_char);
    fn rust_on_tool_start(tool_name: *const c_char, tool_id: *const c_char);
    fn rust_on_tool_complete(tool_id: *const c_char, result: *const c_char);
    fn rust_on_error(error: *const c_char);
}