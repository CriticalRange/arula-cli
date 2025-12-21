package com.arula.terminal.ui.menu;

import android.animation.Animator;
import android.animation.AnimatorListenerAdapter;
import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.text.Editable;
import android.text.TextWatcher;
import android.util.AttributeSet;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.animation.DecelerateInterpolator;
import android.widget.ArrayAdapter;
import android.widget.CheckBox;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ImageButton;
import android.widget.SeekBar;
import android.widget.Spinner;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.AdapterView;
import androidx.annotation.Nullable;

import com.arula.terminal.R;
import com.arula.terminal.SettingsManager;
import com.arula.terminal.ui.animation.SpringAnimation;

/**
 * Full-screen overlay menu with liquid expanding background
 * Matches desktop version with submenu slide navigation and functional settings
 */
public class SlidingMenuView extends FrameLayout {
    public enum MenuState {
        CLOSED,
        ANIMATING,
        OPEN
    }

    public enum MenuPage {
        MAIN,
        PROVIDER,
        BEHAVIOR,
        APPEARANCE,
        ABOUT
    }

    private MenuState currentState = MenuState.CLOSED;
    private MenuPage currentPage = MenuPage.MAIN;

    // Spring animation
    private SpringAnimation spring;
    private ValueAnimator animator;
    private ValueAnimator pageAnimator;

    // Drawing
    private Paint circlePaint;
    private int backgroundColor;

    private static final float ANCHOR_X = 40f;
    private float anchorY;
    private float maxRadius;

    // Page transition
    private float pageProgress = 0f;
    private float screenWidth = 0f;

    // Content views
    private View menuContent;
    private ViewGroup mainMenuContainer;
    private ViewGroup submenuContainer;
    private ImageButton closeButton;
    private View backButton;
    private View menuButton;

    // Settings manager
    private SettingsManager settingsManager;

    // Settings UI elements
    private Spinner providerSpinner;
    private TextView modelName;
    private EditText apiKeyInput;
    private CheckBox thinkingModeCheckbox;
    private EditText endpointInput;

    private EditText systemPromptInput;
    private SeekBar temperatureSeekBar;
    private TextView temperatureLabel;
    private EditText maxTokensInput;
    private Switch streamingSwitch;

    private Switch livingBackgroundSwitch;

    public interface MenuListener {
        void onMenuOpened();

        void onMenuClosed();

        void onPageChanged(MenuPage page);
    }

    private MenuListener listener;

    public SlidingMenuView(Context context) {
        super(context);
        init();
    }

    public SlidingMenuView(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        init();
    }

    public SlidingMenuView(Context context, @Nullable AttributeSet attrs, int defStyleAttr) {
        super(context, attrs, defStyleAttr);
        init();
    }

    private void init() {
        setWillNotDraw(false);

        spring = new SpringAnimation();
        backgroundColor = getContext().getColor(R.color.neon_background);

        circlePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        circlePaint.setStyle(Paint.Style.FILL);

        // Initialize settings manager
        settingsManager = new SettingsManager(getContext());

        // Inflate menu content
        LayoutInflater.from(getContext()).inflate(R.layout.sliding_menu_content, this, true);
        menuContent = findViewById(R.id.menuContent);
        mainMenuContainer = findViewById(R.id.mainMenuContainer);
        submenuContainer = findViewById(R.id.submenuContainer);
        closeButton = findViewById(R.id.closeMenuButton);
        backButton = findViewById(R.id.backButton);

        // Setup close button
        if (closeButton != null) {
            closeButton.setOnClickListener(v -> closeMenu());
        }

        // Setup back button
        if (backButton != null) {
            backButton.setOnClickListener(v -> navigateToMain());
        }

        // Setup category buttons
        setupCategoryButtons();

        // Setup settings UI elements
        setupSettingsUI();

        // Initially hidden
        setVisibility(View.GONE);
        if (menuContent != null) {
            menuContent.setAlpha(0f);
        }
    }

    private void setupCategoryButtons() {
        View btnProvider = findViewById(R.id.btnProvider);
        View btnBehavior = findViewById(R.id.btnBehavior);
        View btnAppearance = findViewById(R.id.btnAppearance);

        if (btnProvider != null) {
            btnProvider.setOnClickListener(v -> navigateToPage(MenuPage.PROVIDER));
        }
        if (btnBehavior != null) {
            btnBehavior.setOnClickListener(v -> navigateToPage(MenuPage.BEHAVIOR));
        }
        if (btnAppearance != null) {
            btnAppearance.setOnClickListener(v -> navigateToPage(MenuPage.APPEARANCE));
        }
    }

