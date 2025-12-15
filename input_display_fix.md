# Input Display Fix - Summary

## Problem
The user's input line was being overlapped and hidden when the AI was thinking or using tools. The thinking/status area would expand and interfere with the input display, making it impossible to see what the user was typing.

## Solution Implemented

### 1. **Fixed Viewport Layout System** (Inspired by Codex)

Changed the layout from a simple sequential stack to a proper separation:

```rust
// Always reserve space for input and info at the bottom
let input_height = 1;
let info_height = 1;
let bottom_reserved = input_height + info_height;

// Status can only use space ABOVE the reserved bottom area
let status_max_height = area.height.saturating_sub(bottom_reserved);
let status_height = self.status_height().min(status_max_height);
```

### 2. **Input Always at Bottom**

The input area is now always positioned at a fixed location:
- Uses `chunks.len().saturating_sub(2)` to place input 2nd from bottom
- Info line is always at the very bottom (`chunks.len() - 1`)
- Status/content only occupies space above these reserved areas

### 3. **Visual Separation with Borders**

Added clear visual separators:
- **Input**: Top border (`Borders::TOP`) to separate from status above
- **Status**: Bottom border (`Borders::BOTTOM`) to separate from input
- This creates a clear visual hierarchy

### 4. **Proper Area Management**

- Clear the input area before rendering to prevent artifacts
- Ensure cursor position accounts for borders
- Bounds checking prevents cursor from going outside valid area

### 5. **Dynamic Height Calculation**

Updated `required_viewport_height()` to:
- Always reserve 2 lines for input + info at bottom
- Allow status to use remaining space above
- Prevent viewport from exceeding screen height

## Key Improvements

1. **No More Overlap**: Input is always visible and never overlapped by AI status
2. **Clear Separation**: Visual borders clearly distinguish between status, input, and info areas
3. **Consistent Layout**: Input stays at the bottom regardless of status changes
4. **Responsive Design**: Layout adapts to terminal size while preserving input visibility
5. **Inspired by Codex**: Uses similar principles to Codex's flex layout system

## Testing Scenarios

The fix handles these cases properly:
- AI thinking while user is typing
- Multiple tools running simultaneously
- Terminal resize events
- Very small terminal windows
- Rapid status changes

The user's input now remains visible and accessible at all times, with clear visual separation from AI activity above.