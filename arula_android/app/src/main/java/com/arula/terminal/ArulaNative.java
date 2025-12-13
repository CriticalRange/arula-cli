package com.arula.terminal;

import android.content.Context;
import android.util.Log;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * JNI bridge to Rust core library
 */
public class ArulaNative {
    private static final String TAG = "ArulaNative";
    private static boolean initialized = false;
    private static ArulaCallback callback;
    private static final ConcurrentLinkedQueue<String> messageQueue = new ConcurrentLinkedQueue<>();
    private static final AtomicBoolean processing = new AtomicBoolean(false);

    // Load native library
    static {
        try {
            System.loadLibrary("arula_android");
            Log.i(TAG, "Native library loaded successfully");
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "Failed to load native library", e);
        }
    }

    /**
     * Initialize the Arula core with configuration
     */
    public static native boolean initialize(String configJson);

    /**
     * Send a message to the AI
     */
    public static native void sendMessage(String message);

    /**
     * Set configuration provider
     */
    public static native void setConfig(String configJson);

    /**
     * Get current configuration
     */
    public static native String getConfig();

    /**
     * Cleanup resources
     */
    public static native void cleanup();

    /**
     * Check if core is initialized
     */
    public static boolean isInitialized() {
        return initialized;
    }

    /**
     * Set callback for receiving messages from Rust
     */
    public static void setCallback(ArulaCallback cb) {
        callback = cb;
    }

    /**
     * Called from Rust to deliver messages
     */
    private static void onMessageReceived(String message) {
        Log.d(TAG, "Message received from Rust: " + message);
        if (callback != null) {
            callback.onMessage(message);
        }
    }

    /**
     * Called from Rust to deliver stream chunks
     */
    private static void onStreamChunk(String chunk) {
        Log.d(TAG, "Stream chunk received from Rust: " + chunk);
        if (callback != null) {
            callback.onStreamChunk(chunk);
        }
    }

    /**
     * Called from Rust when a tool execution starts
     */
    private static void onToolStart(String toolName, String toolId) {
        Log.d(TAG, "Tool started: " + toolName + " (" + toolId + ")");
        if (callback != null) {
            callback.onToolStart(toolName, toolId);
        }
    }

    /**
     * Called from Rust when a tool execution completes
     */
    private static void onToolComplete(String toolId, String result) {
        Log.d(TAG, "Tool completed: " + toolId);
        if (callback != null) {
            callback.onToolComplete(toolId, result);
        }
    }

    /**
     * Called from Rust for errors
     */
    private static void onError(String error) {
        Log.e(TAG, "Error from Rust: " + error);
        if (callback != null) {
            callback.onError(error);
        }
    }

    /**
     * Callback interface for Rust events
     */
    public interface ArulaCallback {
        void onMessage(String message);
        void onStreamChunk(String chunk);
        void onToolStart(String toolName, String toolId);
        void onToolComplete(String toolId, String result);
        void onError(String error);
    }

    /**
     * Initialize with Android context
     */
    public static boolean initializeWithContext(Context context, String configJson) {
        if (!initialized) {
            // Set up Android-specific paths and configuration
            String androidConfig = enhanceConfigForAndroid(context, configJson);
            initialized = initialize(androidConfig);
            if (initialized) {
                Log.i(TAG, "Arula core initialized successfully");
            }
        }
        return initialized;
    }

    /**
     * Enhance configuration with Android-specific settings
     */
    private static String enhanceConfigForAndroid(Context context, String configJson) {
        // Add Android paths, Termux integration, etc.
        return configJson; // TODO: Implement Android-specific config
    }
}