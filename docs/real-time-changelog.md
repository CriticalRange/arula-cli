# Real-Time Changelog Display

## Overview

ARULA CLI now displays a real-time changelog on startup, fetched from the remote git repository (`origin/main`). The changelog automatically detects the build type (Release, Custom, or Development) and shows the most recent changes.

## Features

### 1. Remote Changelog Fetching
- Automatically fetches `CHANGELOG.md` from `origin/main` branch
- Falls back to local `CHANGELOG.md` if git fetch fails
- Uses default changelog if no file exists

### 2. Build Type Detection
The system automatically detects three types of builds:

**Release Build**: Official ARULA repository
- Detected by checking git remote URL for official repository patterns
- Shows: `📦 Release`

**Custom Build**: Forked or modified repository
- Detected when git remote points to a different repository
- Shows: `🔧 Custom Build`

**Development Build**: Local development without git
- Detected when git is not available
- Shows: `⚙️ Development`

### 3. Type Header in CHANGELOG.md
You can manually specify the changelog type:

```markdown
# Changelog
<!-- type: release -->

## [Unreleased]
...
```

Supported types:
- `<!-- type: release -->` - Official release
- `<!-- type: custom -->` - Custom/forked build
- No type comment - Auto-detected from git

## CHANGELOG.md Format

### Structure

```markdown
# Changelog
<!-- type: release -->

All notable changes to ARULA CLI will be documented in this file.

## [Unreleased]

### Added
- New feature one
- New feature two

### Changed
- Modified behavior
- Updated dependency

### Fixed
- Bug fix one
- Bug fix two

### Removed
- Deprecated feature

### Security
- Security patch

### Deprecated
- Feature marked for removal

## [0.1.0] - 2025-01-22

### Added
- Initial release
```

### Change Categories

Each category gets a unique emoji:
- ✨ **Added** - New features
- 🔄 **Changed** - Changes in existing functionality
- 🐛 **Fixed** - Bug fixes
- 🗑️ **Removed** - Removed features
- 🔒 **Security** - Security improvements
- ⚠️ **Deprecated** - Soon-to-be removed features

## Display Behavior

### Startup Banner
```
 █████╗ ██████╗ ██╗   ██╗██╗      █████╗
 ██╔══██╗██╔══██╗██║   ██║██║     ██╔══██╗
 ███████║██████╔╝██║   ██║██║     ███████║
 ██╔══██║██╔══██╗██║   ██║██║     ██╔══██║
 ██║  ██║██║  ██║╚██████╔╝███████╗██║  ██║
 ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝

    Autonomous AI Command-Line Interface

📋 What's New (📦 Release)
  ✨ Multi-provider configuration support - switch between AI providers
  ✨ Custom provider support with full control over settings
  ✨ Real-time changelog display in startup banner
  🔄 Configuration structure now supports multiple providers
  🐛 Provider switching now preserves configurations
```

### Limits
- Shows up to 5 most recent changes from `[Unreleased]` section
- Stops at the first versioned release section
- Displays in order: Added → Changed → Fixed → Removed → Security → Deprecated

## Implementation Details

### File: `src/changelog.rs`

**Key Types:**
```rust
pub enum ChangelogType {
    Release,
    Custom,
    Development,
}

pub struct ChangelogEntry {
    pub title: String,          // "Added", "Changed", etc.
    pub changes: Vec<String>,   // List of changes
}

pub struct Changelog {
    pub changelog_type: ChangelogType,
    pub entries: Vec<ChangelogEntry>,
}
```

**Key Methods:**
```rust
// Fetch from remote git
Changelog::fetch_from_remote() -> Result<Self>

// Fetch from local file
Changelog::fetch_local() -> Result<Self>

// Parse changelog content
Changelog::parse(content: &str) -> Self

// Get recent changes (limited count)
changelog.get_recent_changes(max_items: usize) -> Vec<String>

// Detect build type from git
Changelog::detect_build_type() -> ChangelogType

// Get type label for display
changelog.get_type_label() -> &str
```

### Git Commands Used

**Fetch remote changelog:**
```bash
git show origin/main:CHANGELOG.md
```

**Detect repository:**
```bash
git remote get-url origin
```

## Offline Behavior

When git is not available or network fails:
1. Tries to fetch from `origin/main`
2. Falls back to local `CHANGELOG.md`
3. Uses default changelog if no file exists
4. Shows build type as "Development"

## Customization for Forks

If you fork ARULA CLI:

1. **Add type header to your CHANGELOG.md:**
```markdown
# Changelog
<!-- type: custom -->
```

2. **Update changes in [Unreleased] section:**
```markdown
## [Unreleased]

### Added
- Your custom feature
- Another modification

### Changed
- Customized behavior
```

3. The startup banner will automatically show:
```
📋 What's New (🔧 Custom Build)
  ✨ Your custom feature
  ✨ Another modification
  🔄 Customized behavior
```

## Migration from Tips

**Before:**
```
💡 Tips:
  • Type your message and press Enter to send
  • Use Shift+Enter for new lines
  • Paste multi-line content
  • End line with \ to continue
  • Cursor changed to blinking block
```

**After:**
```
📋 What's New (📦 Release)
  ✨ Multi-provider configuration support
  ✨ Custom provider support
  🔄 Configuration structure updated
  🐛 Provider switching fixes
  📋 Real-time changelog display
```

Users can still learn these tips from the help command (`/help`) or documentation.

## Benefits

1. **Real-Time Updates**: Users see latest changes immediately
2. **Build Transparency**: Clear indication of release vs custom builds
3. **Change Visibility**: Important updates shown on every startup
4. **Fork-Friendly**: Custom builds can show their own changes
5. **Offline Support**: Graceful fallback to local changelog

## Future Enhancements

Potential improvements:
- [ ] Cache changelog to avoid git calls on every startup
- [ ] Show version number in changelog header
- [ ] Add release date for latest version
- [ ] Show "X days since last update"
- [ ] Interactive changelog viewer (`/changelog` command)
- [ ] Changelog filtering by category
- [ ] Changelog search functionality

## Testing

Run the changelog tests:
```bash
cargo test --lib changelog
```

Manual testing:
```bash
# Test with release type
echo '<!-- type: release -->' > CHANGELOG.md

# Test with custom type
echo '<!-- type: custom -->' > CHANGELOG.md

# Test build type detection
git remote get-url origin

# Test remote fetch
git show origin/main:CHANGELOG.md
```

## Troubleshooting

**Changelog not showing:**
- Check `CHANGELOG.md` exists in repository root
- Verify `[Unreleased]` section exists
- Ensure changes are under category headers (### Added, etc.)

**Wrong build type:**
- Check git remote: `git remote get-url origin`
- Add explicit type header: `<!-- type: custom -->`

**Git errors:**
- System falls back to local file automatically
- Check git is installed and repository has origin remote
- Verify network connectivity for remote fetch

## Example CHANGELOG.md

See the complete example in the repository root: [`CHANGELOG.md`](../CHANGELOG.md)
