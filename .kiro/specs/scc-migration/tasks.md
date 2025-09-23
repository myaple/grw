# Implementation Plan

- [x] 1. Set up scc dependency and basic shared state infrastructure
  - Add scc crate to Cargo.toml with appropriate features
  - Create new module `src/shared_state.rs` with basic structure
  - Implement SharedStateManager with initialization logic
  - _Requirements: 1.3, 5.1_

- [x] 2. Implement GitSharedState structure and operations
  - [x] 2.1 Create GitSharedState struct with scc data structures
    - Define GitSharedState with scc::HashMap for repo data and commit cache
    - Implement atomic fields for view mode and timestamps
    - Add error state management with scc::HashMap
    - _Requirements: 3.1, 3.2, 1.4_

  - [x] 2.2 Implement GitSharedState methods for data access
    - Write update_repo, get_repo methods with proper error handling
    - Implement commit caching methods (cache_commit, get_cached_commit)
    - Add error state management methods (set_error, clear_error, get_error)
    - Create unit tests for all GitSharedState operations
    - _Requirements: 3.1, 3.3, 2.3_

- [x] 3. Implement LlmSharedState structure and operations
  - [x] 3.1 Create LlmSharedState struct with concurrent collections
    - Define LlmSharedState with scc::HashMap for summary and advice caches
    - Add scc::Bag for tracking active tasks
    - Implement error state management structures
    - _Requirements: 4.1, 4.2, 1.4_

  - [x] 3.2 Implement LlmSharedState methods for cache management
    - Write summary caching methods (cache_summary, get_cached_summary)
    - Implement task tracking methods (start_summary_task, complete_summary_task, is_summary_loading)
    - Add advice management methods (update_advice, get_current_advice)
    - Create comprehensive unit tests for LlmSharedState
    - _Requirements: 4.1, 4.3, 4.4_

- [x] 4. Implement MonitorSharedState structure
  - [x] 4.1 Create MonitorSharedState with output and timing data
    - Define MonitorSharedState with scc::HashMap for output and timing
    - Implement MonitorTiming struct for timing information
    - Add configuration management with shared state
    - _Requirements: 6.1, 6.4_

  - [x] 4.2 Implement MonitorSharedState access methods
    - Write output management methods (update_output, get_output)
    - Implement timing methods (update_timing, get_timing)
    - Add configuration methods for monitor settings
    - Create unit tests for MonitorSharedState operations
    - _Requirements: 6.1, 6.2_

- [x] 5. Create SharedStateManager integration
  - [x] 5.1 Implement SharedStateManager with all state components
    - Create SharedStateManager struct holding Arc references to all shared states
    - Implement initialization methods for all shared state components
    - Add cleanup and shutdown methods for proper resource management
    - _Requirements: 1.3, 1.5, 5.2_

  - [x] 5.2 Add SharedStateManager to main application
    - Integrate SharedStateManager into main.rs initialization
    - Pass shared state references to existing workers during transition
    - Maintain existing channel functionality during hybrid phase
    - Create integration tests for SharedStateManager
    - _Requirements: 1.3, 2.4, 5.1_

- [ ] 6. Migrate GitWorker to use shared state
  - [ ] 6.1 Update GitWorker struct to use GitSharedState
    - Modify GitWorker to accept Arc<GitSharedState> in constructor
    - Remove channel-related fields (rx, tx) from GitWorker
    - Update internal caches to use shared state instead of local HashMap
    - _Requirements: 3.1, 2.1, 2.2_

  - [ ] 6.2 Implement shared state operations in GitWorker
    - Replace channel sends with direct shared state updates in update() method
    - Modify get_commit_history to store results in shared commit cache
    - Update cache_summary and get_cached_summary to use shared state
    - Remove channel message handling from run() method
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ] 6.3 Update GitWorker run loop for continuous operation
    - Implement continuous update loop without channel dependency
    - Add proper error handling that updates shared error state
    - Implement configurable update intervals using tokio::time::sleep
    - Create integration tests for GitWorker shared state operations
    - _Requirements: 3.4, 3.5, 2.3_

- [ ] 7. Update main thread to read from GitSharedState
  - [ ] 7.1 Replace git channel polling with shared state access
    - Remove git_repo.try_get_result() calls from main loop
    - Replace with direct access to shared_state.git_state().get_repo()
    - Update error handling to check shared error state
    - _Requirements: 3.4, 2.3_

  - [ ] 7.2 Update commit picker to use shared commit cache
    - Modify commit picker activation to read from shared commit cache
    - Replace GitWorkerResult::CommitHistory handling with shared state access
    - Update commit loading logic to trigger shared state updates
    - Create tests for commit picker shared state integration
    - _Requirements: 3.2, 3.4_

