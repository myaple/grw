# Data Model: Advice Panel

## Core Entities

### AdvicePanel
The main advice panel entity that manages the display and interaction of AI-generated code advice.

**Fields:**
- `visible: bool` - Whether the advice panel is currently visible
- `mode: AdviceMode` - Current mode of the advice panel (Viewing, Chatting, Help)
- `content: AdviceContent` - The current content being displayed
- `chat_input: String` - Current chat input text
- `scroll_offset: usize` - Current scroll position for content
- `loading: bool` - Whether an LLM request is in progress
- `error: Option<String>` - Current error message, if any

### AdviceContent
Represents the different types of content that can be displayed in the advice panel.

**Variants:**
- `InitialAdvice(InitialAdviceData)` - Initial 3 actionable improvements
- `ChatMessage(ChatMessageData)` - Chat conversation with LLM
- `Help(HelpContent)` - Help documentation
- `Error(ErrorMessage)` - Error display

### InitialAdviceData
Contains the initial AI-generated advice for code improvements.

**Fields:**
- `improvements: Vec<AdviceImprovement>` - List of 3 actionable improvements
- `timestamp: DateTime<Utc>` - When the advice was generated
- `diff_hash: String` - Hash of the diff that was analyzed
- `model_used: String` - LLM model used for generation

### AdviceImprovement
A single actionable improvement suggestion.

**Fields:**
- `id: String` - Unique identifier for the improvement
- `title: String` - Brief title of the improvement
- `description: String` - Detailed description of the improvement
- `priority: ImprovementPriority` - Priority level (Low, Medium, High)
- `category: ImprovementCategory` - Type of improvement (Code Quality, Performance, Security, etc.)

### ChatMessageData
Represents a chat conversation with the LLM.

**Fields:**
- `messages: Vec<ChatMessage>` - List of messages in the conversation
- `current_input: String` - Current user input being typed
- `cursor_position: usize` - Cursor position in the input field
- `context_diff_hash: String` - Hash of the diff context for this chat

### ChatMessage
A single message in the chat conversation.

**Fields:**
- `id: String` - Unique identifier for the message
- `role: MessageRole` - Role (User or Assistant)
- `content: String` - Message content
- `timestamp: DateTime<Utc>` - When the message was sent
- `model_used: Option<String>` - LLM model used for assistant messages

### HelpContent
Help documentation for the advice panel.

**Fields:**
- `sections: Vec<HelpSection>` - Help sections
- `current_section: usize` - Currently displayed section index

### HelpSection
A section in the help documentation.

**Fields:**
- `title: String` - Section title
- `content: String` - Section content
- `key_bindings: Vec<KeyBinding>` - Relevant key bindings for this section

### KeyBinding
Represents a key binding with its description.

**Fields:**
- `key: String` - Key combination (e.g., "Ctrl+L", "/")
- `description: String` - Description of what the key does
- `context: String` - Context where this key is active

## Enums

### AdviceMode
Operating modes of the advice panel.

**Variants:**
- `Viewing` - Viewing advice content
- `Chatting` - Entering chat input
- `Help` - Viewing help documentation

### MessageRole
Role of a chat message.

**Variants:**
- `User` - Message from the user
- `Assistant` - Message from the AI assistant

### ImprovementPriority
Priority levels for improvements.

**Variants:**
- `Low` - Low priority improvement
- `Medium` - Medium priority improvement
- `High` - High priority improvement

### ImprovementCategory
Categories for improvements.

**Variants:**
- `CodeQuality` - Code style, readability, maintainability
- `Performance` - Performance optimizations
- `Security` - Security improvements
- `BugFix` - Bug fixes
- `Feature` - Feature enhancements
- `Documentation` - Documentation improvements

## Configuration Extensions

### AdviceConfig
Extended configuration for the advice panel.

**Fields:**
- `enabled: bool` - Whether the advice panel is enabled
- `advice_model: String` - LLM model to use for advice generation
- `max_improvements: usize` - Maximum number of improvements to generate (default: 3)
- `chat_history_limit: usize` - Maximum number of chat messages to keep (default: 10)
- `timeout_seconds: u64` - Timeout for LLM requests in seconds (default: 30)
- `context_lines: usize` - Number of context lines to include in diff (default: 50)

## State Management

### AdviceState
Shared state for the advice panel.

**Fields:**
- `active_advice: Option<InitialAdviceData>` - Currently active initial advice
- `chat_sessions: HashMap<String, ChatMessageData>` - Active chat sessions keyed by diff hash
- `cached_advice: HashMap<String, InitialAdviceData>` - Cached advice keyed by diff hash
- `last_request_time: Option<DateTime<Utc>>` - Last time an LLM request was made
- `request_count: u64` - Number of LLM requests made

## LLM Request Types

### AdviceRequest
Request structure for generating initial advice.

**Fields:**
- `diff: String` - The git diff to analyze
- `context: String` - Additional context about the codebase
- `max_improvements: usize` - Maximum number of improvements to generate
- `model: String` - LLM model to use

### ChatRequest
Request structure for chat follow-up questions.

**Fields:**
- `question: String` - User's question
- `conversation_history: Vec<ChatMessage>` - Previous conversation context
- `original_diff: String` - Original diff context
- `model: String` - LLM model to use

## Validation Rules

### AdvicePanel Validation
- Must have at least one improvement when in InitialAdvice mode
- Chat input must not exceed reasonable length limits (e.g., 1000 characters)
- Scroll offset must be within valid range for content length
- Must have valid diff hash when generating advice

### Configuration Validation
- `max_improvements` must be between 1 and 10
- `chat_history_limit` must be between 1 and 100
- `timeout_seconds` must be between 5 and 300
- `context_lines` must be between 10 and 200

### LLM Request Validation
- Diff content must not be empty
- Model name must be a valid supported model
- Chat question must not be empty
- Request size must fit within LLM context limits

## Relationships

### Parent-Child Relationships
- `AdvicePanel` contains `AdviceContent`
- `AdviceContent` can be `InitialAdviceData` or `ChatMessageData`
- `InitialAdviceData` contains multiple `AdviceImprovement`
- `ChatMessageData` contains multiple `ChatMessage`
- `HelpContent` contains multiple `HelpSection`

### State Relationships
- `AdviceState` manages cached advice for performance
- Chat sessions are keyed by diff hash for context preservation
- Advice requests include diff hash for cache lookup

### Configuration Relationships
- `AdviceConfig` extends the main application configuration
- LLM model settings inherit from global LLM configuration with advice-specific overrides

## Security Considerations

### Data Privacy
- Code diffs may contain sensitive information
- Chat conversations may contain proprietary information
- All data sent to LLM should be considered potentially exposed

### Input Validation
- All user input must be sanitized before sending to LLM
- Diff content must be validated for size and content
- Configuration values must be within safe bounds

### Error Handling
- LLM API failures should not crash the application
- Network timeouts should be handled gracefully
- Invalid responses should be displayed as errors, not crashes