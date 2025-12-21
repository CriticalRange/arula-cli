package com.arula.terminal;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.Editable;
import android.text.TextWatcher;
import android.util.Log;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import android.widget.EditText;
import android.widget.ImageButton;
import android.widget.LinearLayout;
import android.widget.TextView;

// Import custom UI components
import com.arula.terminal.ui.canvas.LivingBackground;
import com.arula.terminal.ui.canvas.LoadingSpinner;
import com.arula.terminal.ui.menu.SlidingMenuView;
import com.arula.terminal.api.AiApiClient;
import androidx.appcompat.app.AppCompatActivity;
import androidx.lifecycle.ViewModelProvider;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import com.arula.terminal.databinding.ActivityMainBinding;
import com.google.android.material.snackbar.Snackbar;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.List;

/**
 * Main activity for Arula Terminal
 */
public class MainActivity extends AppCompatActivity implements ArulaNative.ArulaCallback {
    private static final String TAG = "MainActivity";
    private static final int REQUEST_SETTINGS = 1001;

    private ActivityMainBinding binding;
    private MessageAdapter messageAdapter;
    private MainViewModel viewModel;
    private Handler mainHandler;

    // Advanced UI components
    private LivingBackground livingBackground;
    private LoadingSpinner typingSpinner;
    private SlidingMenuView slidingMenu;
    private ImageButton menuButton;

    // API client for AI requests
    private AiApiClient apiClient;
    private StringBuilder streamingMessage;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        binding = ActivityMainBinding.inflate(getLayoutInflater());
        setContentView(binding.getRoot());

        // Initialize main handler for UI updates
        mainHandler = new Handler(Looper.getMainLooper());

        // Initialize ViewModel
        viewModel = new ViewModelProvider(this).get(MainViewModel.class);

        // Setup RecyclerView for messages
        setupMessageList();

        // Setup input field
        setupInputField();

        // Setup menu button
        menuButton = binding.getRoot().findViewById(R.id.menuButton);
        menuButton.setOnClickListener(v -> toggleMenu());

        // Setup send button
        binding.sendButton.setOnClickListener(v -> sendMessage());

        // Initialize advanced UI components
        initializeAdvancedUI();