    private void setupSettingsUI() {
        // Provider settings
        providerSpinner = findViewById(R.id.providerSpinner);
        modelName = findViewById(R.id.modelName);
        apiKeyInput = findViewById(R.id.apiKeyInput);
        thinkingModeCheckbox = findViewById(R.id.thinkingModeCheckbox);
        endpointInput = findViewById(R.id.endpointInput);

        // Z.AI specific
        View zaiEndpointContainer = findViewById(R.id.zaiEndpointContainer);
        Spinner zaiEndpointSpinner = findViewById(R.id.zaiEndpointSpinner);

        // Behavior settings
        systemPromptInput = findViewById(R.id.systemPromptInput);
        temperatureSeekBar = findViewById(R.id.temperatureSeekBar);
        temperatureLabel = findViewById(R.id.temperatureLabel);
        maxTokensInput = findViewById(R.id.maxTokensInput);
        streamingSwitch = findViewById(R.id.streamingSwitch);

        // Appearance settings
        livingBackgroundSwitch = findViewById(R.id.livingBackgroundSwitch);

        // Setup z.ai endpoint spinner
        if (zaiEndpointSpinner != null) {
            ArrayAdapter<String> endpointAdapter = new ArrayAdapter<>(
                    getContext(),
                    android.R.layout.simple_spinner_item,
                    SettingsManager.ZAI_ENDPOINTS);
            endpointAdapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
            zaiEndpointSpinner.setAdapter(endpointAdapter);

            // Set current endpoint
            String currentEndpoint = settingsManager.getZaiEndpoint();
            for (int i = 0; i < SettingsManager.ZAI_ENDPOINTS.length; i++) {
                if (SettingsManager.ZAI_ENDPOINTS[i].equals(currentEndpoint)) {
                    zaiEndpointSpinner.setSelection(i);
                    break;
                }
            }

            zaiEndpointSpinner.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
                @Override
                public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                    String endpoint = SettingsManager.ZAI_ENDPOINTS[position];
                    settingsManager.setZaiEndpoint(endpoint);
                    // Update endpoint URL display
                    if (endpointInput != null) {
                        String url = SettingsManager.getZaiEndpointUrl(endpoint);
                        if (!url.isEmpty()) {
                            endpointInput.setText(url);
                        }
                    }
                }

                @Override
                public void onNothingSelected(AdapterView<?> parent) {
                }
            });
        }

        // Setup provider spinner
        if (providerSpinner != null) {
            ArrayAdapter<String> adapter = new ArrayAdapter<>(
                    getContext(),
                    android.R.layout.simple_spinner_item,
                    SettingsManager.PROVIDERS);
            adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
            providerSpinner.setAdapter(adapter);

            // Set current provider
            String currentProvider = settingsManager.getActiveProvider();
            for (int i = 0; i < SettingsManager.PROVIDERS.length; i++) {
                if (SettingsManager.PROVIDERS[i].equalsIgnoreCase(currentProvider)) {
                    providerSpinner.setSelection(i);
                    break;
                }
            }

            // Show/hide z.ai options based on current provider
            updateZaiVisibility(currentProvider, zaiEndpointContainer);

            providerSpinner.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
                @Override
                public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                    String provider = SettingsManager.PROVIDERS[position];
                    settingsManager.setActiveProvider(provider);

                    // Show/hide z.ai endpoint options
                    updateZaiVisibility(provider, zaiEndpointContainer);

                    // Update endpoint URL to default for this provider
                    if (endpointInput != null) {
                        endpointInput.setText(SettingsManager.getDefaultApiUrl(provider));
                    }
                    // Update model display to first model for this provider
                    String[] models = SettingsManager.getModelsForProvider(provider);
                    if (modelName != null && models.length > 0) {
                        modelName.setText(models[0]);
                        settingsManager.setModel(models[0]);
                    }
                }

                @Override
                public void onNothingSelected(AdapterView<?> parent) {
                }
            });
        }

        // Setup model selector (click to show model picker)
        View modelSelector = findViewById(R.id.modelSelector);
        if (modelSelector != null) {
            modelSelector.setOnClickListener(v -> showModelPicker());
        }

        // Setup API key input
        if (apiKeyInput != null) {
            apiKeyInput.setText(settingsManager.getApiKey());
            apiKeyInput.addTextChangedListener(new SimpleTextWatcher() {
                @Override
                public void afterTextChanged(Editable s) {
                    settingsManager.setApiKey(s.toString());
                }
            });
        }

        // Setup thinking mode checkbox
        if (thinkingModeCheckbox != null) {
            thinkingModeCheckbox.setChecked(settingsManager.isThinkingEnabled());
            thinkingModeCheckbox.setOnCheckedChangeListener(
                    (buttonView, isChecked) -> settingsManager.setThinkingEnabled(isChecked));
        }

        // Setup endpoint input
        if (endpointInput != null) {
            endpointInput.setText(settingsManager.getApiUrl());
            endpointInput.addTextChangedListener(new SimpleTextWatcher() {
                @Override
                public void afterTextChanged(Editable s) {
                    settingsManager.setApiUrl(s.toString());
                }
            });
        }

        // Setup system prompt
        if (systemPromptInput != null) {
            systemPromptInput.setText(settingsManager.getSystemPrompt());
            systemPromptInput.addTextChangedListener(new SimpleTextWatcher() {
                @Override
                public void afterTextChanged(Editable s) {
                    settingsManager.setSystemPrompt(s.toString());
                }
            });
        }

        // Setup temperature slider
        if (temperatureSeekBar != null && temperatureLabel != null) {
            int progress = (int) (settingsManager.getTemperature() * 10);
            temperatureSeekBar.setProgress(progress);
            temperatureLabel.setText(String.format("Temperature: %.1f", settingsManager.getTemperature()));

            temperatureSeekBar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
                @Override
                public void onProgressChanged(SeekBar seekBar, int progress, boolean fromUser) {
                    float temp = progress / 10f;
                    temperatureLabel.setText(String.format("Temperature: %.1f", temp));
                    if (fromUser) {
                        settingsManager.setTemperature(temp);
                    }
                }

                @Override
                public void onStartTrackingTouch(SeekBar seekBar) {
                }

                @Override
                public void onStopTrackingTouch(SeekBar seekBar) {
                }
            });
        }

        // Setup max tokens
        if (maxTokensInput != null) {
            maxTokensInput.setText(String.valueOf(settingsManager.getMaxTokens()));
            maxTokensInput.addTextChangedListener(new SimpleTextWatcher() {
                @Override
                public void afterTextChanged(Editable s) {
                    try {
                        int tokens = Integer.parseInt(s.toString());
                        settingsManager.setMaxTokens(tokens);
                    } catch (NumberFormatException e) {
                        // Ignore invalid input
                    }
                }
            });
        }

        // Setup streaming switch
        if (streamingSwitch != null) {
            streamingSwitch.setChecked(settingsManager.isStreamingEnabled());
            streamingSwitch.setOnCheckedChangeListener(
                    (buttonView, isChecked) -> settingsManager.setStreamingEnabled(isChecked));
        }

        // Setup living background switch
        if (livingBackgroundSwitch != null) {
            livingBackgroundSwitch.setChecked(settingsManager.isLivingBackgroundEnabled());
            livingBackgroundSwitch.setOnCheckedChangeListener(
                    (buttonView, isChecked) -> settingsManager.setLivingBackgroundEnabled(isChecked));
        }

        // Setup debug mode switch
        Switch debugModeSwitch = findViewById(R.id.debugModeSwitch);
        if (debugModeSwitch != null) {
            debugModeSwitch.setChecked(settingsManager.isDebugModeEnabled());
            debugModeSwitch.setOnCheckedChangeListener(
                    (buttonView, isChecked) -> settingsManager.setDebugModeEnabled(isChecked));
        }

        // Update model name display
        if (modelName != null) {
            modelName.setText(settingsManager.getModel());
        }
    }

    private void updateZaiVisibility(String provider, View zaiEndpointContainer) {
        if (zaiEndpointContainer == null)
            return;

        boolean isZai = SettingsManager.isZaiProvider(provider);
        zaiEndpointContainer.setVisibility(isZai ? View.VISIBLE : View.GONE);
    }

    private void showModelPicker() {
        String provider = settingsManager.getActiveProvider();
        String[] models = SettingsManager.getModelsForProvider(provider);

        android.app.AlertDialog.Builder builder = new android.app.AlertDialog.Builder(getContext());
        builder.setTitle("Select Model");
        builder.setItems(models, (dialog, which) -> {
            String selectedModel = models[which];
            settingsManager.setModel(selectedModel);
            if (modelName != null) {
                modelName.setText(selectedModel);
            }
        });
        builder.show();
    }

    public void setSettingsManager(SettingsManager manager) {
        this.settingsManager = manager;
        // Reload UI with new settings
        loadSettingsToUI();
    }

    private void loadSettingsToUI() {
        if (settingsManager == null)
            return;

        if (apiKeyInput != null)
            apiKeyInput.setText(settingsManager.getApiKey());
        if (endpointInput != null)
            endpointInput.setText(settingsManager.getApiUrl());
        if (modelName != null)
            modelName.setText(settingsManager.getModel());
        if (thinkingModeCheckbox != null)
            thinkingModeCheckbox.setChecked(settingsManager.isThinkingEnabled());
        if (systemPromptInput != null)
            systemPromptInput.setText(settingsManager.getSystemPrompt());
        if (temperatureSeekBar != null)
            temperatureSeekBar.setProgress((int) (settingsManager.getTemperature() * 10));
        if (maxTokensInput != null)
            maxTokensInput.setText(String.valueOf(settingsManager.getMaxTokens()));
        if (streamingSwitch != null)
            streamingSwitch.setChecked(settingsManager.isStreamingEnabled());
        if (livingBackgroundSwitch != null)
            livingBackgroundSwitch.setChecked(settingsManager.isLivingBackgroundEnabled());
    }

    @Override
    protected void onSizeChanged(int w, int h, int oldw, int oldh) {
        super.onSizeChanged(w, h, oldw, oldh);

        anchorY = h - 40f;
        screenWidth = w;

        float dx = w - ANCHOR_X;
        float dy = anchorY;
        float diagonalDistance = (float) Math.sqrt(dx * dx + dy * dy);
        float desktopFormula = Math.max(w, h) * 1.8f;
        maxRadius = Math.max(diagonalDistance * 1.1f, desktopFormula);
    }

    public void setMenuButton(View button) {
        this.menuButton = button;
    }

    public void openMenu() {
        if (currentState == MenuState.OPEN)
            return;

        currentState = MenuState.ANIMATING;
        setVisibility(View.VISIBLE);

        currentPage = MenuPage.MAIN;
        pageProgress = 0f;
        setupContainers();
        loadSettingsToUI();

        animateMenuButton(true);
        spring.setTarget(1.0f);
        startAnimation();

        if (listener != null) {
            listener.onMenuOpened();
        }
    }

    public void closeMenu() {
        if (currentState == MenuState.CLOSED)
            return;

        currentState = MenuState.ANIMATING;
        animateMenuButton(false);
        spring.setTarget(0.0f);
        startAnimation();
    }

    public void toggleMenu() {
        if (currentState == MenuState.OPEN || spring.getTarget() > 0.5f) {
            closeMenu();
        } else {
            openMenu();
        }
    }

    private void animateMenuButton(boolean toClose) {
        if (menuButton == null)
            return;

        ImageButton btn = (ImageButton) menuButton;

        btn.animate()
                .scaleX(0.0f)
                .scaleY(0.0f)
                .setDuration(150)
                .setInterpolator(new DecelerateInterpolator())
                .setListener(new AnimatorListenerAdapter() {
                    @Override
                    public void onAnimationEnd(Animator animation) {
                        btn.setImageResource(toClose ? R.drawable.ic_close : R.drawable.ic_menu);
                        btn.animate()
                                .scaleX(1.0f)
                                .scaleY(1.0f)
                                .setDuration(150)
                                .setInterpolator(new DecelerateInterpolator())
                                .setListener(null)
                                .start();
                    }
                })
                .start();
    }

    private void startAnimation() {
        if (animator != null && animator.isRunning()) {
            animator.cancel();
        }

        animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(16);
        animator.setRepeatCount(ValueAnimator.INFINITE);

        animator.addUpdateListener(animation -> {
            boolean stillAnimating = spring.update();

            float progress = spring.getPosition();
            if (menuContent != null) {
                if (progress > 0.2f) {
                    float contentAlpha = Math.min(1f, (progress - 0.2f) / 0.3f);
                    menuContent.setAlpha(contentAlpha);
                } else {
                    menuContent.setAlpha(0f);
                }
            }

            invalidate();

            if (!stillAnimating) {
                animation.cancel();

                if (spring.getPosition() < 0.01f) {
                    setVisibility(View.GONE);
                    currentState = MenuState.CLOSED;
                    if (listener != null) {
                        listener.onMenuClosed();
                    }
                } else {
                    currentState = MenuState.OPEN;
                }
            }
        });

        animator.start();
    }

    public void navigateToPage(MenuPage page) {
        if (page == currentPage || page == MenuPage.MAIN)
            return;

        currentPage = page;
        updateSubmenuContent(page);
        startPageTransition(true);

        if (listener != null) {
            listener.onPageChanged(page);
        }
    }

    public void navigateToMain() {
        if (currentPage == MenuPage.MAIN)
            return;
        startPageTransition(false);
    }

    private void startPageTransition(boolean forward) {
        if (pageAnimator != null && pageAnimator.isRunning()) {
            pageAnimator.cancel();
        }

        float startProgress = pageProgress;
        float endProgress = forward ? 1f : 0f;

        pageAnimator = ValueAnimator.ofFloat(startProgress, endProgress);
        pageAnimator.setDuration(250);
        pageAnimator.setInterpolator(new DecelerateInterpolator());

        pageAnimator.addUpdateListener(animation -> {
            pageProgress = (float) animation.getAnimatedValue();

            if (mainMenuContainer != null) {
                mainMenuContainer.setTranslationX(-screenWidth * pageProgress);
                mainMenuContainer.setAlpha(1f - pageProgress);
            }

            if (submenuContainer != null) {
                submenuContainer.setTranslationX(screenWidth * (1f - pageProgress));
                submenuContainer.setAlpha(pageProgress);
            }
        });

        pageAnimator.addListener(new AnimatorListenerAdapter() {
            @Override
            public void onAnimationEnd(Animator animation) {
                if (!forward) {
                    currentPage = MenuPage.MAIN;
                    if (submenuContainer != null) {
                        submenuContainer.setVisibility(View.GONE);
                    }
                }
            }

            @Override
            public void onAnimationStart(Animator animation) {
                if (forward && submenuContainer != null) {
                    submenuContainer.setVisibility(View.VISIBLE);
                }
            }
        });

        pageAnimator.start();
    }

    private void updateSubmenuContent(MenuPage page) {
        TextView submenuTitle = findViewById(R.id.submenuTitle);

        View providerContent = findViewById(R.id.providerContent);
        View behaviorContent = findViewById(R.id.behaviorContent);
        View appearanceContent = findViewById(R.id.appearanceContent);

        if (providerContent != null)
            providerContent.setVisibility(View.GONE);
        if (behaviorContent != null)
            behaviorContent.setVisibility(View.GONE);
        if (appearanceContent != null)
            appearanceContent.setVisibility(View.GONE);

        if (submenuTitle != null) {
            switch (page) {
                case PROVIDER:
                    submenuTitle.setText("Provider & Model");
                    if (providerContent != null)
                        providerContent.setVisibility(View.VISIBLE);
                    break;
                case BEHAVIOR:
                    submenuTitle.setText("Behavior");
                    if (behaviorContent != null)
                        behaviorContent.setVisibility(View.VISIBLE);
                    break;
                case APPEARANCE:
                    submenuTitle.setText("Appearance");
                    if (appearanceContent != null)
                        appearanceContent.setVisibility(View.VISIBLE);
                    break;
                default:
                    submenuTitle.setText("Settings");
            }
        }
    }

    private void setupContainers() {
        if (mainMenuContainer != null) {
            mainMenuContainer.setVisibility(View.VISIBLE);
            mainMenuContainer.setTranslationX(0);
            mainMenuContainer.setAlpha(1f);
        }
        if (submenuContainer != null) {
            submenuContainer.setVisibility(View.GONE);
            submenuContainer.setTranslationX(screenWidth);
            submenuContainer.setAlpha(0f);
        }
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);

        float progress = spring.getPosition();
        if (progress < 0.01f)
            return;

        float currentRadius = maxRadius * progress;
        float alpha = 0.98f * Math.min(progress, 1.0f);

        int colorWithAlpha = Color.argb(
                (int) (alpha * 255),
                Color.red(backgroundColor),
                Color.green(backgroundColor),
                Color.blue(backgroundColor));
        circlePaint.setColor(colorWithAlpha);

        canvas.drawCircle(ANCHOR_X, anchorY, currentRadius, circlePaint);
    }

    @Override
    protected void dispatchDraw(Canvas canvas) {
        onDraw(canvas);
        super.dispatchDraw(canvas);
    }

    public boolean isMenuOpen() {
        return currentState == MenuState.OPEN || spring.getTarget() > 0.5f;
    }

    public boolean isOpen() {
        return isMenuOpen();
    }

    public MenuPage getCurrentPage() {
        return currentPage;
    }

    public float getProgress() {
        return spring.getPosition();
    }

    public void setListener(MenuListener listener) {
        this.listener = listener;
    }

    @Override
    protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        if (animator != null)
            animator.cancel();
        if (pageAnimator != null)
            pageAnimator.cancel();
    }

    // Simple TextWatcher implementation
    private abstract class SimpleTextWatcher implements TextWatcher {
        @Override
        public void beforeTextChanged(CharSequence s, int start, int count, int after) {
        }

        @Override
        public void onTextChanged(CharSequence s, int start, int before, int count) {
        }
    }
}