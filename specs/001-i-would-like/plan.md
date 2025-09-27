# Implementation Plan: Advice Panel

**Branch**: `001-i-would-like` | **Date**: 2025-09-27 | **Spec**: /specs/001-i-would-like/spec.md
**Input**: Feature specification from `/specs/001-i-would-like/spec.md`

## Execution Flow (/plan command scope)
```
1. Load feature spec from Input path
   → If not found: ERROR "No feature spec at {path}"
2. Fill Technical Context (scan for NEEDS CLARIFICATION)
   → Detect Project Type from file system structure or context (web=frontend+backend, mobile=app+api)
   → Set Structure Decision based on project type
3. Fill the Constitution Check section based on the content of the constitution document.
4. Evaluate Constitution Check section below
   → If violations exist: Document in Complexity Tracking
   → If no justification possible: ERROR "Simplify approach first"
   → Update Progress Tracking: Initial Constitution Check
5. Execute Phase 0 → research.md
   → If NEEDS CLARIFICATION remain: ERROR "Resolve unknowns"
6. Execute Phase 1 → contracts, data-model.md, quickstart.md, agent-specific template file (e.g., `CLAUDE.md` for Claude Code, `.github/copilot-instructions.md` for GitHub Copilot, `GEMINI.md` for Gemini CLI, `QWEN.md` for Qwen Code or `AGENTS.md` for opencode).
7. Re-evaluate Constitution Check section
   → If new violations: Refactor design, return to Phase 1
   → Update Progress Tracking: Post-Design Constitution Check
8. Plan Phase 2 → Describe task generation approach (DO NOT create tasks.md)
9. STOP - Ready for /tasks command
```

**IMPORTANT**: The /plan command STOPS at step 7. Phases 2-4 are executed by other commands:
- Phase 2: /tasks command creates tasks.md
- Phase 3-4: Implementation execution (manual or via tools)

## Summary
The advice panel feature adds a new full-screen AI-powered panel to GRW that provides actionable code improvement suggestions. When users press Ctrl+L, the panel automatically analyzes the current git diff and generates 3 specific improvements using an LLM. Users can then ask follow-up questions via a chat interface activated by pressing '/'. The feature extends the existing LLM configuration system and integrates with the current pane architecture, maintaining consistency with the application's terminal-first design principles.

## Technical Context
**Language/Version**: Rust 1.70+
**Primary Dependencies**: Ratatui 0.29, Tokio 1.42, scc 2.1, openai-api-rs 6.0.11
**Storage**: In-memory caching via shared state, configuration via JSON (~/.config/grw/config.json)
**Testing**: cargo test with unit tests, integration tests, and contract tests
**Target Platform**: Linux terminal application
**Project Type**: Single Rust application with modular architecture
**Performance Goals**: <100ms panel toggle, <10s LLM responses, <16ms UI updates (60fps)
**Constraints**: <100MB memory usage, lock-free shared state, async LLM operations, must not block UI
**Scale/Scope**: Single feature addition to existing codebase, extends 3 existing modules (pane, config, llm)

## Constitution Check
*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### I. Terminal-First Architecture ✅
- **Compliance**: Advice panel is implemented as a terminal-based pane using Ratatui
- **Navigation**: Uses standard vim-like keybindings (j, k, etc.) consistent with existing panes
- **Accessibility**: All functionality accessible via keyboard (Ctrl+L, /, ?, Esc)
- **Output**: Structured text display with machine-readable formats for LLM responses

### II. Real-Time Monitoring ✅
- **Performance**: Panel operations must complete within 16ms for 60fps UI
- **Updates**: Uses shared state architecture for non-blocking state updates
- **Responsiveness**: Async LLM operations prevent UI blocking
- **Overhead**: Minimal performance impact through efficient caching and state management

### III. Test-Driven Development ✅
- **TDD Required**: All features must be implemented with tests first
- **Test Levels**: Unit tests for components, integration tests for LLM flows, contract tests for APIs
- **Red-Green-Refactor**: Strict enforcement for all code changes
- **Validation**: Tests must validate both functionality and performance characteristics

### IV. Performance & Observability ✅
- **Data Structures**: Uses lock-free concurrent data structures (scc crate) via shared state
- **Logging**: Comprehensive logging at INFO/DEBUG levels for all LLM operations
- **Metrics**: Performance tracking for LLM response times and panel operations
- **Optimization**: Caching strategies for repeated diff analysis and chat sessions

### V. User Experience Excellence ✅
- **Intuitive Design**: Full-screen panel with clear modes (Viewing, Chatting, Help)
- **Keybindings**: Consistent with existing vim-like navigation patterns
- **Features**: Contextual help, chat interface, error handling, loading states
- **Performance**: Responsive UI with immediate feedback for all user actions

## Project Structure

### Documentation (this feature)
```
specs/001-i-would-like/
├── plan.md              # This file (/plan command output)
├── research.md          # Phase 0 output (/plan command)
├── data-model.md        # Phase 1 output (/plan command)
├── quickstart.md        # Phase 1 output (/plan command)
├── contracts/           # Phase 1 output (/plan command)
│   └── advice-panel-api.md
└── tasks.md             # Phase 2 output (/tasks command - NOT created by /plan)
```

