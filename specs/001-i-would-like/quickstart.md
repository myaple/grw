# Advice Panel Quickstart

This quickstart guide demonstrates how to use the new advice panel feature in GRW.

## Prerequisites

- GRW installed and configured
- OpenAI API key set (either in config file or OPENAI_API_KEY environment variable)
- Git repository with some changes to analyze

## Setup

### 1. Configure LLM for Advice

Add the advice panel configuration to your `~/.config/grw/config.json`:

```json
{
  "llm": {
    "provider": "openai",
    "model": "gpt-4o-mini",
    "api_key": "your-api-key-here",
    "base_url": "https://api.openai.com/v1"
  },
  "advice": {
    "enabled": true,
    "advice_model": "gpt-4o-mini",
    "max_improvements": 3,
    "chat_history_limit": 10,
    "timeout_seconds": 30,
    "context_lines": 50
  }
}
```

### 2. Start GRW

Navigate to your git repository and start GRW:

```bash
cd /path/to/your/repo
grw
```

## Basic Usage

### 1. Open Advice Panel

Press `Ctrl+L` to open the advice panel. The panel will:

- Take over the entire screen
- Automatically analyze your current git diff
- Generate 3 actionable improvement suggestions

### 2. View Initial Advice

The advice panel will show:

1. **Initial Advice Section**: 3 specific improvements with:
   - Title and priority level
   - Detailed description
   - Category (Code Quality, Performance, Security, etc.)

2. **Status Bar**: Shows loading state and model used

3. **Navigation Help**: Brief key reference at bottom

### 3. Navigate Advice Content

Use standard vim-like navigation:

- `j` / `↓` - Scroll down
- `k` / `↑` - Scroll up
- `g g` - Go to top
- `Shift+G` - Go to bottom
- `PageDown` / `PageUp` - Page navigation

### 4. Chat Follow-up Questions

Press `/` to enter chat mode:

1. A chat input line appears at the bottom
2. Type your question about the code or suggestions
3. Press `Enter` to send
4. View the AI response in the conversation

**Example questions:**
- "Can you explain the first improvement in more detail?"
- "What are the performance implications of this change?"
- "Can you suggest a code example for this improvement?"

### 5. Get Help

Press `?` to view the help system:

- See all available key bindings
- Learn about different panel modes
- Understand chat functionality
- Get troubleshooting tips

### 6. Exit Advice Panel

Press `Ctrl+L` again to close the advice panel and return to the normal GRW interface.

## Advanced Usage

### 1. Different Contexts

The advice panel can analyze:

- **Current Working Directory**: All uncommitted changes
- **Specific Commit**: When using commit picker mode
- **Staged Changes**: When viewing staged files

### 2. Chat Features

- **Conversation History**: Maintains context within the session
- **Follow-up Questions**: Ask for clarification on suggestions
- **Code Examples**: Request specific code implementations
- **Alternative Approaches**: Explore different solutions

### 3. Customization

Configure the advice behavior:

```json
{
  "advice": {
    "max_improvements": 5,  // Show more suggestions
    "timeout_seconds": 60,   // Longer timeout for complex analysis
    "context_lines": 100    // Include more context in analysis
  }
}
```

## Common Scenarios

### Scenario 1: Code Quality Review

1. Make changes to your code
2. Press `Ctrl+L` to open advice panel
3. Review the 3 suggested improvements
4. Use chat to ask: "Can you prioritize these by impact?"
5. Apply the most important suggestions

### Scenario 2: Performance Optimization

1. After making performance-related changes
2. Open advice panel with `Ctrl+L`
3. Look for Performance category improvements
4. Chat: "What are the expected performance gains?"
5. Implement the suggested optimizations

### Scenario 3: Security Review

1. Before committing security-sensitive changes
2. Open advice panel with `Ctrl+L`
3. Focus on Security category improvements
4. Chat: "Are there any security vulnerabilities I missed?"
5. Address the security concerns

### Scenario 4: Learning and Best Practices

1. While working on unfamiliar code
2. Open advice panel with `Ctrl+L`
3. Review all suggested improvements
4. Chat: "Can you explain why these are best practices?"
5. Learn from the AI explanations

## Troubleshooting

### Issue: Advice Panel Doesn't Open

**Solution**:
- Check that LLM configuration is correct
- Verify API key is set
- Ensure advice panel is enabled in config

### Issue: "No Improvements Found"

**Solution**:
- Make sure you have actual changes in your git diff
- Check that the changes are significant enough for analysis
- Try increasing `context_lines` in configuration

### Issue: Slow Response Times

**Solution**:
- Check your internet connection
- Try a faster model (e.g., gpt-4o-mini instead of gpt-4o)
- Increase `timeout_seconds` in configuration
- Reduce `max_improvements` to decrease processing time

### Issue: Chat Not Working

**Solution**:
- Press `/` to enter chat mode
- Ensure you have an active advice session
- Check that the LLM service is available
- Try refreshing the advice panel

### Issue: Navigation Keys Not Working

**Solution**:
- Make sure the advice panel has focus
- Check that you're not in chat mode (press Esc to exit chat)
- Verify the advice panel is visible

## Tips and Best Practices

### 1. Effective Usage

- Use the advice panel regularly during development
- Combine AI suggestions with your own judgment
- Ask follow-up questions to deepen understanding
- Use the advice panel as a learning tool

### 2. Configuration Tips

- Start with default settings and adjust as needed
- Use faster models for quick iterations
- Increase context for complex codebases
- Set appropriate timeouts for your workflow

### 3. Performance Optimization

- Cache advice for similar diffs
- Limit chat history to maintain performance
- Use appropriate model sizes for your needs
- Monitor response times and adjust settings

## Integration with Development Workflow

### 1. Before Committing

1. Stage your changes
2. Open advice panel (`Ctrl+L`)
3. Review improvement suggestions
4. Chat about any concerns
5. Apply relevant improvements
6. Commit with confidence

### 2. Code Reviews

1. Open advice panel for the changes
2. Use AI to identify potential issues
3. Chat about edge cases
4. Document AI suggestions in review comments
5. Make improvements before merging

### 3. Learning and Improvement

1. Regularly use advice panel during development
2. Ask questions about unfamiliar patterns
3. Learn from AI explanations
4. Apply best practices consistently
5. Improve code quality over time

## Next Steps

- Explore advanced configuration options
- Integrate with your team's development workflow
- Share feedback on advice quality
- Contribute to improving the advice system

## Getting Help

- Press `?` in the advice panel for immediate help
- Check the GRW documentation for general usage
- Review configuration options for customization
- Report issues or suggestions for improvement