# Requirements Document

## Introduction

This feature involves migrating the current channel-based communication architecture to use lock-free shared data structures from the scc crate. The goal is to improve performance and reduce complexity by eliminating the need for channels between background workers and the main thread, replacing them with shared concurrent data structures that provide better cache locality and reduced overhead.

## Requirements

### Requirement 1

**User Story:** As a developer using the application, I want background workers to share data efficiently with the main thread, so that the application has better performance and lower latency.

#### Acceptance Criteria

1. WHEN background workers update git information THEN the main thread SHALL access this data through scc shared structures without channel communication
2. WHEN LLM summaries are generated THEN they SHALL be stored in scc concurrent data structures accessible by the main thread
3. WHEN the application starts THEN all shared data structures SHALL be initialized and accessible to both main thread and background workers
4. WHEN multiple workers access shared data THEN there SHALL be no data races or corruption
5. WHEN the application shuts down THEN all shared data structures SHALL be properly cleaned up

### Requirement 2

**User Story:** As a developer maintaining the codebase, I want simplified data sharing logic, so that the code is easier to understand and maintain.

#### Acceptance Criteria

1. WHEN implementing data sharing THEN the system SHALL use scc data structures instead of mpsc channels
2. WHEN workers need to update shared state THEN they SHALL directly modify scc structures without sending messages
3. WHEN the main thread needs data THEN it SHALL directly access scc structures without receiving from channels
4. WHEN adding new shared data THEN developers SHALL use consistent scc patterns across the codebase
5. WHEN debugging data flow THEN the shared state SHALL be directly inspectable without channel message tracing

### Requirement 3

**User Story:** As a user of the application, I want git operations to remain responsive, so that I can efficiently browse repository changes.

#### Acceptance Criteria

1. WHEN git status updates occur THEN the shared GitRepo data SHALL be updated atomically in scc structures
2. WHEN commit history is loaded THEN commit data SHALL be stored in shared scc collections accessible by the main thread
3. WHEN file diffs are computed THEN they SHALL be stored in shared structures without channel overhead
4. WHEN the main thread renders UI THEN it SHALL access current git data directly from shared structures
5. WHEN git operations complete THEN the UI SHALL reflect changes without waiting for channel messages

### Requirement 4

**User Story:** As a user generating commit summaries, I want LLM operations to be efficient, so that summaries load quickly and don't block the interface.

#### Acceptance Criteria

1. WHEN LLM summaries are generated THEN they SHALL be cached in shared scc HashMap structures
2. WHEN summary preloading occurs THEN multiple summaries SHALL be stored concurrently in shared cache
3. WHEN the main thread needs a summary THEN it SHALL check shared cache directly without channel communication
4. WHEN summary generation completes THEN the result SHALL be immediately available in shared structures
5. WHEN cache eviction is needed THEN it SHALL occur safely using scc's concurrent operations

### Requirement 5

**User Story:** As a developer extending the application, I want consistent patterns for shared data, so that new features integrate seamlessly.

#### Acceptance Criteria

1. WHEN adding new background workers THEN they SHALL follow established scc sharing patterns
2. WHEN creating new shared state THEN it SHALL use appropriate scc data structures (HashMap, Queue, etc.)
3. WHEN implementing caching THEN it SHALL use shared scc structures with consistent eviction policies
4. WHEN workers need coordination THEN they SHALL use scc synchronization primitives instead of channels
5. WHEN shared data needs atomic updates THEN the system SHALL use scc's atomic operations

### Requirement 6

**User Story:** As a user of the application, I want monitor and advice features to remain functional, so that all existing functionality continues to work after the migration.

#### Acceptance Criteria

1. WHEN monitor commands execute THEN their output SHALL be shared through scc structures
2. WHEN LLM advice is generated THEN it SHALL be stored in shared structures accessible by the main thread
3. WHEN the application polls for updates THEN it SHALL check shared structures instead of receiving from channels
4. WHEN background tasks complete THEN their results SHALL be immediately visible in shared state
5. WHEN errors occur in background workers THEN they SHALL be communicated through shared error structures