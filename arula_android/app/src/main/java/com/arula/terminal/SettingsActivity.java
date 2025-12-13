package com.arula.terminal;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Bundle;
import android.text.InputType;
import android.util.Log;
import androidx.appcompat.app.AppCompatActivity;
import androidx.preference.EditTextPreference;
import androidx.preference.ListPreference;
import androidx.preference.Preference;
import androidx.preference.PreferenceFragmentCompat;
import androidx.preference.PreferenceManager;
import androidx.preference.SwitchPreferenceCompat;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Settings activity for Arula configuration
 */
public class SettingsActivity extends AppCompatActivity {
    private static final String TAG = "SettingsActivity";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.settings_activity);
        getSupportFragmentManager()
            .beginTransaction()
            .replace(R.id.settings, new SettingsFragment())
            .commit();

        if (getSupportActionBar() != null) {
            getSupportActionBar().setDisplayHomeAsUpEnabled(true);
        }
    }

    @Override
    public boolean onSupportNavigateUp() {
        onBackPressed();
        return true;
    }

    public static class SettingsFragment extends PreferenceFragmentCompat
            implements SharedPreferences.OnSharedPreferenceChangeListener {

        private SharedPreferences prefs;

        @Override
        public void onCreatePreferences(Bundle savedInstanceState, String rootKey) {
            setPreferencesFromResource(R.xml.root_preferences, rootKey);
            prefs = PreferenceManager.getDefaultSharedPreferences(requireContext());

            setupPreferences();
        }

        private void setupPreferences() {
            // Provider selection
            ListPreference providerPref = findPreference("active_provider");
            if (providerPref != null) {
                providerPref.setOnPreferenceChangeListener((preference, newValue) -> {
                    String provider = (String) newValue;
                    updateProviderVisibility(provider);
                    return true;
                });
                updateProviderVisibility(providerPref.getValue());
            }

            // API key preferences - hide password input
            setupPasswordInput("openai_api_key");
            setupPasswordInput("anthropic_api_key");
            setupPasswordInput("zai_api_key");
            setupPasswordInput("custom_api_key");

            // API URL preferences
            setupUrlValidation("openai_api_url", "https://api.openai.com/v1");
            setupUrlValidation("anthropic_api_url", "https://api.anthropic.com");
            setupUrlValidation("zai_api_url", "https://z.ai/api");
            setupUrlValidation("custom_api_url", "https://api.example.com/v1");

            // Model preferences
            setupModelList("openai_model", "openai");
            setupModelList("anthropic_model", "anthropic");
            setupModelList("zai_model", "zai");

            // Numeric preferences
            setupNumericInput("openai_max_tokens", 4096, 1, 32768);
            setupNumericInput("anthropic_max_tokens", 4096, 1, 32768);
            setupNumericInput("zai_max_tokens", 4096, 1, 32768);

            setupNumericInput("openai_temperature", 0.7f, 0.0f, 2.0f);
            setupNumericInput("anthropic_temperature", 0.7f, 0.0f, 2.0f);
            setupNumericInput("zai_temperature", 0.7f, 0.0f, 2.0f);

            // UI preferences
            setupThemeList("ui_theme");
            setupNumericInput("ui_font_size", 14, 8, 32);

            // Test connection button
            Preference testPref = findPreference("test_connection");
            if (testPref != null) {
                testPref.setOnPreferenceClickListener(preference -> {
                    testConnection();
                    return true;
                });
            }
        }

        private void setupPasswordInput(String key) {
            EditTextPreference pref = findPreference(key);
            if (pref != null) {
                pref.setOnBindEditTextListener(editText -> {
                    editText.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
                });
            }
        }

        private void setupUrlValidation(String key, String defaultUrl) {
            EditTextPreference pref = findPreference(key);
            if (pref != null) {
                pref.setOnBindEditTextListener(editText -> {
                    editText.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
                });

                pref.setOnPreferenceChangeListener((preference, newValue) -> {
                    String url = (String) newValue;
                    if (url.isEmpty()) return true;

                    if (!url.startsWith("http://") && !url.startsWith("https://")) {
                        pref.setSummary("Must start with http:// or https://");
                        return false;
                    }

                    pref.setSummary(url);
                    return true;
                });

                // Set initial summary
                String value = prefs.getString(key, defaultUrl);
                if (value != null) {
                    pref.setSummary(value);
                }
            }
        }

        private void setupModelList(String key, String provider) {
            ListPreference pref = findPreference(key);
            if (pref != null) {
                String[] models = getModelArray(provider);
                pref.setEntryValues(models);
                pref.setEntries(models);
            }
        }

        private String[] getModelArray(String provider) {
            switch (provider) {
                case "openai":
                    return new String[]{"gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"};
                case "anthropic":
                    return new String[]{"claude-3-opus-20240229", "claude-3-sonnet-20240229", "claude-3-haiku-20240307"};
                case "zai":
                    return new String[]{"glm-4", "glm-3-turbo"};
                default:
                    return new String[]{"gpt-3.5-turbo"};
            }
        }

        private void setupNumericInput(String key, Number defaultValue, Number min, Number max) {
            EditTextPreference pref = findPreference(key);
            if (pref != null) {
                pref.setOnBindEditTextListener(editText -> {
                    if (defaultValue instanceof Float) {
                        editText.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL);
                    } else {
                        editText.setInputType(InputType.TYPE_CLASS_NUMBER);
                    }
                });

                pref.setOnPreferenceChangeListener((preference, newValue) -> {
                    try {
                        String valueStr = (String) newValue;
                        if (valueStr.isEmpty()) {
                            newValue = defaultValue;
                            return true;
                        }

                        if (defaultValue instanceof Float) {
                            float value = Float.parseFloat(valueStr);
                            if (value < min.floatValue() || value > max.floatValue()) {
                                return false;
                            }
                        } else {
                            int value = Integer.parseInt(valueStr);
                            if (value < min.intValue() || value > max.intValue()) {
                                return false;
                            }
                        }

                        pref.setSummary(valueStr);
                        return true;
                    } catch (NumberFormatException e) {
                        return false;
                    }
                });

                // Set initial summary
                String value = prefs.getString(key, defaultValue.toString());
                if (value != null) {
                    pref.setSummary(value);
                }
            }
        }

        private void setupThemeList(String key) {
            ListPreference pref = findPreference(key);
            if (pref != null) {
                String[] themes = {"Light", "Dark", "System Default"};
                String[] values = {"light", "dark", "system"};
                pref.setEntryValues(values);
                pref.setEntries(themes);
            }
        }

        private void updateProviderVisibility(String provider) {
            boolean isOpenAI = "openai".equals(provider);
            boolean isAnthropic = "anthropic".equals(provider);
            boolean isZai = "zai".equals(provider);
            boolean isCustom = "custom".equals(provider);

            setPreferenceVisibility("openai_category", isOpenAI);
            setPreferenceVisibility("anthropic_category", isAnthropic);
            setPreferenceVisibility("zai_category", isZai);
            setPreferenceVisibility("custom_category", isCustom);
        }

        private void setPreferenceVisibility(String key, boolean visible) {
            Preference pref = findPreference(key);
            if (pref != null) {
                pref.setVisible(visible);
            }
        }

        private void testConnection() {
            // Build configuration JSON
            JSONObject config = new JSONObject();
            try {
                config.put("active_provider", prefs.getString("active_provider", "openai"));
                config.put("openai_api_key", prefs.getString("openai_api_key", ""));
                config.put("openai_api_url", prefs.getString("openai_api_url", "https://api.openai.com/v1"));
                config.put("openai_model", prefs.getString("openai_model", "gpt-4"));

                config.put("anthropic_api_key", prefs.getString("anthropic_api_key", ""));
                config.put("anthropic_api_url", prefs.getString("anthropic_api_url", "https://api.anthropic.com"));
                config.put("anthropic_model", prefs.getString("anthropic_model", "claude-3-opus-20240229"));

                config.put("zai_api_key", prefs.getString("zai_api_key", ""));
                config.put("zai_api_url", prefs.getString("zai_api_url", "https://z.ai/api"));
                config.put("zai_model", prefs.getString("zai_model", "glm-4"));

                // Test with ArulaNative
                ArulaNative.setConfig(config.toString());

                // Show test message
                // In a real implementation, this would test the actual connection
                Log.i(TAG, "Configuration updated: " + config.toString());

            } catch (JSONException e) {
                Log.e(TAG, "Failed to build configuration", e);
            }
        }

        @Override
        public void onResume() {
            super.onResume();
            prefs.registerOnSharedPreferenceChangeListener(this);
        }

        @Override
        public void onPause() {
            super.onPause();
            prefs.unregisterOnSharedPreferenceChangeListener(this);
        }

        @Override
        public void onSharedPreferenceChanged(SharedPreferences sharedPreferences, String key) {
            // Update configuration when preferences change
            updateConfiguration();
        }

        private void updateConfiguration() {
            // This would update the native configuration
            Log.d(TAG, "Updating configuration");
        }
    }
}