        // Initialize Arula core
        initializeArula();
    }

    private void setupMessageList() {
        messageAdapter = new MessageAdapter();
        binding.messageList.setLayoutManager(new LinearLayoutManager(this));
        binding.messageList.setAdapter(messageAdapter);

        // Scroll to bottom when new messages are added
        messageAdapter.registerAdapterDataObserver(new RecyclerView.AdapterDataObserver() {
            @Override
            public void onItemRangeInserted(int positionStart, int itemCount) {
                int totalItems = messageAdapter.getItemCount();
                if (totalItems > 0) {
                    binding.messageList.smoothScrollToPosition(totalItems - 1);
                }
            }
        });
    }

    private void setupInputField() {
        EditText inputField = binding.messageInput;

        // Enable send button when there's text
        inputField.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {
            }

            @Override
            public void onTextChanged(CharSequence s, int start, int before, int count) {
                binding.sendButton.setEnabled(s.toString().trim().length() > 0);
            }

            @Override
            public void afterTextChanged(Editable s) {
            }
        });

        // Send on Ctrl+Enter or Enter if not multiline
        inputField.setOnEditorActionListener((v, actionId, event) -> {
            if (event != null && event.getAction() == android.view.KeyEvent.ACTION_DOWN) {
                if (event.getKeyCode() == android.view.KeyEvent.KEYCODE_ENTER &&
                        (event.isShiftPressed() || event.isCtrlPressed())) {
                    sendMessage();
                    return true;
                }
            }
            return false;
        });
    }

    private void initializeArula() {
        // Initialize API client
        apiClient = new AiApiClient();
        streamingMessage = new StringBuilder();

        // Configure API client from settings
        updateApiClientFromSettings();

        // Load conversation history
        List<Message> history = viewModel.getMessages().getValue();
        if (history != null) {
            messageAdapter.setMessages(history);
        }

        // Keep native callback for potential future use
        ArulaNative.setCallback(this);
    }

    private void updateApiClientFromSettings() {
        if (apiClient == null || settingsManager == null)
            return;

        apiClient.setProvider(settingsManager.getActiveProvider());
        apiClient.setApiKey(settingsManager.getApiKey());
        apiClient.setApiUrl(settingsManager.getApiUrl());
        apiClient.setModel(settingsManager.getModel());
        apiClient.setSystemPrompt(settingsManager.getSystemPrompt());
        apiClient.setTemperature(settingsManager.getTemperature());
        apiClient.setMaxTokens(settingsManager.getMaxTokens());
        apiClient.setStreamingEnabled(settingsManager.isStreamingEnabled());
        apiClient.setDebugModeEnabled(settingsManager.isDebugModeEnabled());
    }

    private void sendMessage() {
        EditText inputField = binding.messageInput;
        String message = inputField.getText().toString().trim();

        if (message.isEmpty())
            return;

        // Clear input
        inputField.setText("");

        // Add user message to UI
        Message userMessage = new Message(message, Message.Type.USER);
        messageAdapter.addMessage(userMessage);
        viewModel.addMessage(userMessage);

        // Show typing indicator
        showTypingIndicator(true);

        // Update API client with latest settings before sending
        updateApiClientFromSettings();

        // Reset streaming message buffer
        streamingMessage.setLength(0);

        // Create AI message but DON'T add to adapter yet - wait for content
        // This prevents empty bubbles from showing
        final Message aiMessage = new Message("", Message.Type.ASSISTANT);
        final boolean[] addedToAdapter = { false };

        // Send to AI via OkHttp
        apiClient.sendMessage(message, new AiApiClient.ChatCallback() {
            @Override
            public void onResponse(String response) {
                Log.d(TAG, "onResponse: " + (response != null ? response.length() + " chars" : "null"));
                showTypingIndicator(false);

                if (response == null || response.trim().isEmpty()) {
                    Log.w(TAG, "Empty response received");
                    return;
                }

                aiMessage.setText(response);
                // Only add to adapter now that we have content
                if (!addedToAdapter[0]) {
                    messageAdapter.addMessage(aiMessage);
                    addedToAdapter[0] = true;
                }
                viewModel.addMessage(aiMessage);
                scrollToBottom();
            }

            @Override
            public void onStreamChunk(String chunk) {
                if (chunk == null || chunk.isEmpty()) {
                    return;
                }

                streamingMessage.append(chunk);
                aiMessage.setText(streamingMessage.toString());

                // Add to adapter on first chunk
                if (!addedToAdapter[0]) {
                    messageAdapter.addMessage(aiMessage);
                    addedToAdapter[0] = true;
                    Log.d(TAG, "Added AI message to adapter on first chunk");
                } else {
                    // Update existing message
                    messageAdapter.updateLastMessage();
                }
                scrollToBottom();
            }

            @Override
            public void onStreamComplete(String fullMessage) {
                Log.d(TAG, "onStreamComplete: " + (fullMessage != null ? fullMessage.length() + " chars" : "null"));
                showTypingIndicator(false);

                // If we already received chunks, the message is already in adapter
                if (addedToAdapter[0]) {
                    // Just save to viewmodel if we haven't already
                    if (fullMessage != null && !fullMessage.trim().isEmpty()) {
                        aiMessage.setText(fullMessage);
                        messageAdapter.updateLastMessage();
                    }
                    viewModel.addMessage(aiMessage);
                    scrollToBottom();
                    return;
                }

                // No chunks received - add the complete message now
                if (fullMessage != null && !fullMessage.trim().isEmpty()) {
                    aiMessage.setText(fullMessage);
                    messageAdapter.addMessage(aiMessage);
                    addedToAdapter[0] = true;
                    viewModel.addMessage(aiMessage);
                    scrollToBottom();
                }
            }

            @Override
            public void onError(String error) {
                Log.e(TAG, "onError: " + error);
                showTypingIndicator(false);

                // Remove AI message if it was added (during streaming)
                if (addedToAdapter[0]) {
                    messageAdapter.removeLastMessage();
                }

                showError(error);
                Message errorMessage = new Message("Error: " + error, Message.Type.ERROR);
                messageAdapter.addMessage(errorMessage);
            }
        });
    }

    private void scrollToBottom() {
        if (binding.messageList.getAdapter() != null) {
            binding.messageList.scrollToPosition(binding.messageList.getAdapter().getItemCount() - 1);
        }
    }

    @Override
    public void onMessage(String message) {
        mainHandler.post(() -> {
            showTypingIndicator(false);
            Message aiMessage = new Message(message, Message.Type.ASSISTANT);
            messageAdapter.addMessage(aiMessage);
            viewModel.addMessage(aiMessage);
        });
    }

    @Override
    public void onStreamChunk(String chunk) {
        mainHandler.post(() -> {
            // Update last message with streaming chunk
            messageAdapter.appendToLastMessage(chunk);
        });
    }

    @Override
    public void onToolStart(String toolName, String toolId) {
        mainHandler.post(() -> {
            // Show tool execution indicator
            Message toolMessage = new Message("🔧 " + toolName + "...", Message.Type.TOOL);
            toolMessage.setToolId(toolId);
            messageAdapter.addMessage(toolMessage);
        });
    }

    @Override
    public void onToolComplete(String toolId, String result) {
        mainHandler.post(() -> {
            // Update tool message with result
            messageAdapter.updateToolMessage(toolId, result);
        });
    }

    @Override
    public void onError(String error) {
        mainHandler.post(() -> {
            showTypingIndicator(false);
            showError(error);
            Message errorMessage = new Message("Error: " + error, Message.Type.ERROR);
            messageAdapter.addMessage(errorMessage);
        });
    }

    private SettingsManager settingsManager;

    private void initializeAdvancedUI() {
        // Initialize settings manager
        settingsManager = new SettingsManager(this);

        // Initialize living background
        livingBackground = binding.getRoot().findViewById(R.id.livingBackground);
        boolean bgEnabled = settingsManager.isLivingBackgroundEnabled();
        livingBackground.setEnabled(bgEnabled);
        livingBackground.setOpacity(bgEnabled ? 0.5f : 0.0f);

        // Setup settings change listener
        settingsManager.setListener(new SettingsManager.SettingsChangeListener() {
            @Override
            public void onSettingsChanged() {
                // Settings were changed, sync to native
                Log.d(TAG, "Settings changed, syncing to native");
            }

            @Override
            public void onLivingBackgroundChanged(boolean enabled) {
                // Update living background in real-time
                livingBackground.setEnabled(enabled);
                livingBackground.setOpacity(enabled ? 0.5f : 0.0f);
            }
        });

        // Initialize typing indicator spinner
        LinearLayout typingIndicator = binding.getRoot().findViewById(R.id.typingIndicator);
        typingSpinner = typingIndicator.findViewById(R.id.typingSpinner);
        typingSpinner.setAnimationSpeed(1.5f);

        // Initialize sliding menu
        slidingMenu = binding.getRoot().findViewById(R.id.slidingMenu);
        slidingMenu.setMenuButton(menuButton); // Connect menu button for animation
        slidingMenu.setSettingsManager(settingsManager); // Share settings manager
        slidingMenu.setListener(new SlidingMenuView.MenuListener() {
            @Override
            public void onMenuOpened() {
                // Dim living background when menu is open
                if (settingsManager.isLivingBackgroundEnabled()) {
                    livingBackground.setOpacity(0.2f);
                }
            }

            @Override
            public void onMenuClosed() {
                // Restore living background opacity
                if (settingsManager.isLivingBackgroundEnabled()) {
                    livingBackground.setOpacity(0.5f);
                }
            }

            @Override
            public void onPageChanged(SlidingMenuView.MenuPage page) {
                Log.d(TAG, "Menu page changed to: " + page);
            }
        });

        // Sync settings to native on startup
        settingsManager.syncToNative();
    }

    private void toggleMenu() {
        slidingMenu.toggleMenu();
    }

    private void showTypingIndicator(boolean show) {
        LinearLayout typingIndicator = binding.getRoot().findViewById(R.id.typingIndicator);
        if (show) {
            typingIndicator.setVisibility(View.VISIBLE);
            typingSpinner.show();
            // Apply pulse animation
            typingIndicator.startAnimation(android.view.animation.AnimationUtils.loadAnimation(
                    this, R.anim.typing_indicator_pulse));
        } else {
            typingSpinner.hide();
            typingIndicator.setVisibility(View.GONE);
            typingIndicator.clearAnimation();
        }
    }

    private void showError(String error) {
        Snackbar.make(binding.coordinator, error, Snackbar.LENGTH_LONG)
                .setAction("Dismiss", v -> {
                })
                .show();
    }

    @Override
    public boolean onCreateOptionsMenu(Menu menu) {
        getMenuInflater().inflate(R.menu.menu_main, menu);
        return true;
    }

    @Override
    public boolean onOptionsItemSelected(MenuItem item) {
        int id = item.getItemId();

        if (id == R.id.action_settings) {
            openSettings();
            return true;
        } else if (id == R.id.action_clear) {
            clearConversation();
            return true;
        } else if (id == R.id.action_export) {
            exportConversation();
            return true;
        }

        return super.onOptionsItemSelected(item);
    }

    private void openSettings() {
        Intent intent = new Intent(this, SettingsActivity.class);
        startActivityForResult(intent, REQUEST_SETTINGS);
    }

    private void clearConversation() {
        viewModel.clearMessages();
        messageAdapter.clearMessages();
    }

    private void exportConversation() {
        try {
            String exported = viewModel.exportConversation();
            // TODO: Implement share intent
            showError("Export feature coming soon");
        } catch (Exception e) {
            showError("Failed to export: " + e.getMessage());
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);

        if (requestCode == REQUEST_SETTINGS && resultCode == RESULT_OK) {
            // Configuration changed, reinitialize
            initializeArula();
        }
    }

    // Menu click handlers (legacy, remove if not used from XML onClick)
    public void onProviderClick(View v) {
        slidingMenu.navigateToPage(SlidingMenuView.MenuPage.PROVIDER);
    }

    public void onBehaviorClick(View v) {
        slidingMenu.navigateToPage(SlidingMenuView.MenuPage.BEHAVIOR);
    }

    public void onAppearanceClick(View v) {
        slidingMenu.navigateToPage(SlidingMenuView.MenuPage.APPEARANCE);
    }

    @Override
    public void onBackPressed() {
        if (slidingMenu.isOpen()) {
            if (slidingMenu.getCurrentPage() != SlidingMenuView.MenuPage.MAIN) {
                slidingMenu.navigateToMain();
            } else {
                slidingMenu.closeMenu();
            }
        } else {
            super.onBackPressed();
        }
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        ArulaNative.cleanup();
        binding = null;
    }
}