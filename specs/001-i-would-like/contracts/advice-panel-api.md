# Advice Panel API Contracts

## Overview
This document defines the internal API contracts for the advice panel functionality within GRW.

## 1. Advice Panel State Management

### 1.1 Advice Panel Creation
**Endpoint**: `AdvicePanel::new()`
**Method**: Constructor
**Description**: Creates a new advice panel instance

**Request**: None
**Response**: `AdvicePanel`

**Validation Rules**:
- Must initialize with default state
- Must not be visible by default
- Must start in Viewing mode

### 1.2 Panel Visibility
**Endpoint**: `AdvicePanel::set_visible(bool)`
**Method**: Instance method
**Description**: Sets the visibility of the advice panel

**Request**:
```json
{
  "visible": true
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must handle visibility changes gracefully
- Must preserve state when hiding/showing
- Must trigger appropriate UI updates

### 1.3 Panel Rendering
**Endpoint**: `AdvicePanel::render(Frame, &App, Rect, &GitRepo)`
**Method**: Instance method
**Description**: Renders the advice panel to the terminal

**Request**:
- `frame`: Ratatui frame
- `app`: Application state reference
- `area`: Render area rectangle
- `git_repo`: Git repository reference

**Response**: `Result<(), Box<dyn std::error::Error>>`

**Validation Rules**:
- Must handle all rendering errors gracefully
- Must not crash on invalid state
- Must respect the provided render area
- Must show appropriate content based on current mode

## 2. LLM Integration Contracts

### 2.1 Generate Initial Advice
**Endpoint**: `LlmClient::generate_advice(AdviceRequest)`
**Method**: Async instance method
**Description**: Generates initial code improvement advice

**Request**:
```json
{
  "diff": "string",
  "context": "string",
  "max_improvements": 3,
  "model": "gpt-4o-mini"
}
```

**Response**: `Result<InitialAdviceData, String>`

**Validation Rules**:
- `diff` must not be empty
- `max_improvements` must be between 1 and 10
- `model` must be a valid supported model
- Must return exactly `max_improvements` improvements
- Each improvement must have valid title and description

### 2.2 Chat Follow-up
**Endpoint**: `LlmClient::chat_followup(ChatRequest)`
**Method**: Async instance method
**Description**: Handles follow-up chat questions

**Request**:
```json
{
  "question": "string",
  "conversation_history": [],
  "original_diff": "string",
  "model": "gpt-4o-mini"
}
```

**Response**: `Result<ChatMessage, String>`

**Validation Rules**:
- `question` must not be empty
- `conversation_history` must be valid array of messages
- `original_diff` must match the initial diff context
- Response must be a valid assistant message

### 2.3 Help Content
**Endpoint**: `AdvicePanel::get_help_content()`
**Method**: Instance method
**Description**: Returns help content for the advice panel

**Request**: None
**Response**: `HelpContent`

**Validation Rules**:
- Must include all available key bindings
- Must include navigation instructions
- Must include mode-specific help

## 3. Configuration Management

### 3.1 Extended Configuration
**Endpoint**: `Config::advice_config()`
**Method**: Instance method
**Description**: Returns advice-specific configuration

**Request**: None
**Response**: `AdviceConfig`

**Validation Rules**:
- Must return valid configuration with sensible defaults
- Must merge with global LLM configuration appropriately
- Must validate all configuration values

### 3.2 Configuration Validation
**Endpoint**: `AdviceConfig::validate()`
**Method**: Instance method
**Description**: Validates advice configuration

**Request**: None
**Response**: `Result<(), String>`

**Validation Rules**:
- `max_improvements` must be 1-10
- `chat_history_limit` must be 1-100
- `timeout_seconds` must be 5-300
- `context_lines` must be 10-200

## 4. Event Handling

### 4.1 Key Events
**Endpoint**: `AdvicePanel::handle_event(&AppEvent)`
**Method**: Instance method
**Description**: Handles keyboard events for the advice panel

**Request**:
```json
{
  "event_type": "Key",
  "key_code": "Char",
  "key_char": "l",
  "modifiers": ["CONTROL"]
}
```

**Response**: `bool` (true if handled, false if not)

**Validation Rules**:
- Must handle Ctrl+L to show/hide panel
- Must handle / to enter chat mode
- Must handle ? to show help
- Must handle standard navigation keys (j, k, etc.)
- Must return false for unhandled keys

### 4.2 Mode Transitions
**Endpoint**: `AdvicePanel::set_mode(AdviceMode)`
**Method**: Instance method
**Description**: Changes the operating mode of the advice panel

**Request**:
```json
{
  "mode": "Chatting"
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must validate mode transitions
- Must preserve appropriate state during transitions
- Must update UI accordingly

## 5. State Management

### 5.1 Shared State Updates
**Endpoint**: `LlmSharedState::update_advice_state(AdviceState)`
**Method**: Instance method
**Description**: Updates the shared advice state

**Request**:
```json
{
  "active_advice": "InitialAdviceData",
  "chat_sessions": {},
  "cached_advice": {},
  "last_request_time": "2025-09-27T10:00:00Z",
  "request_count": 1
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must use thread-safe operations
- Must handle concurrent access correctly
- Must validate state before updates

### 5.2 Cache Management
**Endpoint**: `LlmSharedState::cache_advice(String, InitialAdviceData)`
**Method**: Instance method
**Description**: Caches advice for a specific diff

**Request**:
```json
{
  "diff_hash": "abc123",
  "advice": "InitialAdviceData"
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must use diff hash as cache key
- Must enforce cache size limits
- Must handle cache invalidation

## 6. Error Handling

### 6.1 LLM Error Handling
**Endpoint**: `AdvicePanel::handle_llm_error(String)`
**Method**: Instance method
**Description**: Handles LLM-related errors

**Request**:
```json
{
  "error": "API request failed: timeout"
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must display user-friendly error messages
- Must preserve panel state during errors
- Must allow retry after errors

### 6.2 Validation Errors
**Endpoint**: `AdvicePanel::handle_validation_error(String)`
**Method**: Instance method
**Description**: Handles validation errors

**Request**:
```json
{
  "error": "Invalid configuration: max_improvements must be 1-10"
}
```

**Response**: `Result<(), String>`

**Validation Rules**:
- Must show specific error details
- Must suggest corrective actions
- Must not crash the application

## 7. Data Contracts

### 7.1 Advice Improvement
```json
{
  "id": "string",
  "title": "string",
  "description": "string",
  "priority": "Medium",
  "category": "CodeQuality"
}
```

**Validation Rules**:
- All string fields must be non-empty
- Priority must be valid enum value
- Category must be valid enum value

### 7.2 Chat Message
```json
{
  "id": "string",
  "role": "User",
  "content": "string",
  "timestamp": "2025-09-27T10:00:00Z",
  "model_used": null
}
```

**Validation Rules**:
- `id` must be unique
- `role` must be valid enum value
- `content` must not be empty
- `timestamp` must be valid ISO 8601

### 7.3 Help Section
```json
{
  "title": "string",
  "content": "string",
  "key_bindings": [
    {
      "key": "Ctrl+L",
      "description": "Toggle advice panel",
      "context": "Global"
    }
  ]
}
```

**Validation Rules**:
- `title` and `content` must not be empty
- `key_bindings` must be valid array
- Each key binding must have valid key and description

## 8. Performance Requirements

### 8.1 Response Times
- Panel visibility toggle: <100ms
- Initial advice generation: <10s
- Chat response generation: <10s
- Key handling response: <16ms

### 8.2 Resource Limits
- Chat history: max 100 messages
- Cached advice: max 50 entries
- Single message length: max 2000 characters
- Diff context: max 1000 lines

### 8.3 Concurrency
- Must handle multiple concurrent LLM requests
- Must prevent race conditions in shared state
- Must cancel stale requests appropriately

## 9. Security Requirements

### 9.1 Data Protection
- Must not log sensitive code content
- Must use secure API key handling
- Must validate all user inputs

### 9.2 API Security
- Must use HTTPS for all LLM API calls
- Must handle API key rotation gracefully
- Must respect rate limits

## 10. Testing Requirements

### 10.1 Unit Tests
- Must test all public methods with 80%+ coverage
- Must test error handling scenarios
- Must test edge cases and boundary conditions

### 10.2 Integration Tests
- Must test LLM integration with mocked responses
- Must test UI rendering with different states
- Must test configuration validation

### 10.3 Contract Tests
- Must test all API contracts defined here
- Must test request/response validation
- Must test error responses