### Source Code (repository root)
```
src/
├── pane.rs              # Modified: Add AdvicePane implementation
├── config.rs            # Modified: Extend Config with advice settings
├── llm.rs               # Modified: Add advice generation methods
├── shared_state.rs      # Modified: Extend LlmSharedState for advice
├── ui.rs                # Modified: Add advice panel integration
├── main.rs              # Modified: Add Ctrl+L key handling
└── lib.rs               # Modified: Export new advice types

tests/
├── advice_panel_tests.rs        # New: Unit tests for advice panel
├── llm_advice_tests.rs          # New: LLM integration tests
└── integration_advice_tests.rs  # New: Full integration tests
```

**Structure Decision**: Single Rust application extending existing modules. The advice panel feature integrates with the current architecture by extending the pane system, configuration management, and LLM integration rather than creating separate components. This maintains consistency and reduces code duplication.

## Phase 0: Outline & Research
1. **Extract unknowns from Technical Context** above:
   - No NEEDS CLARIFICATION markers found - all technical aspects are defined
   - All dependencies identified and best practices researched
   - Integration patterns established through existing codebase analysis

2. **Generate and dispatch research agents**:
   ```
   Research pane architecture patterns in existing codebase
   Research LLM integration extension strategies
   Research configuration management patterns
   Research shared state usage for new features
   Research terminal UI best practices for chat interfaces
   ```

3. **Consolidate findings** in `research.md` using format:
   - Decision: Extend existing pane system with AdvicePane
   - Rationale: Maintains consistency and leverages existing patterns
   - Alternatives considered: Separate advice system, custom widget framework

**Output**: research.md with all technical decisions and architecture patterns documented

## Phase 1: Design & Contracts
*Prerequisites: research.md complete*

1. **Extract entities from feature spec** → `data-model.md`:
   - Entity name, fields, relationships
   - Validation rules from requirements
   - State transitions if applicable

2. **Generate API contracts** from functional requirements:
   - For each user action → endpoint
   - Use standard REST/GraphQL patterns
   - Output OpenAPI/GraphQL schema to `/contracts/`

3. **Generate contract tests** from contracts:
   - One test file per endpoint
   - Assert request/response schemas
   - Tests must fail (no implementation yet)

4. **Extract test scenarios** from user stories:
   - Each story → integration test scenario
   - Quickstart test = story validation steps

5. **Update agent file incrementally** (O(1) operation):
   - Run `.specify/scripts/bash/update-agent-context.sh claude`
     **IMPORTANT**: Execute it exactly as specified above. Do not add or remove any arguments.
   - If exists: Add only NEW tech from current plan
   - Preserve manual additions between markers
   - Update recent changes (keep last 3)
   - Keep under 150 lines for token efficiency
   - Output to repository root

**Output**: data-model.md, /contracts/*, failing tests, quickstart.md, agent-specific file

## Phase 2: Task Planning Approach
*This section describes what the /tasks command will do - DO NOT execute during /plan*

**Task Generation Strategy**:
- Load `.specify/templates/tasks-template.md` as base
- Generate tasks from Phase 1 design docs (contracts, data model, quickstart)
- Each contract → contract test task [P]
- Each entity → model creation task [P]
- Each user story → integration test task
- Implementation tasks to make tests pass

**Ordering Strategy**:
- TDD order: Tests before implementation
- Dependency order: Models before services before UI
- Mark [P] for parallel execution (independent files)

**Task Categories**:
- **Setup**: Configuration extension, new data structures
- **Tests**: Contract tests, integration tests, unit tests
- **Core**: AdvicePane implementation, LLM integration methods
- **UI**: Rendering, key handling, mode management
- **Integration**: Shared state updates, event handling
- **Polish**: Error handling, performance optimization, documentation

**Estimated Output**: 25-30 numbered, ordered tasks in tasks.md

**IMPORTANT**: This phase is executed by the /tasks command, NOT by /plan

## Phase 3+: Future Implementation
*These phases are beyond the scope of the /plan command*

**Phase 3**: Task execution (/tasks command creates tasks.md)
**Phase 4**: Implementation (execute tasks.md following constitutional principles)
**Phase 5**: Validation (run tests, execute quickstart.md, performance validation)

## Complexity Tracking
*Fill ONLY if Constitution Check has violations that must be justified*

No constitutional violations identified. The design follows all established principles:
- Terminal-first architecture with keyboard navigation
- Real-time performance requirements met through async operations
- Test-driven development approach maintained
- Performance and observability through shared state
- User experience excellence with consistent patterns

## Progress Tracking
*This checklist is updated during execution flow*

**Phase Status**:
- [x] Phase 0: Research complete (/plan command)
- [x] Phase 1: Design complete (/plan command)
- [x] Phase 2: Task planning complete (/plan command - describe approach only)
- [ ] Phase 3: Tasks generated (/tasks command)
- [ ] Phase 4: Implementation complete
- [ ] Phase 5: Validation passed

**Gate Status**:
- [x] Initial Constitution Check: PASS
- [x] Post-Design Constitution Check: PASS
- [x] All NEEDS CLARIFICATION resolved
- [x] Complexity deviations documented (none)

---
*Based on Constitution v1.0.0 - See `/memory/constitution.md`*