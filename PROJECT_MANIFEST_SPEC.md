# Project Manifest Specification

The PROJECT.manifest file is a single, AI-optimized summary of the entire project.

## File Format
```
PROJECT_MANIFEST v1.0

# METADATA
name: <project_name>
type: <web_api|cli_tool|library|desktop_app|mobile_app|other>
language: <primary_language>
framework: <main_framework>
created: <YYYY-MM-DD>
last_updated: <YYYY-MM-DD>

# ESSENCE (TL;DR for AI)
purpose: <one_sentence_description>
architecture: <brief_architecture_overview>
key_technologies: <tech1,tech2,tech3>

# STRUCTURE
## Core Components
- <component_name>: <brief_description>

## Key Files
- <file_path>: <purpose>

## Entry Points
- main: <main_entry_file>
- api: <api_entry_file_if_applicable>
- cli: <cli_entry_file_if_applicable>

# PATTERNS & CONVENTIONS
## Naming
- files: <file_naming_convention>
- functions: <function_naming_convention>
- variables: <variable_naming_convention>

## Architecture Patterns
- <pattern1>: <where_used>
- <pattern2>: <where_used>

# DEPENDENCIES
## External Libraries
- <library>: <purpose>

## System Requirements
- <requirement>: <details>

# WORKFLOW
## How to Run
```bash
<command_to_run>
```

## How to Test
```bash
<command_to_test>
```

## How to Build
```bash
<command_to_build>
```

# DECISION LOG
## [YYYY-MM-DD] Decision: <title>
Context: <why_decision_was_made>
Result: <what_was_implemented>

# TODO & FUTURE
## Immediate
- <task1>
- <task2>

## Considered
- <feature1>: <brief_note>

# AI ASSISTANCE NOTES
## Common Tasks
- <task>: <how_to_approach>

## Gotchas
- <pitfall>: <how_to_avoid>

## Recent Changes
- [YYYY-MM-DD] <change_description>
```

## Benefits
1. **Single source of truth** - AI reads one file instead of scanning hundreds
2. **Quick context** - ESSENCE section gives immediate understanding
3. **Evolving with project** - Updated as code changes
4. **AI-optimized** - Structured for quick parsing
5. **Human-readable** - Developers can also understand and maintain it

## Usage
- AI reads PROJECT.manifest first
- Uses it to understand project structure
- References it for context when making changes
- Updates it when adding significant features