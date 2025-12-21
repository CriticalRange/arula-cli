package com.arula.terminal;

import android.content.Context;
import android.content.SharedPreferences;
import androidx.preference.PreferenceManager;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Manages application settings and synchronizes with native core
 */
public class SettingsManager {
    private static final String TAG = "SettingsManager";

    // Provider keys
    public static final String KEY_ACTIVE_PROVIDER = "active_provider";
    public static final String KEY_API_KEY = "api_key";
    public static final String KEY_API_URL = "api_url";
    public static final String KEY_MODEL = "model";

    // Behavior keys
    public static final String KEY_SYSTEM_PROMPT = "system_prompt";
    public static final String KEY_TEMPERATURE = "temperature";
    public static final String KEY_MAX_TOKENS = "max_tokens";
    public static final String KEY_STREAMING_ENABLED = "streaming_enabled";
    public static final String KEY_THINKING_ENABLED = "thinking_enabled";

    // Appearance keys
    public static final String KEY_LIVING_BACKGROUND_ENABLED = "living_background_enabled";
    public static final String KEY_DEBUG_MODE_ENABLED = "debug_mode_enabled";

    // Default values
    public static final String DEFAULT_PROVIDER = "openai";
    public static final String DEFAULT_MODEL = "gpt-4";
    public static final String DEFAULT_API_URL = "https://api.openai.com/v1";
    public static final String DEFAULT_SYSTEM_PROMPT = "You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.";
    public static final float DEFAULT_TEMPERATURE = 0.7f;
    public static final int DEFAULT_MAX_TOKENS = 2048;
    public static final boolean DEFAULT_STREAMING = true;
    public static final boolean DEFAULT_THINKING = false;
    public static final boolean DEFAULT_LIVING_BACKGROUND = true;
    public static final boolean DEFAULT_DEBUG_MODE = false;

    // Provider list (matching desktop)
    public static final String[] PROVIDERS = { "openai", "anthropic", "z.ai coding plan", "ollama", "openrouter" };

    // Z.AI endpoint options
    public static final String[] ZAI_ENDPOINTS = { "Coding Plan", "Anthropic Compatible", "Custom" };
    public static final String KEY_ZAI_ENDPOINT = "zai_endpoint";

    private final SharedPreferences prefs;
    private final Context context;
    private SettingsChangeListener listener;

    public interface SettingsChangeListener {
        void onSettingsChanged();

        void onLivingBackgroundChanged(boolean enabled);
    }

    public SettingsManager(Context context) {
        this.context = context;
        this.prefs = PreferenceManager.getDefaultSharedPreferences(context);
    }

    public void setListener(SettingsChangeListener listener) {
        this.listener = listener;
    }

    // Provider & Model getters/setters
    public String getActiveProvider() {
        return prefs.getString(KEY_ACTIVE_PROVIDER, DEFAULT_PROVIDER);
    }

    public void setActiveProvider(String provider) {
        prefs.edit().putString(KEY_ACTIVE_PROVIDER, provider).apply();
        notifyChanged();
    }

    public String getApiKey() {
        return prefs.getString(KEY_API_KEY, "");
    }

    public void setApiKey(String apiKey) {
        prefs.edit().putString(KEY_API_KEY, apiKey).apply();
        notifyChanged();
    }

    public String getApiUrl() {
        return prefs.getString(KEY_API_URL, DEFAULT_API_URL);
    }

    public void setApiUrl(String apiUrl) {
        prefs.edit().putString(KEY_API_URL, apiUrl).apply();
        notifyChanged();
    }

    public String getModel() {
        return prefs.getString(KEY_MODEL, DEFAULT_MODEL);
    }

    public void setModel(String model) {
        prefs.edit().putString(KEY_MODEL, model).apply();
        notifyChanged();
    }

    public boolean isThinkingEnabled() {
        return prefs.getBoolean(KEY_THINKING_ENABLED, DEFAULT_THINKING);
    }

    public void setThinkingEnabled(boolean enabled) {
        prefs.edit().putBoolean(KEY_THINKING_ENABLED, enabled).apply();
        notifyChanged();
    }

    // Z.AI specific
    public String getZaiEndpoint() {
        return prefs.getString(KEY_ZAI_ENDPOINT, "Coding Plan");
    }

    public void setZaiEndpoint(String endpoint) {
        prefs.edit().putString(KEY_ZAI_ENDPOINT, endpoint).apply();
        // Update API URL based on endpoint
        setApiUrl(getZaiEndpointUrl(endpoint));
    }

    // Behavior getters/setters
    public String getSystemPrompt() {
        return prefs.getString(KEY_SYSTEM_PROMPT, DEFAULT_SYSTEM_PROMPT);
    }

    public void setSystemPrompt(String prompt) {
        prefs.edit().putString(KEY_SYSTEM_PROMPT, prompt).apply();
        notifyChanged();
    }

    public float getTemperature() {
        return prefs.getFloat(KEY_TEMPERATURE, DEFAULT_TEMPERATURE);
    }

    public void setTemperature(float temperature) {
        prefs.edit().putFloat(KEY_TEMPERATURE, temperature).apply();
        notifyChanged();
    }

    public int getMaxTokens() {
        return prefs.getInt(KEY_MAX_TOKENS, DEFAULT_MAX_TOKENS);
    }

