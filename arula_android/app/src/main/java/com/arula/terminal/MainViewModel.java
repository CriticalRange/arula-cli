package com.arula.terminal;

import android.app.Application;
import android.content.SharedPreferences;
import androidx.lifecycle.AndroidViewModel;
import androidx.lifecycle.LiveData;
import androidx.lifecycle.MutableLiveData;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.ArrayList;
import java.util.List;

/**
 * ViewModel for MainActivity - manages conversation state
 */
public class MainViewModel extends AndroidViewModel {
    private static final String TAG = "MainViewModel";
    private static final String PREF_KEY_MESSAGES = "messages";
    private static final int MAX_MESSAGES = 1000;

    private final MutableLiveData<List<Message>> messages = new MutableLiveData<>();
    private final MutableLiveData<String> statusText = new MutableLiveData<>();
    private final SharedPreferences preferences;
    private final List<Message> messageList = new ArrayList<>();

    public MainViewModel(Application application) {
        super(application);
        preferences = application.getSharedPreferences("arula_prefs", Application.MODE_PRIVATE);
        loadMessages();
    }

    public LiveData<List<Message>> getMessages() {
        return messages;
    }

    public LiveData<String> getStatusText() {
        return statusText;
    }

    public void addMessage(Message message) {
        messageList.add(message);

        // Limit message history
        while (messageList.size() > MAX_MESSAGES) {
            messageList.remove(0);
        }

        messages.setValue(new ArrayList<>(messageList));
        saveMessages();
    }

    public void clearMessages() {
        messageList.clear();
        messages.setValue(new ArrayList<>(messageList));
        preferences.edit().remove(PREF_KEY_MESSAGES).apply();
    }

    private void loadMessages() {
        String messagesJson = preferences.getString(PREF_KEY_MESSAGES, "[]");
        try {
            org.json.JSONArray array = new org.json.JSONArray(messagesJson);
            for (int i = 0; i < array.length(); i++) {
                org.json.JSONObject obj = array.getJSONObject(i);
                Message message = new Message(
                    obj.getLong("id"),
                    obj.getString("text"),
                    Message.Type.valueOf(obj.getString("type")),
                    obj.getLong("timestamp"),
                    obj.optString("toolId", null)
                );
                messageList.add(message);
            }
            messages.setValue(new ArrayList<>(messageList));
        } catch (JSONException e) {
            // Handle error, start with empty list
            messages.setValue(new ArrayList<>());
        }
    }

    private void saveMessages() {
        try {
            org.json.JSONArray array = new org.json.JSONArray();
            for (Message message : messageList) {
                org.json.JSONObject obj = new org.json.JSONObject();
                obj.put("id", message.getId());
                obj.put("text", message.getText());
                obj.put("type", message.getType().toString());
                obj.put("timestamp", message.getTimestamp());
                if (message.getToolId() != null) {
                    obj.put("toolId", message.getToolId());
                }
                array.put(obj);
            }

            SharedPreferences.Editor editor = preferences.edit();
            editor.putString(PREF_KEY_MESSAGES, array.toString());
            editor.apply();
        } catch (JSONException e) {
            // Handle error
        }
    }

    public JSONObject getConfig() {
        JSONObject config = new JSONObject();
        try {
            // Default configuration
            config.put("active_provider", preferences.getString("active_provider", "openai"));

            // UI settings
            config.put("ui_theme", preferences.getString("ui_theme", "light"));
            config.put("ui_font_size", preferences.getString("ui_font_size", "14"));
            config.put("ui_show_timestamps", preferences.getBoolean("ui_show_timestamps", true));
            config.put("ui_auto_scroll", preferences.getBoolean("ui_auto_scroll", true));
            config.put("ui_enable_notifications", preferences.getBoolean("ui_enable_notifications", true));
            config.put("ui_vibrate_on_tool", preferences.getBoolean("ui_vibrate_on_tool", false));

            // System settings
            config.put("system_log_level", preferences.getString("system_log_level", "info"));
            config.put("system_max_history", preferences.getInt("system_max_history", 1000));
            config.put("system_auto_save", preferences.getBoolean("system_auto_save", true));
            config.put("system_export_format", preferences.getString("system_export_format", "json"));
            config.put("system_enable_termux_api", preferences.getBoolean("system_enable_termux_api", true));

            // Provider configurations
            JSONObject providers = new JSONObject();

            // OpenAI
            JSONObject openai = new JSONObject();
            openai.put("api_key", preferences.getString("openai_api_key", ""));
            openai.put("api_url", preferences.getString("openai_api_url", "https://api.openai.com/v1"));
            openai.put("model", preferences.getString("openai_model", "gpt-4"));
            openai.put("max_tokens", Integer.parseInt(preferences.getString("openai_max_tokens", "4096")));
            openai.put("temperature", Float.parseFloat(preferences.getString("openai_temperature", "0.7")));
            providers.put("openai", openai);

            // Anthropic
            JSONObject anthropic = new JSONObject();
            anthropic.put("api_key", preferences.getString("anthropic_api_key", ""));
            anthropic.put("api_url", preferences.getString("anthropic_api_url", "https://api.anthropic.com"));
            anthropic.put("model", preferences.getString("anthropic_model", "claude-3-opus-20240229"));
            anthropic.put("max_tokens", Integer.parseInt(preferences.getString("anthropic_max_tokens", "4096")));
            anthropic.put("temperature", Float.parseFloat(preferences.getString("anthropic_temperature", "0.7")));
            providers.put("anthropic", anthropic);

            // Z.AI
            JSONObject zai = new JSONObject();
            zai.put("api_key", preferences.getString("zai_api_key", ""));
            zai.put("api_url", preferences.getString("zai_api_url", "https://z.ai/api"));
            zai.put("model", preferences.getString("zai_model", "glm-4"));
            zai.put("max_tokens", Integer.parseInt(preferences.getString("zai_max_tokens", "4096")));
            zai.put("temperature", Float.parseFloat(preferences.getString("zai_temperature", "0.7")));
            providers.put("zai", zai);

            config.put("providers", providers);

        } catch (JSONException e) {
            // Return empty config on error
            config = new JSONObject();
        }

        return config;
    }

    public String exportConversation() throws Exception {
        if (messageList.isEmpty()) {
            return "No messages to export";
        }

        StringBuilder export = new StringBuilder();
        export.append("# Arula Conversation Export\n\n");
        export.append("Exported: ").append(new java.util.Date()).append("\n\n");

        for (Message message : messageList) {
            export.append("## ");
            switch (message.getType()) {
                case USER:
                    export.append("You");
                    break;
                case ASSISTANT:
                    export.append("Arula");
                    break;
                case TOOL:
                    export.append("Tool");
                    break;
                case ERROR:
                    export.append("Error");
                    break;
            }

            if (message.getTimestamp() > 0) {
                export.append(" (").append(new java.util.Date(message.getTimestamp())).append(")");
            }

            export.append("\n\n");
            export.append(message.getText()).append("\n\n");
            export.append("---\n\n");
        }

        return export.toString();
    }

    public void updateStatus(String status) {
        statusText.setValue(status);
    }
}