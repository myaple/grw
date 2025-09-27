# Tasks: Advice Panel

**Input**: Design documents from `/specs/001-i-would-like/`
**Prerequisites**: plan.md (required), research.md, data-model.md, contracts/, quickstart.md

## Execution Flow (main)
```
1. Load plan.md from feature directory
   → If not found: ERROR "No implementation plan found"
   → Extract: tech stack, libraries, structure
2. Load optional design documents:
   → data-model.md: Extract entities → model tasks
   → contracts/: Each file → contract test task
   → research.md: Extract decisions → setup tasks
   → quickstart.md: Extract scenarios → integration tests
3. Generate tasks by category:
   → Setup: project init, dependencies, linting
   → Tests: contract tests, integration tests
   → Core: models, services, CLI commands
   → Integration: DB, middleware, logging
   → Polish: unit tests, performance, docs
4. Apply task rules:
   → Different files = mark [P] for parallel
   → Same file = sequential (no [P])
   → Tests before implementation (TDD)
5. Number tasks sequentially (T001, T002...)
6. Generate dependency graph
7. Create parallel execution examples
8. Validate task completeness:
   → All contracts have tests?
   → All entities have models?
   → All endpoints implemented?
9. Return: SUCCESS (tasks ready for execution)
```

## Format: `[ID] [P?] Description`
- **[P]**: Can run in parallel (different files, no dependencies)
- Include exact file paths in descriptions

## Path Conventions
- **Single project**: `src/`, `tests/` at repository root
- **Web app**: `backend/src/`, `frontend/src/`
- **Mobile**: `api/src/`, `ios/src/` or `android/src/`
- Paths shown below assume single project - adjust based on plan.md structure

## Phase 3.1: Setup
- [X] T001 Extend configuration with advice settings in src/config.rs
- [X] T002 Add Advice variant to PaneId enum in src/pane.rs
- [X] T003 [P] Configure test framework for advice panel tests

## Phase 3.2: Tests First (TDD) ⚠️ MUST COMPLETE BEFORE 3.3
**CRITICAL: These tests MUST be written and MUST FAIL before ANY implementation**
- [ ] T004 [P] Contract test AdvicePanel creation in tests/contract/test_advice_panel_creation.rs
- [ ] T005 [P] Contract test panel visibility in tests/contract/test_panel_visibility.rs
- [ ] T006 [P] Contract test advice generation API in tests/contract/test_advice_generation.rs
- [ ] T007 [P] Contract test chat functionality in tests/contract/test_chat_functionality.rs
- [ ] T008 [P] Integration test panel opening flow in tests/integration/test_panel_opening.rs
- [ ] T009 [P] Integration test chat conversation flow in tests/integration/test_chat_conversation.rs
- [ ] T010 [P] Integration test help system in tests/integration/test_help_system.rs

## Phase 3.3: Core Implementation (ONLY after tests are failing)
- [ ] T011 [P] Implement AdvicePanel struct in src/pane.rs
- [ ] T012 [P] Implement AdviceContent enum and variants in src/pane.rs
- [ ] T013 [P] Implement AdviceMode enum in src/pane.rs
- [ ] T014 [P] Implement AdviceImprovement struct in src/pane.rs
- [ ] T015 [P] Implement ChatMessageData struct in src/pane.rs
- [ ] T016 [P] Implement HelpContent struct in src/pane.rs
- [ ] T017 Extend LlmClient with advice generation methods in src/llm.rs
- [ ] T018 Extend LlmSharedState with advice caching in src/shared_state.rs
- [ ] T019 Implement advice panel render method in src/pane.rs
- [ ] T020 Implement advice panel key event handling in src/pane.rs
- [ ] T021 Implement chat input handling in src/pane.rs
- [ ] T022 Implement help content display in src/pane.rs
- [ ] T023 Add advice panel to PaneRegistry in src/pane.rs
- [ ] T024 Add Ctrl+L key binding in src/main.rs
- [ ] T025 Add advice panel integration in src/ui.rs

## Phase 3.4: Integration
- [ ] T026 Connect advice panel to shared state updates
- [ ] T027 Implement diff analysis for advice generation
- [ ] T028 Implement chat session management
- [ ] T029 Add error handling for LLM failures
- [ ] T030 Implement loading states and progress indicators
- [ ] T031 Add scroll management for long content
- [ ] T032 Implement mode switching (Viewing/Chatting/Help)
- [ ] T033 Add panel state persistence
- [ ] T034 Implement performance optimization (caching)
- [ ] T035 Add logging for advice panel operations

