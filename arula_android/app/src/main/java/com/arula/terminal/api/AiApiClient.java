package com.arula.terminal.api;

import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.IOException;
import java.util.concurrent.TimeUnit;

import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.sse.EventSource;
import okhttp3.sse.EventSourceListener;
import okhttp3.sse.EventSources;

/**
 * Async API client for AI providers using OkHttp
 * Supports OpenAI, Anthropic, Ollama, and OpenAI-compatible endpoints
 */
public class AiApiClient {
    private static final String TAG = "AiApiClient";
    private static final MediaType JSON = MediaType.get("application/json; charset=utf-8");

    private final OkHttpClient client;
    private final OkHttpClient sseClient;
    private final Handler mainHandler;

    // Configuration
    private String provider = "openai";
    private String apiKey = "";
    private String apiUrl = "https://api.openai.com/v1";
    private String model = "gpt-4";
    private String systemPrompt = "You are a helpful AI assistant.";
    private float temperature = 0.7f;
    private int maxTokens = 2048;
    private boolean streamingEnabled = true;
    private boolean debugModeEnabled = false;

    public interface ChatCallback {
        void onResponse(String message);

        void onStreamChunk(String chunk);

        void onStreamComplete(String fullMessage);

        void onError(String error);
    }

    public AiApiClient() {
        client = new OkHttpClient.Builder()
                .connectTimeout(30, TimeUnit.SECONDS)
                .readTimeout(120, TimeUnit.SECONDS)
                .writeTimeout(30, TimeUnit.SECONDS)
                .build();

        sseClient = new OkHttpClient.Builder()
                .connectTimeout(30, TimeUnit.SECONDS)
                .readTimeout(0, TimeUnit.SECONDS) // No timeout for SSE
                .writeTimeout(30, TimeUnit.SECONDS)
                .build();

        mainHandler = new Handler(Looper.getMainLooper());
    }

    // Configuration setters
    public void setProvider(String provider) {
        this.provider = provider;
    }

    public void setApiKey(String apiKey) {
        this.apiKey = apiKey;
    }

    public void setApiUrl(String apiUrl) {
        this.apiUrl = apiUrl;
    }

    public void setModel(String model) {
        this.model = model;
    }

    public void setSystemPrompt(String systemPrompt) {
        this.systemPrompt = systemPrompt;
    }

    public void setTemperature(float temperature) {
        this.temperature = temperature;
    }

    public void setMaxTokens(int maxTokens) {
        this.maxTokens = maxTokens;
    }

    public void setStreamingEnabled(boolean enabled) {
        this.streamingEnabled = enabled;
    }

    public void setDebugModeEnabled(boolean enabled) {
        this.debugModeEnabled = enabled;
    }

    /**
     * Sends a chat message and receives response via callback
     */
    public void sendMessage(String userMessage, ChatCallback callback) {
        if (apiKey == null || apiKey.isEmpty()) {
            mainHandler.post(() -> callback.onError("API key is not set. Please configure it in Settings."));
            return;
        }

        try {
            if (streamingEnabled) {
                sendStreamingRequest(userMessage, callback);
            } else {
                sendNonStreamingRequest(userMessage, callback);
            }
        } catch (Exception e) {
            Log.e(TAG, "Error sending message", e);
            mainHandler.post(() -> callback.onError("Failed to send message: " + e.getMessage()));
        }
    }

