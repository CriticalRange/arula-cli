# Visioneer - Hybrid Desktop Automation Tool

Visioneer is a powerful hybrid perception-and-action tool for ARULA CLI that combines screen capture, OCR (Optical Character Recognition), vision-language models, and UI automation capabilities. It enables AI agents to see, understand, and interact with desktop applications and games.

## Features

### 🔍 **Perception Layer**
- **Screen Capture**: High-quality screen capture with region selection and multiple output formats
- **OCR Engine**: Tesseract-based text extraction with confidence scores and bounding boxes
- **Vision-Language Model Integration**: AI-powered UI analysis and understanding using Ollama VLM models

### 🎯 **Action Layer**
- **Click Actions**: Coordinate, text-based, pattern-based, and element-based clicking
- **Text Input**: Type text with configurable delays and clearing options
- **Hotkey Execution**: Send keyboard shortcuts and combinations
- **Navigation**: Mouse movement and directional navigation
- **Wait Conditions**: Smart waiting for UI elements and state changes

### ⚡ **Core Capabilities**
- **Target Selection**: Works with window titles, process IDs, or handles
- **Region-based Operations**: Focus on specific screen areas for precision
- **Multi-format Support**: Base64 encoding, file saving, and memory buffers
- **Cross-platform Architecture**: Designed for Windows with extensible platform support
- **Ollama Integration**: Uses local Ollama VLM models for intelligent UI analysis

## Usage

### Basic Structure

```json
{
  "target": "window_title_or_process_id",
  "action": {
    "type": "action_type",
    "parameters": "..."
  },
  "ocr_config": { ... },
  "vlm_config": {
    "model": "llava",
    "endpoint": "http://localhost:11434",
    "provider": "ollama",
    "max_tokens": 1024,
    "temperature": 0.7,
    "detail": "medium"
  }
}
```

### VLM Configuration

The `vlm_config` parameter allows you to configure the vision-language model for UI analysis:

```json
{
  "vlm_config": {
    "model": "llava",                    // Ollama model name (required)
    "endpoint": "http://localhost:11434", // Ollama endpoint (optional, defaults to localhost:11434)
    "provider": "ollama",                // Provider type (optional, defaults to ollama)
    "max_tokens": 1024,                  // Maximum tokens in response (optional)
    "temperature": 0.7,                   // Temperature for response generation (optional)
    "detail": "medium"                    // Detail level for analysis: "low", "medium", "high" (optional)
  }
}
```

### Supported Ollama VLM Models

Visioneer works with any Ollama model that supports vision capabilities. Some popular options include:

- **llava**: A general-purpose vision-language model
- **llava-llama3**: LLaVA based on Llama 3
- **bakllava**: A compact vision model
- **moondream**: A lightweight vision model

To use a model with Ollama:

```bash
# Pull a vision model
ollama pull llava

# Start Ollama server
ollama serve
```

### Action Types

#### 1. Screen Capture
```json
{
  "type": "Capture",
  "region": {
    "x": 0,
    "y": 0,
    "width": 800,
    "height": 600
  },
  "save_path": "/path/to/screenshot.png",
  "encode_base64": true
}
```

#### 2. Text Extraction
```json
{
  "type": "ExtractText",
  "region": {
    "x": 100,
    "y": 100,
    "width": 400,
    "height": 200
  },
  "language": "eng"
}
```

#### 3. UI Analysis with VLM
```json
{
  "type": "Analyze",
  "query": "What buttons are visible and how can I click them?",
  "region": null
}
```

When using the Analyze action with a VLM configuration, Visioneer will:
1. Capture the specified screen region
2. Send the image and query to the configured Ollama VLM
3. Parse the response to extract UI elements and suggestions
4. Return structured analysis results

#### 4. Click Actions
```json
{
  "type": "Click",
  "target": {
    "type": "Coordinates",
    "x": 150,
    "y": 250
  },
  "button": "Left",
  "double_click": false
}
```

#### 5. Text Input
```json
{
  "type": "Type",
  "text": "Hello, World!",
  "clear_first": true,
  "delay_ms": 50
}
```

