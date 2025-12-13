package com.arula.terminal;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;
import androidx.annotation.Nullable;
import androidx.core.app.NotificationCompat;

/**
 * Background service for AI processing
 */
public class ArulaService extends Service implements ArulaNative.ArulaCallback {
    private static final String TAG = "ArulaService";
    private static final String CHANNEL_ID = "ArulaServiceChannel";
    private static final int NOTIFICATION_ID = 1;

    private Handler mainHandler;
    private boolean isInitialized = false;

    @Override
    public void onCreate() {
        super.onCreate();
        mainHandler = new Handler(Looper.getMainLooper());
        createNotificationChannel();
        initializeArula();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent.getAction();

        if (action != null) {
            switch (action) {
                case "START_AI":
                    startAiProcessing();
                    break;
                case "STOP_AI":
                    stopAiProcessing();
                    break;
                case "SEND_MESSAGE":
                    String message = intent.getStringExtra("message");
                    if (message != null) {
                        sendMessage(message);
                    }
                    break;
            }
        }

        return START_STICKY;
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel serviceChannel = new NotificationChannel(
                CHANNEL_ID,
                "Arula Service",
                NotificationManager.IMPORTANCE_DEFAULT
            );
            serviceChannel.setDescription("Arula AI processing service");

            NotificationManager manager = getSystemService(NotificationManager.class);
            manager.createNotificationChannel(serviceChannel);
        }
    }

    private void initializeArula() {
        try {
            // Load configuration
            ArulaNative.setCallback(this);

            // Default configuration for service
            String config = createServiceConfig();
            isInitialized = ArulaNative.initialize(this, config);

            if (isInitialized) {
                Log.i(TAG, "Arula service initialized successfully");
            } else {
                Log.e(TAG, "Failed to initialize Arula service");
            }
        } catch (Exception e) {
            Log.e(TAG, "Error initializing Arula service", e);
        }
    }

    private String createServiceConfig() {
        return "{" +
            "\"active_provider\": \"openai\"," +
            "\"providers\": {" +
            "\"openai\": {" +
            "\"api_key\": \"" + System.getenv("OPENAI_API_KEY") + "\"," +
            "\"api_url\": \"https://api.openai.com/v1\"," +
            "\"model\": \"gpt-4\"," +
            "\"max_tokens\": 4096," +
            "\"temperature\": 0.7" +
            "}" +
            "}" +
        "}";
    }

    private void startAiProcessing() {
        startForeground(NOTIFICATION_ID, createServiceNotification("Arula AI Ready"));
        Log.i(TAG, "AI processing started");
    }

    private void stopAiProcessing() {
        stopForeground(true);
        stopSelf();
        Log.i(TAG, "AI processing stopped");
    }

    private void sendMessage(String message) {
        if (isInitialized) {
            ArulaNative.sendMessage(message);
            updateNotification("Processing...");
        } else {
            Log.e(TAG, "Service not initialized");
        }
    }

    private Notification createServiceNotification(String text) {
        return new NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Arula Terminal")
            .setContentText(text)
            .setSmallIcon(R.drawable.ic_terminal)
            .setOngoing(true)
            .build();
    }

    private void updateNotification(String text) {
        Notification notification = createServiceNotification(text);
        NotificationManager manager = getSystemService(NotificationManager.class);
        manager.notify(NOTIFICATION_ID, notification);
    }

    // ArulaNative.ArulaCallback implementations

    @Override
    public void onMessage(String message) {
        mainHandler.post(() -> {
            Log.i(TAG, "AI Response: " + message);
            updateNotification("Response received");

            // In a real implementation, this would save to database
            // and notify any listening components
        });
    }

    @Override
    public void onStreamChunk(String chunk) {
        mainHandler.post(() -> {
            Log.d(TAG, "AI Stream: " + chunk);
            // Handle streaming updates
        });
    }

    @Override
    public void onToolStart(String toolName, String toolId) {
        mainHandler.post(() -> {
            Log.i(TAG, "Tool started: " + toolName);
            updateNotification("Running: " + toolName);
        });
    }

    @Override
    public void onToolComplete(String toolId, String result) {
        mainHandler.post(() -> {
            Log.i(TAG, "Tool completed: " + toolId);
            updateNotification("Ready");
        });
    }

    @Override
    public void onError(String error) {
        mainHandler.post(() -> {
            Log.e(TAG, "AI Error: " + error);
            updateNotification("Error occurred");
        });
    }

    @Override
    public void onDestroy() {
        super.onDestroy();
        if (isInitialized) {
            ArulaNative.cleanup();
        }
        Log.i(TAG, "Arula service destroyed");
    }
}