    private void sendNonStreamingRequest(String userMessage, ChatCallback callback) throws JSONException {
        JSONObject body = buildRequestBody(userMessage, false);
        String endpoint = getChatEndpoint();
        final String requestBodyStr = body.toString(); // Capture for debug

        Request request = new Request.Builder()
                .url(endpoint)
                .addHeader("Authorization", getAuthHeader())
                .addHeader("Content-Type", "application/json")
                .post(RequestBody.create(requestBodyStr, JSON))
                .build();

        // Add provider-specific headers
        Request.Builder requestBuilder = request.newBuilder();
        addProviderHeaders(requestBuilder);
        request = requestBuilder.build();

        client.newCall(request).enqueue(new Callback() {
            @Override
            public void onFailure(Call call, IOException e) {
                Log.e(TAG, "Request failed", e);
                mainHandler.post(() -> callback.onError("Network error: " + e.getMessage()));
            }

            @Override
            public void onResponse(Call call, Response response) throws IOException {
                try {
                    String responseBody = response.body() != null ? response.body().string() : "";

                    if (!response.isSuccessful()) {
                        String error = parseErrorMessage(responseBody, response.code());
                        if (debugModeEnabled) {
                            error += "\n\n[DEBUG] Request:\n" + requestBodyStr;
                            error += "\n\n[DEBUG] URL: " + endpoint;
                        }
                        String finalError = error;
                        mainHandler.post(() -> callback.onError(finalError));
                        return;
                    }

                    String message = parseResponseMessage(responseBody);
                    mainHandler.post(() -> callback.onResponse(message));

                } catch (Exception e) {
                    Log.e(TAG, "Error parsing response", e);
                    mainHandler.post(() -> callback.onError("Failed to parse response: " + e.getMessage()));
                }
            }
        });
    }

    private void sendStreamingRequest(String userMessage, ChatCallback callback) throws JSONException {
        JSONObject body = buildRequestBody(userMessage, true);
        String endpoint = getChatEndpoint();
        final String requestBodyStr = body.toString(); // Capture for debug

        Request request = new Request.Builder()
                .url(endpoint)
                .addHeader("Authorization", getAuthHeader())
                .addHeader("Content-Type", "application/json")
                .addHeader("Accept", "text/event-stream")
                .post(RequestBody.create(requestBodyStr, JSON))
                .build();

        Request.Builder requestBuilder = request.newBuilder();
        addProviderHeaders(requestBuilder);
        request = requestBuilder.build();

        StringBuilder fullMessage = new StringBuilder();

        EventSourceListener listener = new EventSourceListener() {
            @Override
            public void onOpen(EventSource eventSource, Response response) {
                Log.d(TAG, "SSE connection opened");
            }

            @Override
            public void onEvent(EventSource eventSource, String id, String type, String data) {
                if ("[DONE]".equals(data)) {
                    String complete = fullMessage.toString();
                    mainHandler.post(() -> callback.onStreamComplete(complete));
                    return;
                }

                try {
                    String chunk = parseStreamChunk(data);
                    if (chunk != null && !chunk.isEmpty()) {
                        fullMessage.append(chunk);
                        mainHandler.post(() -> callback.onStreamChunk(chunk));
                    }
                } catch (Exception e) {
                    Log.w(TAG, "Failed to parse chunk: " + data, e);
                }
            }

            @Override
            public void onClosed(EventSource eventSource) {
                Log.d(TAG, "SSE connection closed");
                if (fullMessage.length() > 0) {
                    String complete = fullMessage.toString();
                    mainHandler.post(() -> callback.onStreamComplete(complete));
                }
            }

            @Override
            public void onFailure(EventSource eventSource, Throwable t, Response response) {
                String error = t != null ? t.getMessage() : "Unknown error";
                if (response != null && !response.isSuccessful()) {
                    try {
                        String body = response.body() != null ? response.body().string() : "";
                        error = parseErrorMessage(body, response.code());
                    } catch (IOException e) {
                        error = "HTTP " + response.code();
                    }
                }

                if (debugModeEnabled) {
                    error += "\n\n[DEBUG] Request:\n" + requestBodyStr;
                    error += "\n\n[DEBUG] URL: " + endpoint;
                }

                String finalError = error;
                Log.e(TAG, "SSE failed: " + error, t);
                mainHandler.post(() -> callback.onError(finalError));
            }
        };

        EventSource.Factory factory = EventSources.createFactory(sseClient);
        factory.newEventSource(request, listener);
    }