#### 6. Hotkey Execution
```json
{
  "type": "Hotkey",
  "keys": ["ctrl", "c"],
  "hold_ms": 100
}
```

#### 7. Wait Conditions
```json
{
  "type": "WaitFor",
  "condition": {
    "type": "Text",
    "text": "Complete",
    "appears": true
  },
  "timeout_ms": 10000,
  "check_interval_ms": 500
}
```

#### 8. Navigation
```json
{
  "type": "Navigate",
  "direction": "Down",
  "distance": 100,
  "steps": 3
}
```

## Examples

### Example 1: Analyze UI with Ollama VLM

```json
{
  "target": "Calculator",
  "vlm_config": {
    "model": "llava",
    "endpoint": "http://localhost:11434",
    "provider": "ollama",
    "temperature": 0.7
  },
  "action": {
    "type": "Analyze",
    "query": "Identify all buttons on this calculator interface and their positions",
    "region": null
  }
}
```

### Example 2: Find and Click a Button Using VLM

```json
{
  "target": "Chrome",
  "vlm_config": {
    "model": "llava-llama3",
    "endpoint": "http://localhost:11434",
    "provider": "ollama"
  },
  "action": {
    "type": "Analyze",
    "query": "Find the download button and provide its coordinates",
    "region": {
      "x": 0,
      "y": 0,
      "width": 1200,
      "height": 800
    }
  }
}
```

### Example 3: Automate Notepad Text Entry

```json
{
  "target": "Untitled - Notepad",
  "action": {
    "type": "Type",
    "text": "Hello from Visioneer!",
    "clear_first": true,
    "delay_ms": 100
  }
}
```

### Example 4: Extract Text from Calculator

```json
{
  "target": "Calculator",
  "action": {
    "type": "ExtractText",
    "region": {
      "x": 50,
      "y": 100,
      "width": 200,
      "height": 50
    }
  },
  "ocr_config": {
    "engine": "tesseract",
    "language": "eng",
    "confidence_threshold": 0.8
  }
}
```

### Example 5: Smart Button Clicking

```json
{
  "target": "Web Browser",
  "action": {
    "type": "Click",
    "target": {
      "type": "Text",
      "text": "Submit",
      "region": {
        "x": 0,
        "y": 0,
        "width": 1200,
        "height": 800
      }
    },
    "button": "Left"
  }
}
```

### Example 6: UI Analysis with Vision Model

```json
{
  "target": "Application",
  "action": {
    "type": "Analyze",
    "query": "Find the login button and describe how to interact with it",
    "region": null,
    "context": "User needs to log in"
  },
  "vlm_config": {
    "model": "gpt-4-vision",
    "max_tokens": 500,
    "detail": "high"
  }
}
```

### Example 7: Complex Automation Workflow

```json
{
  "target": "Application",
  "action": {
    "type": "WaitFor",
    "condition": {
      "type": "Text",
      "text": "Ready",
      "appears": true
    },
    "timeout_ms": 30000,
    "check_interval_ms": 1000
  }
}
```

Followed by:
```json
{
  "target": "Application",
  "action": {
    "type": "Click",
    "target": {
      "type": "Text",
      "text": "Start"
    }
  }
}
```

## Configuration

### OCR Configuration
```json
{
  "engine": "tesseract",
  "language": "eng+fra+spa",
  "confidence_threshold": 0.75,
  "preprocessing": {
    "grayscale": true,
    "threshold": 128,
    "denoise": true,
    "scale_factor": 2.0
  }
}
```

### Vision-Language Model Configuration
```json
{
  "model": "gpt-4-vision",
  "max_tokens": 1000,
  "temperature": 0.1,
  "detail": "high"
}
```

## Response Formats

### Capture Result
```json
{
  "success": true,
  "action_type": "capture",
  "data": {
    "image_path": "/tmp/screenshot.png",
    "base64_data": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA...",
    "width": 1920,
    "height": 1080,
    "format": "rgba",
    "region": {
      "x": 0,
      "y": 0,
      "width": 800,
      "height": 600
    }
  },
  "execution_time_ms": 150,
  "metadata": {
    "target": "Untitled - Notepad",
    "platform": "windows"
  }
}
```

