# Design Document

## Overview

This design outlines the migration from channel-based communication to shared concurrent data structures using the scc crate. The migration will replace mpsc channels with lock-free data structures, providing better performance through reduced overhead and improved cache locality while maintaining thread safety.

The scc crate provides several key data structures we'll leverage:
- `HashMap`: Lock-free concurrent hash map for caching and state storage
- `Queue`: Lock-free queue for task coordination when needed
- `TreeIndex`: For hierarchical data like file trees
- `Bag`: For collecting results from multiple workers

## Architecture

### Current Architecture Issues
- Channel overhead for frequent git status updates
- Message serialization/deserialization costs
- Complex error handling across channel boundaries
- Difficulty in implementing efficient caching due to channel constraints
- Memory pressure from queued messages

### New Shared State Architecture

```
┌─────────────────┐    ┌─────────────────────────────────┐
│   Main Thread   │    │        Shared State             │
│                 │◄──►│  ┌─────────────────────────────┐ │
│  - UI Rendering │    │  │     GitSharedState          │ │
│  - Event Loop   │    │  │  - repo_data: HashMap       │ │
│  - User Input   │    │  │  - commit_cache: HashMap    │ │
│                 │    │  │  - file_diffs: HashMap      │ │
└─────────────────┘    │  └─────────────────────────────┘ │
                       │  ┌─────────────────────────────┐ │
┌─────────────────┐    │  │     LlmSharedState          │ │
│   Git Worker    │◄──►│  │  - summary_cache: HashMap   │ │
│                 │    │  │  - advice_cache: HashMap    │ │
│  - Status Check │    │  │  - active_tasks: Bag        │ │
│  - Diff Compute │    │  └─────────────────────────────┘ │
│  - History Load │    │  ┌─────────────────────────────┐ │
└─────────────────┘    │  │   MonitorSharedState        │ │
                       │  │  - output: HashMap          │ │
┌─────────────────┐    │  │  - timing: HashMap          │ │
│   LLM Worker    │◄──►│  └─────────────────────────────┘ │
│                 │    └─────────────────────────────────┘
│  - Summary Gen  │
│  - Advice Gen   │
│  - Preloading   │
└─────────────────┘
```

## Components and Interfaces

### 1. Shared State Manager

```rust
pub struct SharedStateManager {
    git_state: Arc<GitSharedState>,
    llm_state: Arc<LlmSharedState>,
    monitor_state: Arc<MonitorSharedState>,
}

impl SharedStateManager {
    pub fn new() -> Self { ... }
    pub fn git_state(&self) -> &Arc<GitSharedState> { ... }
    pub fn llm_state(&self) -> &Arc<LlmSharedState> { ... }
    pub fn monitor_state(&self) -> &Arc<MonitorSharedState> { ... }
}
```

### 2. Git Shared State

```rust
pub struct GitSharedState {
    // Current repository state
    repo_data: scc::HashMap<String, GitRepo>,
    
    // Commit information cache
    commit_cache: scc::HashMap<String, CommitInfo>,
    
    // File diff cache for performance
    file_diff_cache: scc::HashMap<String, Vec<FileDiff>>,
    
    // Current view mode and metadata
    view_mode: AtomicU8, // Encoded ViewMode
    last_update: AtomicU64, // Timestamp
    
    // Error state
    error_state: scc::HashMap<String, String>,
}

impl GitSharedState {
    pub fn update_repo(&self, repo: GitRepo) -> Result<(), scc::hash_map::Error<String, GitRepo>>
    pub fn get_repo(&self) -> Option<GitRepo>
    pub fn cache_commit(&self, sha: String, commit: CommitInfo)
    pub fn get_cached_commit(&self, sha: &str) -> Option<CommitInfo>
    pub fn set_error(&self, error: String)
    pub fn clear_error(&self)
    pub fn get_error(&self) -> Option<String>
}
```

### 3. LLM Shared State

```rust
pub struct LlmSharedState {
    // Summary cache with commit SHA as key
    summary_cache: scc::HashMap<String, String>,
    
    // Advice cache with diff hash as key
    advice_cache: scc::HashMap<String, String>,
    
    // Active summary generation tasks
    active_summary_tasks: scc::Bag<String>,
    
    // Active advice generation tasks  
    active_advice_tasks: scc::Bag<String>,
    
    // Current advice content
    current_advice: scc::HashMap<String, String>,
    
    // Error states
    error_state: scc::HashMap<String, String>,
}

impl LlmSharedState {
    pub fn cache_summary(&self, commit_sha: String, summary: String)
    pub fn get_cached_summary(&self, commit_sha: &str) -> Option<String>
    pub fn is_summary_loading(&self, commit_sha: &str) -> bool
    pub fn start_summary_task(&self, commit_sha: String)
    pub fn complete_summary_task(&self, commit_sha: &str)
    pub fn update_advice(&self, advice: String)
    pub fn get_current_advice(&self) -> Option<String>
}
```