    private JSONObject buildRequestBody(String userMessage, boolean stream) throws JSONException {
        JSONObject body = new JSONObject();

        if (isAnthropicProvider() && !isZaiProvider()) {
            // Anthropic API format
            body.put("model", model);
            body.put("max_tokens", maxTokens);
            body.put("stream", stream);

            if (systemPrompt != null && !systemPrompt.isEmpty()) {
                body.put("system", systemPrompt);
            }

            JSONArray messages = new JSONArray();
            JSONObject userMsg = new JSONObject();
            userMsg.put("role", "user");
            userMsg.put("content", userMessage);
            messages.put(userMsg);
            body.put("messages", messages);

        } else if (isZaiProvider()) {
            // Z.AI Coding Plan format - doesn't support "system" role
            // Use "assistant" role for system prompt as requested
            body.put("model", model);
            body.put("max_tokens", maxTokens);
            // Round temperature to 1 decimal place
            double roundedTemp = Math.round(temperature * 10.0) / 10.0;
            body.put("temperature", roundedTemp);
            body.put("stream", stream);

            JSONArray messages = new JSONArray();

            // Add system prompt as the first message with role "assistant"
            if (systemPrompt != null && !systemPrompt.isEmpty()) {
                JSONObject systemMsg = new JSONObject();
                systemMsg.put("role", "assistant");
                systemMsg.put("content", systemPrompt);
                messages.put(systemMsg);
            }

            // Add user message
            JSONObject userMsg = new JSONObject();
            userMsg.put("role", "user");
            userMsg.put("content", userMessage);
            messages.put(userMsg);

            body.put("messages", messages);

            Log.d(TAG, "Z.AI request body: " + body.toString());

        } else {
            // OpenAI-compatible format (works for OpenAI, Ollama, OpenRouter)
            body.put("model", model);
            body.put("max_tokens", maxTokens);
            // Round temperature to 1 decimal place
            double roundedTemp = Math.round(temperature * 10.0) / 10.0;
            body.put("temperature", roundedTemp);
            body.put("stream", stream);

            JSONArray messages = new JSONArray();

            if (systemPrompt != null && !systemPrompt.isEmpty()) {
                JSONObject systemMsg = new JSONObject();
                systemMsg.put("role", "system");
                systemMsg.put("content", systemPrompt);
                messages.put(systemMsg);
            }

            JSONObject userMsg = new JSONObject();
            userMsg.put("role", "user");
            userMsg.put("content", userMessage);
            messages.put(userMsg);

            body.put("messages", messages);
        }

        return body;
    }

    private String getChatEndpoint() {
        String baseUrl = apiUrl.endsWith("/") ? apiUrl.substring(0, apiUrl.length() - 1) : apiUrl;

        if (isAnthropicProvider()) {
            // Anthropic direct API
            if (!baseUrl.contains("/v1/messages")) {
                return baseUrl + "/v1/messages";
            }
            return baseUrl;
        } else if (isOllamaProvider()) {
            return baseUrl + "/api/chat";
        } else if (isZaiProvider()) {
            // Z.AI uses the endpoint URL as-is (already contains full path)
            // e.g., https://api.z.ai/api/coding/paas/v4 for Coding Plan
            // or https://api.z.ai/api/anthropic/v1/messages for Anthropic Compatible
            Log.d(TAG, "Z.AI endpoint: " + baseUrl);
            return baseUrl;
        } else {
            // OpenAI-compatible (OpenAI, OpenRouter, etc.)
            if (!baseUrl.contains("/chat/completions")) {
                if (!baseUrl.endsWith("/v1")) {
                    baseUrl = baseUrl + "/v1";
                }
                return baseUrl + "/chat/completions";
            }
            return baseUrl;
        }
    }

    private String getAuthHeader() {
        if (isAnthropicProvider() && !isZaiProvider()) {
            return apiKey; // Anthropic uses x-api-key header instead
        }
        return "Bearer " + apiKey;
    }

    private void addProviderHeaders(Request.Builder builder) {
        if (isAnthropicProvider() && !isZaiProvider()) {
            builder.removeHeader("Authorization");
            builder.addHeader("x-api-key", apiKey);
            builder.addHeader("anthropic-version", "2023-06-01");
        } else if (isOpenRouterProvider()) {
            builder.addHeader("HTTP-Referer", "https://arula.terminal");
            builder.addHeader("X-Title", "Arula Terminal");
        } else if (isZaiProvider()) {
            // Z.AI uses Bearer token auth
            Log.d(TAG, "Z.AI request with API key: " + (apiKey.isEmpty() ? "(empty)" : "(set)"));
        }
    }