### Text Extraction Result
```json
{
  "success": true,
  "action_type": "extract_text",
  "data": {
    "text": "Extracted text content",
    "confidence": 0.92,
    "words": [
      {
        "text": "Extracted",
        "confidence": 0.95,
        "bbox": {
          "x": 10,
          "y": 10,
          "width": 60,
          "height": 15
        }
      }
    ],
    "language": "eng",
    "region": {
      "x": 100,
      "y": 100,
      "width": 400,
      "height": 200
    }
  },
  "execution_time_ms": 850,
  "metadata": {
    "target": "Application",
    "platform": "windows"
  }
}
```

### UI Analysis Result
```json
{
  "success": true,
  "action_type": "analyze",
  "data": {
    "analysis": "I can see a login form with username and password fields...",
    "elements": [
      {
        "element_type": "button",
        "text": "Login",
        "bbox": {
          "x": 150,
          "y": 200,
          "width": 80,
          "height": 30
        },
        "confidence": 0.88,
        "attributes": {
          "enabled": true,
          "color": "blue"
        }
      }
    ],
    "confidence": 0.85,
    "suggestions": [
      "Click the Login button at coordinates (150, 200)",
      "The form appears ready for input"
    ]
  },
  "execution_time_ms": 2500,
  "metadata": {
    "target": "Application",
    "platform": "windows"
  }
}
```

## Best Practices

### 1. **Target Selection**
- Use specific window titles when possible
- For applications with dynamic titles, consider using process IDs
- Test window detection before complex automation

### 2. **Region Operations**
- Use regions to improve OCR accuracy and reduce processing time
- Start with larger regions and narrow down as needed
- Account for window resizing and movement

### 3. **Error Handling**
- Always check the `success` field in responses
- Monitor `execution_time_ms` for performance optimization
- Use appropriate timeout values for wait operations

### 4. **OCR Optimization**
- Preprocess images for better text recognition
- Choose appropriate languages and confidence thresholds
- Consider image resolution and scaling

### 5. **Action Sequencing**
- Use wait conditions between actions to ensure UI readiness
- Add small delays for animations and loading
- Verify action success before proceeding

## Limitations

- **Windows-First**: Currently optimized for Windows with planned cross-platform expansion
- **Accessibility**: Requires that target applications are accessible to screen capture
- **Performance**: OCR and vision model operations can be resource-intensive
- **Network**: VLM integration requires internet connectivity for cloud models

## Security Considerations

- **Permissions**: Ensure appropriate permissions for screen capture and UI automation
- **Privacy**: Be mindful of capturing sensitive information in screenshots
- **Access Control**: Limit access to applications containing confidential data

## Troubleshooting

### Common Issues

1. **Window Not Found**
   - Verify window title spelling and case sensitivity
   - Check if the application is running and visible
   - Try using process ID instead of window title

2. **OCR Accuracy Issues**
   - Increase image resolution or scale factor
   - Adjust preprocessing parameters
   - Verify language settings match text content

3. **Click Failures**
   - Ensure target window has focus
   - Verify coordinates are within window bounds
   - Check for overlapping windows or dialogs

4. **Performance Problems**
   - Use smaller regions for OCR and analysis
   - Reduce VLM detail level for faster processing
   - Implement caching for repeated operations

## Integration

### Using with ARULA CLI

Visioneer is automatically available in ARULA CLI's tool registry and can be used by AI agents for desktop automation tasks:

```bash
# Start ARULA CLI
cargo run

# Ask the AI to automate a task
"Can you open Notepad and type 'Hello Visioneer!'?"
```

The AI will automatically use the Visioneer tool to:
1. Find the Notepad window
2. Take necessary actions to open it if needed
3. Type the requested text
4. Provide feedback on the operation

## Future Enhancements

- **Cross-Platform Support**: macOS and Linux compatibility
- **Advanced OCR**: Multiple engine support and custom training
- **Computer Vision**: Object detection and pattern recognition
- **Recording & Playback**: Record automation sequences for replay
- **Integration APIs**: Direct programming interface for custom workflows
- **Cloud Processing**: Optional cloud-based OCR and vision services