### 4. Monitor Shared State

```rust
pub struct MonitorSharedState {
    // Monitor command output
    output: scc::HashMap<String, String>,
    
    // Timing information
    timing_info: scc::HashMap<String, MonitorTiming>,
    
    // Configuration
    config: scc::HashMap<String, String>,
}

#[derive(Clone)]
pub struct MonitorTiming {
    pub last_run: u64,
    pub elapsed: u64,
    pub has_run: bool,
}
```

### 5. Worker Adaptations

#### Git Worker Redesign
```rust
pub struct GitWorker {
    shared_state: Arc<GitSharedState>,
    repo: Repository,
    path: PathBuf,
    // Remove: rx, tx channels
    // Remove: local caches (now in shared state)
}

impl GitWorker {
    pub async fn run_continuous(&mut self) {
        loop {
            // Update git state
            let repo_snapshot = self.create_git_repo_snapshot();
            self.shared_state.update_repo(repo_snapshot);
            
            // Sleep for update interval
            tokio::time::sleep(self.update_interval).await;
        }
    }
    
    pub fn get_commit_history(&mut self, limit: usize) -> Result<()> {
        let commits = self.fetch_commits(limit)?;
        for commit in commits {
            self.shared_state.cache_commit(commit.sha.clone(), commit);
        }
        Ok(())
    }
}
```

#### LLM Worker Redesign
```rust
pub struct LlmWorker {
    shared_state: Arc<LlmSharedState>,
    git_state: Arc<GitSharedState>,
    client: LlmClient,
    // Remove: channels
}

impl LlmWorker {
    pub async fn run_summary_generation(&self) {
        loop {
            // Check for pending summary tasks
            if let Some(commit_sha) = self.get_next_summary_task() {
                if let Ok(summary) = self.generate_summary(&commit_sha).await {
                    self.shared_state.cache_summary(commit_sha.clone(), summary);
                    self.shared_state.complete_summary_task(&commit_sha);
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

## Data Models

### Atomic Operations
- Use `AtomicU64` for timestamps and counters
- Use `AtomicU8` for enum states (ViewMode, etc.)
- Use `AtomicBool` for flags and status indicators

### Cache Management
- Implement LRU eviction using scc::HashMap with timestamp tracking
- Use configurable cache sizes for different data types
- Implement cache warming strategies for frequently accessed data

### Error Handling
- Store errors in shared HashMap structures with categorization
- Use Result types for all shared state operations
- Implement error recovery mechanisms in workers

## Error Handling

### Shared State Errors
```rust
#[derive(Debug, Clone)]
pub enum SharedStateError {
    ConcurrentModification,
    CacheEviction,
    InvalidState,
    WorkerFailure(String),
}

impl SharedStateError {
    pub fn to_user_message(&self) -> String { ... }
}
```

### Error Recovery
- Workers continue operating on shared state errors
- Main thread displays user-friendly error messages
- Automatic retry mechanisms for transient failures
- Graceful degradation when shared state is unavailable

## Testing Strategy

### Unit Tests
- Test individual shared state operations
- Verify atomic operations work correctly
- Test cache eviction policies
- Validate error handling paths

### Integration Tests
- Test worker coordination through shared state
- Verify data consistency across concurrent access
- Test performance improvements over channel-based approach
- Validate memory usage patterns

### Concurrency Tests
- Stress test with multiple workers accessing shared state
- Verify no data races or corruption occur
- Test cache consistency under high load
- Validate proper cleanup on worker termination

### Performance Tests
- Benchmark shared state access vs. channel communication
- Measure memory usage improvements
- Test latency improvements for UI updates
- Validate cache hit rates and effectiveness

### Migration Tests
- Test gradual migration path (hybrid channel/shared state)
- Verify feature parity during migration
- Test rollback capabilities if needed
- Validate data migration correctness

## Migration Strategy

### Phase 1: Infrastructure Setup
- Add scc dependency to Cargo.toml
- Create shared state structures
- Implement basic shared state manager
- Add comprehensive tests for shared state operations

### Phase 2: Git Worker Migration
- Migrate GitWorker to use shared state
- Replace git-related channels with shared structures
- Update main thread to read from shared git state
- Maintain backward compatibility during transition

### Phase 3: LLM Worker Migration  
- Migrate LLM summary caching to shared state
- Replace LLM channels with shared structures
- Update summary preloading to use shared state
- Migrate advice generation to shared state

### Phase 4: Monitor Worker Migration
- Migrate monitor command output to shared state
- Replace monitor channels with shared structures
- Update timing information sharing

### Phase 5: Cleanup and Optimization
- Remove all channel-based communication
- Optimize shared state access patterns
- Implement advanced caching strategies
- Performance tuning and final testing