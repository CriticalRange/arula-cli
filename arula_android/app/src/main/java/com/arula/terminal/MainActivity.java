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
import android.widget.TextView;
import androidx.appcompat.app.AppCompatActivity;
import androidx.appcompat.widget.Toolbar;
import androidx.lifecycle.ViewModelProvider;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import com.arula.terminal.databinding.ActivityMainBinding;
import com.google.android.material.snackbar.Snackbar;
import org.json.JSONException;
import org.json.JSONObject;

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

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        binding = ActivityMainBinding.inflate(getLayoutInflater());
        setContentView(binding.getRoot());

        // Initialize main handler for UI updates
        mainHandler = new Handler(Looper.getMainLooper());

        // Setup toolbar
        Toolbar toolbar = binding.toolbar;
        setSupportActionBar(toolbar);

        // Initialize ViewModel
        viewModel = new ViewModelProvider(this).get(MainViewModel.class);

        // Setup RecyclerView for messages
        setupMessageList();

        // Setup input field
        setupInputField();

        // Setup send button
        binding.sendButton.setOnClickListener(v -> sendMessage());

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
                binding.messageList.smoothScrollToPosition(messageAdapter.getItemCount() - 1);
            }
        });
    }

    private void setupInputField() {
        EditText inputField = binding.messageInput;

        // Enable send button when there's text
        inputField.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {}

            @Override
            public void onTextChanged(CharSequence s, int start, int before, int count) {
                binding.sendButton.setEnabled(s.toString().trim().length() > 0);
            }

            @Override
            public void afterTextChanged(Editable s) {}
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
        try {
            // Load configuration
            JSONObject config = viewModel.getConfig();

            // Initialize native library
            ArulaNative.setCallback(this);
            boolean initialized = ArulaNative.initializeWithContext(this, config.toString());

            if (!initialized) {
                showError("Failed to initialize Arula core");
            } else {
                // Load conversation history
                messageAdapter.setMessages(viewModel.getMessages());
            }
        } catch (JSONException e) {
            Log.e(TAG, "Failed to load configuration", e);
            showError("Configuration error: " + e.getMessage());
        }
    }

    private void sendMessage() {
        EditText inputField = binding.messageInput;
        String message = inputField.getText().toString().trim();

        if (message.isEmpty()) return;

        // Clear input
        inputField.setText("");

        // Add user message to UI
        Message userMessage = new Message(message, Message.Type.USER);
        messageAdapter.addMessage(userMessage);
        viewModel.addMessage(userMessage);

        // Show typing indicator
        showTypingIndicator(true);

        // Send to AI
        ArulaNative.sendMessage(message);
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

    private void showTypingIndicator(boolean show) {
        binding.typingIndicator.setVisibility(show ? View.VISIBLE : View.GONE);
    }

    private void showError(String error) {
        Snackbar.make(binding.coordinator, error, Snackbar.LENGTH_LONG)
            .setAction("Dismiss", v -> {})
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

    @Override
    protected void onDestroy() {
        super.onDestroy();
        ArulaNative.cleanup();
        binding = null;
    }
}