    public void setMaxTokens(int maxTokens) {
        prefs.edit().putInt(KEY_MAX_TOKENS, maxTokens).apply();
        notifyChanged();
    }

    public boolean isStreamingEnabled() {
        return prefs.getBoolean(KEY_STREAMING_ENABLED, DEFAULT_STREAMING);
    }

    public void setStreamingEnabled(boolean enabled) {
        prefs.edit().putBoolean(KEY_STREAMING_ENABLED, enabled).apply();
        notifyChanged();
    }

    // Appearance getters/setters
    public boolean isLivingBackgroundEnabled() {
        return prefs.getBoolean(KEY_LIVING_BACKGROUND_ENABLED, DEFAULT_LIVING_BACKGROUND);
    }

    public void setLivingBackgroundEnabled(boolean enabled) {
        prefs.edit().putBoolean(KEY_LIVING_BACKGROUND_ENABLED, enabled).apply();
        if (listener != null) {
            listener.onLivingBackgroundChanged(enabled);
        }
    }

    public boolean isDebugModeEnabled() {
        return prefs.getBoolean(KEY_DEBUG_MODE_ENABLED, DEFAULT_DEBUG_MODE);
    }

    public void setDebugModeEnabled(boolean enabled) {
        prefs.edit().putBoolean(KEY_DEBUG_MODE_ENABLED, enabled).apply();
        notifyChanged();
    }

    private void notifyChanged() {
        if (listener != null) {
            listener.onSettingsChanged();
        }
        // Sync with native
        syncToNative();
    }

    /**
     * Synchronizes all settings to the native core
     */
    public void syncToNative() {
        try {
            JSONObject config = new JSONObject();
            config.put("active_provider", getActiveProvider());
            config.put("api_key", getApiKey());
            config.put("api_url", getApiUrl());
            config.put("model", getModel());
            config.put("system_prompt", getSystemPrompt());
            config.put("temperature", getTemperature());
            config.put("max_tokens", getMaxTokens());
            config.put("streaming", isStreamingEnabled());
            config.put("thinking_enabled", isThinkingEnabled());

            ArulaNative.setConfig(config.toString());
        } catch (JSONException e) {
            android.util.Log.e(TAG, "Failed to sync config to native", e);
        }
    }

    /**
     * Loads settings from native core (if available)
     */
    public void loadFromNative() {
        try {
            String configJson = ArulaNative.getConfig();
            if (configJson != null && !configJson.isEmpty()) {
                JSONObject config = new JSONObject(configJson);

                if (config.has("active_provider")) {
                    setActiveProvider(config.getString("active_provider"));
                }
                if (config.has("api_key")) {
                    setApiKey(config.getString("api_key"));
                }
                if (config.has("api_url")) {
                    setApiUrl(config.getString("api_url"));
                }
                if (config.has("model")) {
                    setModel(config.getString("model"));
                }
                if (config.has("temperature")) {
                    setTemperature((float) config.getDouble("temperature"));
                }
                if (config.has("max_tokens")) {
                    setMaxTokens(config.getInt("max_tokens"));
                }
            }
        } catch (JSONException e) {
            android.util.Log.e(TAG, "Failed to load config from native", e);
        }
    }

    /**
     * Gets the default API URL for a provider
     */
    public static String getDefaultApiUrl(String provider) {
        switch (provider.toLowerCase()) {
            case "openai":
                return "https://api.openai.com/v1";
            case "anthropic":
                return "https://api.anthropic.com";
            case "z.ai coding plan":
            case "z.ai":
                return "https://api.z.ai/api/coding/paas/v4";
            case "ollama":
                return "http://localhost:11434";
            case "openrouter":
                return "https://openrouter.ai/api/v1";
            default:
                return "https://api.openai.com/v1";
        }
    }

    /**
     * Gets the URL for a Z.AI endpoint
     */
    public static String getZaiEndpointUrl(String endpoint) {
        switch (endpoint) {
            case "Coding Plan":
                return "https://api.z.ai/api/coding/paas/v4";
            case "Anthropic Compatible":
                return "https://api.z.ai/api/anthropic/v1/messages";
            case "Custom":
            default:
                return "";
        }
    }

    /**
     * Gets available models for a provider
     */
    public static String[] getModelsForProvider(String provider) {
        switch (provider.toLowerCase()) {
            case "openai":
                return new String[] { "gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4", "gpt-3.5-turbo", "o1-preview",
                        "o1-mini" };
            case "anthropic":
                return new String[] { "claude-3-5-sonnet-20241022", "claude-3-opus-20240229",
                        "claude-3-sonnet-20240229", "claude-3-haiku-20240307" };
            case "z.ai coding plan":
            case "z.ai":
                return new String[] { "GLM-4.6", "gemini-2.0-flash", "gemini-pro", "claude-3-5-sonnet" };
            case "ollama":
                return new String[] { "llama3.2", "llama3.1", "codellama", "mistral", "mixtral", "qwen2.5-coder" };
            case "openrouter":
                return new String[] { "openai/gpt-4o", "anthropic/claude-3-opus", "google/gemini-pro",
                        "meta-llama/llama-3.1-405b-instruct" };
            default:
                return new String[] { "gpt-4" };
        }
    }

    /**
     * Check if provider is Z.AI
     */
    public static boolean isZaiProvider(String provider) {
        String lower = provider.toLowerCase();
        return lower.contains("z.ai") || lower.equals("zai");
    }
}
