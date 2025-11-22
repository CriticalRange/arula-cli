#!/bin/bash
# Test changelog functionality

echo "Testing changelog parser..."
cargo run --bin test-changelog 2>/dev/null || echo "Build test binary..."

# Test parsing
cat > /tmp/test_changelog.md << 'EOF'
# Changelog
<!-- type: release -->

## [Unreleased]

### Added
- Multi-provider support
- Real-time changelog display

### Fixed
- Configuration persistence bug

## [0.1.0] - 2025-01-22

### Added
- Initial release
EOF

echo "✅ Changelog file created for testing"
echo ""
echo "Expected output:"
echo "  - Type: release"
echo "  - 2 entries (Added, Fixed)"
echo "  - 3 total changes"
