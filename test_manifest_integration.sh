#!/bin/bash
# Test script to verify PROJECT.manifest integration

echo "=== Testing PROJECT.manifest Integration ==="
echo

# Check if PROJECT.manifest exists
if [ -f "PROJECT.manifest" ]; then
    echo "✓ PROJECT.manifest found in current directory"
    echo "  File size: $(wc -l < PROJECT.manifest) lines"
    echo
    echo "First 10 lines of PROJECT.manifest:"
    head -10 PROJECT.manifest
    echo
else
    echo "✗ PROJECT.manifest not found in current directory"
    exit 1
fi

# Build the project
echo "Building ARULA CLI..."
cargo build --quiet 2>&1 | tail -5

if [ $? -eq 0 ]; then
    echo "✓ Build successful!"
    echo
    echo "To test the integration:"
    echo "1. Run: cargo run -- --debug"
    echo "2. Check debug output for 'Loaded PROJECT.manifest' message"
    echo "3. The manifest content should appear in the AI's system prompt"
    echo
    echo "The manifest provides AI with:"
    echo "  - Quick project understanding (first 30 seconds)"
    echo "  - Code navigation guide"
    echo "  - Development patterns and gotchas"
    echo "  - Testing and debugging tips"
    echo
else
    echo "✗ Build failed"
    exit 1
fi