# Research: Advice Panel Implementation

## Technology Decisions

### Pane Architecture
**Decision**: Extend existing pane system with new AdvicePane
**Rationale**: The codebase already has a well-established pane system in `src/pane.rs` with a trait-based approach. Adding a new pane follows existing patterns and maintains consistency.

### LLM Integration
**Decision**: Extend existing LLM configuration with advice_llm_model setting
**Rationale**: The codebase already has LLM integration via `src/llm.rs` and `LlmConfig` in `src/config.rs`. Extending this is more efficient than creating a separate system.

### UI Framework
**Decision**: Continue using Ratatui for the new advice panel
**Rationale**: The entire application is built on Ratatui 0.29. Maintaining consistency is crucial for the terminal-first architecture.

### State Management
**Decision**: Use existing shared state architecture via LlmSharedState
**Rationale**: The project uses lock-free concurrent data structures (scc crate) for performance. Advice panel state should follow this pattern.

### Configuration
**Decision**: Extend existing Config structure with advice-specific settings
**Rationale**: Configuration is already handled through `src/config.rs` with JSON support. Adding advice_llm_model follows the existing pattern.

## Architecture Considerations

### Key Integration Points
1. **Pane System**: Add new `Advice` variant to `PaneId` enum
2. **Key Handling**: Add Ctrl+L handler in main.rs key event processing
3. **Configuration**: Extend `LlmConfig` with advice-specific model field
4. **State Management**: Add advice-related state to `LlmSharedState`
5. **Rendering**: Implement full-screen advice panel using Ratatui components

### Performance Considerations
- **LLM Calls**: Must be async to avoid blocking UI
- **State Updates**: Use existing shared state for thread safety
- **Memory**: Advice content should be cached and limited to prevent memory bloat
- **Response Time**: LLM calls may be slow; need loading states and timeouts

## Code Patterns to Follow

### Existing Pane Pattern
```rust
pub struct AdvicePane {
    visible: bool,
    // advice panel specific state
}

impl Pane for AdvicePane {
    fn title(&self) -> String { /* ... */ }
    fn render(&self, /* ... */) { /* ... */ }
    fn handle_event(&mut self, /* ... */) -> bool { /* ... */ }
    // other required methods
}
```

### LLM Integration Pattern
Follow existing `LlmClient` pattern with async methods and proper error handling.

### Configuration Pattern
Extend existing JSON configuration structure with new advice-specific fields.

## Unknowns Resolved

### Full-Screen Panel Implementation
**Resolution**: Existing pane system already supports full-screen rendering through the main layout system. The advice panel can be implemented similarly to the Help pane.

### Chat Interface
**Resolution**: Can be implemented as a text input widget within the advice panel, similar to how other panes handle user input.

### Navigation Keys
**Resolution**: Existing pane system already handles standard vim-like navigation (j, k, etc.). The advice panel can reuse these patterns.

### LLM Configuration Extension
**Resolution**: The existing `LlmConfig` structure can be extended with an `advice_model` field while maintaining backward compatibility.

## Alternatives Considered

### Separate Advice System
**Rejected Because**: Would duplicate LLM integration and configuration management code. Existing system is well-designed and extensible.

### Custom Widget Framework
**Rejected Because**: Ratatui already provides all necessary widgets. Custom framework would add complexity and maintenance burden.

### Separate Configuration File
**Rejected Because**: Would break existing configuration patterns and make the system harder to maintain.

## Best Practices Identified

### Error Handling
- Use `Result` types for all fallible operations
- Follow existing error propagation patterns
- Display user-friendly error messages in the UI

### Async Operations
- Use Tokio for all LLM operations
- Ensure UI remains responsive during long-running operations
- Implement proper cancellation handling

### State Management
- Use existing shared state for thread safety
- Follow lock-free patterns with scc crate
- Keep state updates atomic and consistent

### Testing Strategy
- Unit tests for individual components
- Integration tests for LLM interactions
- UI tests for pane rendering and user interactions

## Performance Requirements

### Response Times
- **Panel Open**: Must be instant (<100ms)
- **Initial Advice**: Should appear within 2-3 seconds (LLM dependent)
- **Chat Responses**: Should appear within 2-3 seconds (LLM dependent)
- **Navigation**: Must be instant (<16ms for 60fps)

### Memory Usage
- **Advice Cache**: Limit to prevent memory bloat
- **Chat History**: Keep reasonable limit (e.g., last 10 messages)
- **LLM State**: Reuse existing shared state patterns

## Security Considerations

### API Key Management
- Follow existing OpenAI API key patterns
- Support environment variables and config file
- Never log or display API keys

### Data Privacy
- Code diffs may contain sensitive information
- Ensure LLM provider handles data appropriately
- Consider adding privacy notice to users

## Integration Dependencies

### Existing Code Dependencies
- `src/pane.rs` - Pane trait and registry
- `src/llm.rs` - LLM client integration
- `src/config.rs` - Configuration management
- `src/ui.rs` - Main UI application structure
- `src/main.rs` - Key event handling

### New Dependencies Required
- None - all functionality can be built with existing dependencies

### External Dependencies
- OpenAI API (or configured LLM provider)
- Git repository access (already available)

## Implementation Strategy

### Phase 1: Core Infrastructure
1. Extend configuration with advice_llm_model
2. Add AdvicePane to pane system
3. Implement basic advice panel rendering
4. Add Ctrl+L key binding

### Phase 2: LLM Integration
1. Implement advice generation logic
2. Add chat interface functionality
3. Implement help system for advice panel
4. Add proper error handling

### Phase 3: Polish and Testing
1. Add comprehensive tests
2. Optimize performance
3. Improve error messages and user feedback
4. Documentation updates