    private String parseResponseMessage(String responseBody) throws JSONException {
        JSONObject json = new JSONObject(responseBody);

        Log.d(TAG, "Parsing response for provider: " + provider);

        if (isAnthropicProvider() && !isZaiProvider()) {
            JSONArray content = json.getJSONArray("content");
            if (content.length() > 0) {
                return content.getJSONObject(0).getString("text");
            }
        } else if (isOllamaProvider()) {
            return json.getJSONObject("message").getString("content");
        } else if (isZaiProvider()) {
            // Z.AI Coding Plan returns OpenAI-compatible format
            if (json.has("choices")) {
                JSONArray choices = json.getJSONArray("choices");
                if (choices.length() > 0) {
                    return choices.getJSONObject(0).getJSONObject("message").getString("content");
                }
            }
            // Also try direct message field
            if (json.has("message")) {
                if (json.get("message") instanceof String) {
                    return json.getString("message");
                } else {
                    return json.getJSONObject("message").optString("content", "");
                }
            }
        } else {
            // OpenAI-compatible
            JSONArray choices = json.getJSONArray("choices");
            if (choices.length() > 0) {
                return choices.getJSONObject(0).getJSONObject("message").getString("content");
            }
        }

        return "";
    }

    private String parseStreamChunk(String data) throws JSONException {
        if (debugModeEnabled) {
            Log.d(TAG, "Stream chunk: " + data);
        }

        JSONObject json = new JSONObject(data);

        if (isAnthropicProvider() && !isZaiProvider()) {
            String type = json.optString("type", "");
            if ("content_block_delta".equals(type)) {
                JSONObject delta = json.getJSONObject("delta");
                return delta.optString("text", "");
            }
        } else if (isOllamaProvider()) {
            return json.getJSONObject("message").optString("content", "");
        } else if (isZaiProvider()) {
            // Z.AI specific handling
            // Standard OpenAI format: choices[0].delta.content
            JSONArray choices = json.optJSONArray("choices");
            if (choices != null && choices.length() > 0) {
                JSONObject choice = choices.getJSONObject(0);
                JSONObject delta = choice.optJSONObject("delta");
                if (delta != null) {
                    return delta.optString("content", "");
                }
            }
            // Fallback: check for direct message/content fields not inside choices
            if (json.has("content")) {
                return json.getString("content");
            }
        } else {
            // OpenAI-compatible (including z.ai)
            JSONArray choices = json.optJSONArray("choices");
            if (choices != null && choices.length() > 0) {
                JSONObject choice = choices.getJSONObject(0);
                JSONObject delta = choice.optJSONObject("delta");
                if (delta != null) {
                    return delta.optString("content", "");
                }
            }
        }

        return "";
    }

    private String parseErrorMessage(String responseBody, int statusCode) {
        try {
            JSONObject json = new JSONObject(responseBody);

            // Try common error formats
            if (json.has("error")) {
                Object error = json.get("error");
                if (error instanceof JSONObject) {
                    return ((JSONObject) error).optString("message", "Unknown error");
                } else if (error instanceof String) {
                    return (String) error;
                }
            }

            if (json.has("message")) {
                return json.getString("message");
            }

            if (json.has("detail")) {
                return json.getString("detail");
            }

        } catch (JSONException e) {
            // Ignore parsing errors
        }

        return "HTTP " + statusCode + ": " + responseBody.substring(0, Math.min(100, responseBody.length()));
    }

    private boolean isAnthropicProvider() {
        return provider != null && provider.toLowerCase().contains("anthropic");
    }

    private boolean isOllamaProvider() {
        return provider != null && provider.toLowerCase().contains("ollama");
    }

    private boolean isOpenRouterProvider() {
        return provider != null && provider.toLowerCase().contains("openrouter");
    }

    private boolean isZaiProvider() {
        if (provider == null)
            return false;
        String lower = provider.toLowerCase();
        return lower.contains("z.ai") || lower.equals("zai");
    }

    /**
     * Cancels all pending requests
     */
    public void cancelAll() {
        client.dispatcher().cancelAll();
        sseClient.dispatcher().cancelAll();
    }
}