- [ ] 8. Migrate LLM workers to use shared state
  - [ ] 8.1 Update AsyncLLMCommand to use LlmSharedState
    - Modify AsyncLLMCommand to accept Arc<LlmSharedState> in constructor
    - Remove channel-related fields (result_rx, result_tx) from AsyncLLMCommand
    - Update advice generation to store results in shared state
    - _Requirements: 4.4, 2.1, 2.2_

  - [ ] 8.2 Implement shared state operations in LLM workers
    - Replace channel sends with direct shared state updates in advice generation
    - Update summary generation to use shared summary cache
    - Modify task tracking to use shared active task collections
    - Remove channel message handling from LLM worker loops
    - _Requirements: 4.1, 4.2, 4.4_

  - [ ] 8.3 Update SummaryPreloader to use shared state
    - Modify SummaryPreloader to use LlmSharedState instead of channels
    - Update preload_single_summary to check and update shared cache
    - Replace GitWorkerCommand sends with direct shared state access
    - Implement task coordination using shared active task tracking
    - _Requirements: 4.2, 4.3, 2.2_

- [ ] 9. Update main thread LLM polling to use shared state
  - [ ] 9.1 Replace LLM channel polling with shared state access
    - Remove llm_command.try_get_result() calls from main loop
    - Replace with direct access to shared_state.llm_state().get_current_advice()
    - Update summary polling to read from shared summary cache
    - _Requirements: 4.4, 2.3_

  - [ ] 9.2 Update commit summary handling to use shared state
    - Modify handle_cached_summary_result to read from shared cache
    - Replace GitWorkerResult::CachedSummary handling with shared state access
    - Update summary preloading to use shared state coordination
    - Create integration tests for LLM shared state operations
    - _Requirements: 4.1, 4.3, 4.4_

- [ ] 10. Migrate monitor functionality to shared state
  - [ ] 10.1 Update AsyncMonitorCommand to use MonitorSharedState
    - Modify AsyncMonitorCommand to accept Arc<MonitorSharedState> in constructor
    - Remove channel-related fields from AsyncMonitorCommand
    - Update monitor output storage to use shared state
    - _Requirements: 6.1, 2.1, 2.2_

  - [ ] 10.2 Update main thread monitor polling
    - Remove monitor.try_get_result() calls from main loop
    - Replace with direct access to shared_state.monitor_state().get_output()
    - Update timing information access to use shared state
    - Create tests for monitor shared state integration
    - _Requirements: 6.2, 6.4, 2.3_

- [ ] 11. Remove all channel dependencies and cleanup
  - [ ] 11.1 Remove unused channel imports and structures
    - Remove tokio::sync::mpsc imports where no longer needed
    - Delete unused Result enums (GitWorkerResult, LLMResult, MonitorResult)
    - Remove channel-related fields from all worker structs
    - _Requirements: 2.1, 2.2_

  - [ ] 11.2 Update error handling to use shared state consistently
    - Ensure all error paths update appropriate shared error state
    - Remove channel-based error propagation logic
    - Implement consistent error recovery using shared state
    - Add comprehensive error handling tests
    - _Requirements: 1.4, 2.3, 6.5_

- [ ] 12. Implement cache management and optimization
  - [ ] 12.1 Add cache eviction policies to shared state
    - Implement LRU eviction for commit cache using timestamps
    - Add configurable cache size limits for all shared caches
    - Implement cache warming strategies for frequently accessed data
    - _Requirements: 5.3, 4.2_

  - [ ] 12.2 Optimize shared state access patterns
    - Profile shared state access performance vs. original channel approach
    - Optimize hot paths for UI rendering and frequent updates
    - Implement batch operations where beneficial
    - Add performance monitoring and metrics
    - _Requirements: 1.1, 3.5, 4.5_

- [ ] 13. Add comprehensive testing and validation
  - [ ] 13.1 Create concurrency stress tests
    - Write tests that simulate multiple workers accessing shared state concurrently
    - Verify no data races or corruption occur under high load
    - Test cache consistency with concurrent reads and writes
    - Validate proper cleanup when workers terminate unexpectedly
    - _Requirements: 1.4, 5.4_

  - [ ] 13.2 Add performance benchmarks and validation
    - Create benchmarks comparing shared state vs. channel performance
    - Measure memory usage improvements and validate against requirements
    - Test UI responsiveness improvements with shared state access
    - Validate cache hit rates and effectiveness
    - _Requirements: 1.1, 3.5, 4.5_

- [ ] 14. Final integration and cleanup
  - [ ] 14.1 Update configuration and documentation
    - Update any configuration related to worker communication
    - Add documentation for shared state architecture
    - Update error messages to reflect new architecture
    - _Requirements: 5.2, 5.4_

  - [ ] 14.2 Perform final testing and validation
    - Run full integration test suite to ensure feature parity
    - Validate all existing functionality works with shared state
    - Test application startup and shutdown with new architecture
    - Verify performance improvements meet requirements
    - _Requirements: 1.1, 6.1, 6.2, 6.3, 6.4, 6.5_