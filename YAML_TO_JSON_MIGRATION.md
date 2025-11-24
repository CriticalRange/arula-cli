# YAML to JSON Configuration Migration

## Overview
ARULA CLI has migrated from YAML configuration files to JSON configuration files for better performance, reduced dependencies, and improved cross-platform compatibility.

## Migration Details

### Changes Made

1. **File Format Change**
   - **Before**: `~/.arula/config.yaml`
   - **After**: `~/.arula/config.json`

2. **Serialization Library**
   - **Before**: `serde_yaml`
   - **After**: `serde_json`

3. **Automatic Migration**
   - The application automatically detects and migrates existing YAML configs to JSON on first run
   - Original YAML files are backed up and then removed after successful migration
   - Migration status messages are displayed to the user

### Benefits

1. **Reduced Dependencies**: YAML parsing has been removed as a primary dependency
2. **Better Performance**: JSON parsing is faster than YAML parsing
3. **Improved Reliability**: JSON has a simpler, more consistent specification
4. **Better IDE Support**: JSON has superior syntax highlighting and validation in most editors
5. **Version Control Friendly**: JSON produces more consistent diffs than YAML

### Migration Process

When ARULA CLI starts:

1. **Check for JSON config**: `~/.arula/config.json`
2. **If JSON exists**: Load it directly
3. **If JSON doesn't exist but YAML exists**:
   - Load YAML configuration
   - Convert to JSON format
   - Save as `~/.arula/config.json`
   - Display migration success message
   - Remove old YAML file
4. **If neither exists**: Create default JSON configuration

### Manual Migration (if needed)

If you want to manually migrate your configuration:

```bash
# Backup existing config
cp ~/.arula/config.yaml ~/.arula/config.yaml.backup

# Convert YAML to JSON (using jq or similar tool)
jq '.' ~/.arula/config.yaml > ~/.arula/config.json

# Verify the JSON config
cat ~/.arula/config.json

# Remove old YAML after verification
rm ~/.arula/config.yaml
```

### Configuration Format Comparison

**YAML Format (Old)**:
```yaml
active_provider: "openai"
providers:
  openai:
    model: "gpt-4"
    api_url: "https://api.openai.com/v1"
    api_key: "sk-test"
```

**JSON Format (New)**:
```json
{
  "active_provider": "openai",
  "providers": {
    "openai": {
      "model": "gpt-4",
      "api_url": "https://api.openai.com/v1",
      "api_key": "sk-test"
    }
  }
}
```

### Backward Compatibility

- The migration code will remain in the codebase temporarily to help users transition
- After a few releases, the YAML migration code can be safely removed
- All existing configuration data and functionality is preserved

### Code Changes

1. **src/utils/config.rs**: Updated to use `serde_json` instead of `serde_yaml`
2. **src/utils/chat.rs**: Updated test functions to use JSON serialization
3. **Cargo.toml**: Marked `serde_yaml` as temporary dependency for migration

### Testing

The migration functionality includes:
- Automatic detection of old YAML configs
- Graceful error handling for invalid YAML
- Verification that JSON conversion maintains all data
- Cleanup of old YAML files after successful migration

### Rollback Plan

If issues arise with the JSON migration:
1. Users can restore their YAML from backup
2. The migration code can be temporarily disabled
3. Manual YAML→JSON conversion is still possible using external tools

This migration maintains full backward compatibility while providing a smoother, more reliable configuration experience.