## Phase 3.5: Polish
- [ ] T036 [P] Unit tests for advice panel components in tests/unit/test_advice_panel.rs
- [ ] T037 [P] Unit tests for LLM advice methods in tests/unit/test_llm_advice.rs
- [ ] T038 [P] Unit tests for configuration validation in tests/unit/test_advice_config.rs
- [ ] T039 Performance tests for panel response times
- [ ] T040 Performance tests for LLM request handling
- [ ] T041 Integration test for full advice workflow
- [ ] T042 [P] Update documentation with advice panel features
- [ ] T043 Add advice panel to README.md
- [ ] T044 Refactor and optimize code based on test results
- [ ] T045 Final validation against quickstart scenarios

## Dependencies
- Tests (T004-T010) before implementation (T011-T035)
- Configuration (T001) before pane implementation (T011-T025)
- PaneId enum (T002) before AdvicePanel implementation (T011)
- Core entities (T011-T016) before UI methods (T019-T022)
- LLM integration (T017) before advice generation (T027)
- Shared state (T018) before caching (T034)
- Implementation before polish (T036-T045)

## Parallel Example
```
# Launch T004-T010 together (contract tests):
Task: "Contract test AdvicePanel creation in tests/contract/test_advice_panel_creation.rs"
Task: "Contract test panel visibility in tests/contract/test_panel_visibility.rs"
Task: "Contract test advice generation API in tests/contract/test_advice_generation.rs"
Task: "Contract test chat functionality in tests/contract/test_chat_functionality.rs"
Task: "Integration test panel opening flow in tests/integration/test_panel_opening.rs"
Task: "Integration test chat conversation flow in tests/integration/test_chat_conversation.rs"
Task: "Integration test help system in tests/integration/test_help_system.rs"

# Launch T011-T016 together (core entities):
Task: "Implement AdvicePanel struct in src/pane.rs"
Task: "Implement AdviceContent enum and variants in src/pane.rs"
Task: "Implement AdviceMode enum in src/pane.rs"
Task: "Implement AdviceImprovement struct in src/pane.rs"
Task: "Implement ChatMessageData struct in src/pane.rs"
Task: "Implement HelpContent struct in src/pane.rs"

# Launch T036-T038 together (unit tests):
Task: "Unit tests for advice panel components in tests/unit/test_advice_panel.rs"
Task: "Unit tests for LLM advice methods in tests/unit/test_llm_advice.rs"
Task: "Unit tests for configuration validation in tests/unit/test_advice_config.rs"
```

## Notes
- [P] tasks = different files, no dependencies
- Verify tests fail before implementing
- Commit after each task
- Avoid: vague tasks, same file conflicts

## Task Generation Rules
*Applied during main() execution*

### From Contracts
- Each contract endpoint → contract test task [P]
- AdvicePanel creation → test_advice_panel_creation.rs
- Panel visibility → test_panel_visibility.rs
- Advice generation API → test_advice_generation.rs
- Chat functionality → test_chat_functionality.rs

### From Data Model
- Each entity → model creation task [P]
- AdvicePanel → AdvicePanel struct
- AdviceContent → AdviceContent enum
- AdviceMode → AdviceMode enum
- AdviceImprovement → AdviceImprovement struct
- ChatMessageData → ChatMessageData struct
- HelpContent → HelpContent struct

### From User Stories
- Panel opening → integration test [P]
- Chat conversation → integration test [P]
- Help system → integration test [P]

### From Quickstart Scenarios
- Configuration setup → config extension task
- Panel opening → implementation task
- Navigation → key handling task
- Chat interaction → chat implementation task
- Help display → help content task

## Validation Checklist
*GATE: Checked by main() before returning*

- [ ] All contracts have corresponding tests
- [ ] All entities have model tasks
- [ ] All tests come before implementation
- [ ] Parallel tasks truly independent
- [ ] Each task specifies exact file path
- [ ] No task modifies same file as another [P] task
- [ ] TDD order maintained (tests before implementation)
- [ ] Configuration tasks before implementation
- [ ] Integration tasks after core implementation
- [ ] Polish tasks after all functionality works