# Feature Specification: Advice Panel

**Feature Branch**: `001-i-would-like`
**Created**: 2025-09-27
**Status**: Draft
**Input**: User description: "i would like to add a pane to the tool. it should take over the entire screen. it is called the advice panel. when a user presses ctrl + l it should open the advice pane. it should have a dynamic help page activated with ? that details all of the navigation and hotkeys and all. i want the same navigation keys as the other panes. in this pane, i want it to take a configuration for an llm called advice_llm_model, building off the config for llm that is already there. as soon as the pane is opened, i want it to send a request to the llm with the entire diff of the current working directory or commit chosen, and ask for 3 actionable improvements to the code. then, when the user presses / it should open up a small chat line where the user can send follow up queries to the llm and receive back the response."

---

## ⚡ Quick Guidelines
- ✅ Focus on WHAT users need and WHY
- ❌ Avoid HOW to implement (no tech stack, APIs, code structure)
- 👥 Written for business stakeholders, not developers

### Section Requirements
- **Mandatory sections**: Must be completed for every feature
- **Optional sections**: Include only when relevant to the feature
- When a section doesn't apply, remove it entirely (don't leave as "N/A")

### For AI Generation
When creating this spec from a user prompt:
1. **Mark all ambiguities**: Use [NEEDS CLARIFICATION: specific question] for any assumption you'd need to make
2. **Don't guess**: If the prompt doesn't specify something (e.g., "login system" without auth method), mark it
3. **Think like a tester**: Every vague requirement should fail the "testable and unambiguous" checklist item
4. **Common underspecified areas**:
   - User types and permissions
   - Data retention/deletion policies
   - Performance targets and scale
   - Error handling behaviors
   - Integration requirements
   - Security/compliance needs

---

## User Scenarios & Testing *(mandatory)*

### Primary User Story
As a developer using GRW, I want to access an AI-powered advice panel that provides actionable code improvements based on my current changes, so I can receive intelligent guidance on improving my code quality and catch potential issues before committing.

### Acceptance Scenarios
1. **Given** I am viewing git changes in GRW, **When** I press Ctrl+L, **Then** the advice panel should open and occupy the entire screen
2. **Given** the advice panel is open, **When** it first loads, **Then** it should automatically send the current diff to the LLM and receive 3 actionable improvement suggestions
3. **Given** the advice panel is open, **When** I press /, **Then** a chat input line should appear allowing me to ask follow-up questions to the LLM
4. **Given** the advice panel is open, **When** I press ?, **Then** a help page should show all navigation keys and hotkeys available in the advice panel
5. **Given** I am in the advice panel, **When** I use standard navigation keys (j, k, etc.), **Then** I should be able to scroll through the advice content

### Edge Cases
- What happens when the LLM service is unavailable or returns an error?
- How does the system handle very large diffs that might exceed LLM context limits?
- What happens when the user presses / but there's no LLM configuration?
- How does the advice panel handle slow LLM response times?
- What happens when the user tries to open the advice panel while another full-screen pane is already open?

## Requirements *(mandatory)*

### Functional Requirements
- **FR-001**: System MUST provide a new advice panel that can be activated via Ctrl+L keybinding
- **FR-002**: System MUST display the advice panel in full-screen mode, replacing all other panes
- **FR-003**: System MUST automatically generate 3 actionable code improvement suggestions when the advice panel opens
- **FR-004**: System MUST send the current working directory diff or selected commit diff to the LLM for analysis
- **FR-005**: System MUST provide a chat interface activated by pressing / for follow-up user queries
- **FR-006**: System MUST display LLM responses in the advice panel for both initial suggestions and follow-up queries
- **FR-007**: System MUST provide contextual help via ? key showing all advice panel navigation and hotkeys
- **FR-008**: System MUST use the same navigation keys as other panes for consistency
- **FR-009**: System MUST support configuration for advice_llm_model, extending existing LLM configuration
- **FR-010**: System MUST handle LLM errors gracefully and display informative error messages

### Key Entities *(include if feature involves data)*
- **Advice Panel**: A full-screen UI component that displays LLM-generated code advice and interactive chat
- **LLM Configuration**: Extended configuration settings for the advice-specific LLM model and parameters
- **Chat Interface**: An input mechanism for users to send follow-up queries to the LLM
- **Help System**: Contextual help displaying navigation keys and hotkeys specific to the advice panel

---

## Review & Acceptance Checklist
*GATE: Automated checks run during main() execution*

### Content Quality
- [ ] No implementation details (languages, frameworks, APIs)
- [ ] Focused on user value and business needs
- [ ] Written for non-technical stakeholders
- [ ] All mandatory sections completed

### Requirement Completeness
- [ ] No [NEEDS CLARIFICATION] markers remain
- [ ] Requirements are testable and unambiguous
- [ ] Success criteria are measurable
- [ ] Scope is clearly bounded
- [ ] Dependencies and assumptions identified

---

## Execution Status
*Updated by main() during processing*

- [ ] User description parsed
- [ ] Key concepts extracted
- [ ] Ambiguities marked
- [ ] User scenarios defined
- [ ] Requirements generated
- [ ] Entities identified
- [ ] Review checklist passed

---