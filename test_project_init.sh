#!/bin/bash
# Test script to verify project manifest system

echo "=== Testing ARULA Project Manifest System ==="
echo

# Build the project first
echo "Building ARULA CLI..."
cargo build --release --quiet

echo "✓ Build successful!"
echo

# Run the CLI and verify the main menu shows
echo "Starting ARULA CLI..."
echo "You should:"
echo "1. Press ESC or wait for the menu to appear"
echo "2. Select '📝 Create Project Manifest' with arrow keys"
echo "3. Press Enter"
echo
echo "Expected behavior:"
echo "- No description dialog should appear"
echo "- A message about creating PROJECT.manifest is sent to AI"
echo "- The AI will help create a single PROJECT.manifest file that provides:"
echo "  * Quick AI understanding of the entire project"
echo "  * Project metadata (name, type, language, framework)"
echo "  * Essence (TL;DR for AI)"
echo "  * Structure (core components, key files)"
echo "  * Patterns & conventions"
echo "  * Dependencies and workflow"
echo "  * AI assistance notes"
echo
echo "The manifest will be saved as 'PROJECT.manifest' in the current directory."
echo "This allows future AI interactions to quickly understand the project"
echo "without scanning through multiple files, saving significant time."
echo
echo "Press any key to start ARULA CLI..."
read -n 1

cargo run --release -- --help