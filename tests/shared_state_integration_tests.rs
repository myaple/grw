use grw::SharedStateManager;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_shared_state_manager_integration() {
    let manager = SharedStateManager::new();
    
    // Test initialization
    let result = manager.initialize();
    assert!(result.is_ok(), "SharedStateManager initialization should succeed");
    
    // Verify all components are accessible
    let git_state = manager.git_state();
    let _llm_state = manager.llm_state();
    let monitor_state = manager.monitor_state();
    
    // Test that we can access the underlying state
    assert_eq!(git_state.get_view_mode(), 0); // Should be set to default by initialize()
    
    // Test configuration was set during initialization
    let update_interval = monitor_state.get_config("update_interval");
    assert!(update_interval.is_some());
    assert_eq!(update_interval.unwrap(), "1000");
    
    // Test shutdown
    let result = manager.shutdown();
    assert!(result.is_ok(), "SharedStateManager shutdown should succeed");
}

#[test]
fn test_shared_state_manager_concurrent_access() {
    let manager = Arc::new(SharedStateManager::new());
    let _ = manager.initialize();
    
    let mut handles = vec![];
    
    // Test concurrent access to git state
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            let git_state = manager_clone.git_state();
            
            // Simulate git operations
            git_state.set_error(format!("error_{}", i), format!("Error message {}", i));
            git_state.set_view_mode(i as u8);
            
            // Read back the data
            let error = git_state.get_error(&format!("error_{}", i));
            assert!(error.is_some());
            assert_eq!(error.unwrap(), format!("Error message {}", i));
        });
        handles.push(handle);
    }
    
    // Test concurrent access to LLM state
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            let llm_state = manager_clone.llm_state();
            
            // Simulate LLM operations
            let commit_sha = format!("commit_{}", i);
            let summary = format!("Summary for commit {}", i);
            
            llm_state.start_summary_task(commit_sha.clone());
            llm_state.cache_summary(commit_sha.clone(), summary.clone());
            llm_state.complete_summary_task(&commit_sha);
            
            // Verify the data
            let cached_summary = llm_state.get_cached_summary(&commit_sha);
            assert!(cached_summary.is_some());
            assert_eq!(cached_summary.unwrap(), summary);
            assert!(!llm_state.is_summary_loading(&commit_sha));
        });
        handles.push(handle);
    }
    
    // Test concurrent access to monitor state
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            let monitor_state = manager_clone.monitor_state();
            
            // Simulate monitor operations
            let cmd_key = format!("cmd_{}", i);
            let output = format!("Output from command {}", i);
            
            monitor_state.update_output(cmd_key.clone(), output.clone());
            monitor_state.update_timing_with_elapsed(cmd_key.clone(), i * 100);
            
            // Verify the data
            let retrieved_output = monitor_state.get_output(&cmd_key);
            assert!(retrieved_output.is_some());
            assert_eq!(retrieved_output.unwrap(), output);
            
            let timing = monitor_state.get_timing(&cmd_key);
            assert!(timing.is_some());
            assert_eq!(timing.unwrap().elapsed, i * 100);
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete successfully");
    }
    
    // Verify final state
    let stats = manager.get_statistics();
    assert_eq!(stats.git_errors, 5);
    assert_eq!(stats.llm_summaries_cached, 5);
    assert_eq!(stats.monitor_outputs, 5);
    assert_eq!(stats.monitor_timings, 5);
    
    // Test cleanup
    let result = manager.cleanup();
    assert!(result.is_ok());
    
    // Verify cleanup worked
    let stats_after_cleanup = manager.get_statistics();
    assert_eq!(stats_after_cleanup.git_errors, 0);
    assert_eq!(stats_after_cleanup.llm_active_summary_tasks, 0);
    assert_eq!(stats_after_cleanup.llm_active_advice_tasks, 0);
}

#[test]
fn test_shared_state_manager_error_handling() {
    let manager = SharedStateManager::new();
    let _ = manager.initialize();
    
    // Add errors to all components
    manager.git_state().set_error("git_test".to_string(), "Git error".to_string());
    manager.llm_state().set_error("llm_test".to_string(), "LLM error".to_string());
    manager.monitor_state().set_error("monitor_test".to_string(), "Monitor error".to_string());
    
    // Test has_errors
    assert!(manager.has_errors());
    
    // Test get_all_errors
    let all_errors = manager.get_all_errors();
    assert_eq!(all_errors.len(), 3);
    
    // Verify error categorization
    let git_errors: Vec<_> = all_errors.iter()
        .filter(|(component, _, _)| component == "git")
        .collect();
    let llm_errors: Vec<_> = all_errors.iter()
        .filter(|(component, _, _)| component == "llm")
        .collect();
    let monitor_errors: Vec<_> = all_errors.iter()
        .filter(|(component, _, _)| component == "monitor")
        .collect();
    
    assert_eq!(git_errors.len(), 1);
    assert_eq!(llm_errors.len(), 1);
    assert_eq!(monitor_errors.len(), 1);
    
    assert_eq!(git_errors[0].1, "git_test");
    assert_eq!(git_errors[0].2, "Git error");
    
    // Test clear_all_errors
    manager.clear_all_errors();
    assert!(!manager.has_errors());
    assert!(manager.get_all_errors().is_empty());
}

#[test]
fn test_shared_state_manager_statistics() {
    let manager = SharedStateManager::new();
    let _ = manager.initialize();
    
    // Initially should have minimal data
    let initial_stats = manager.get_statistics();
    assert_eq!(initial_stats.total_errors(), 0);
    assert!(initial_stats.is_healthy());
    
    // Add some test data
    manager.llm_state().cache_summary("test1".to_string(), "Summary 1".to_string());
    manager.llm_state().cache_summary("test2".to_string(), "Summary 2".to_string());
    manager.llm_state().start_summary_task("task1".to_string());
    manager.monitor_state().update_output("cmd1".to_string(), "Output 1".to_string());
    manager.git_state().set_error("error1".to_string(), "Test error".to_string());
    
    let stats = manager.get_statistics();
    assert_eq!(stats.llm_summaries_cached, 2);
    assert_eq!(stats.llm_active_summary_tasks, 1);
    assert_eq!(stats.monitor_outputs, 1);
    assert_eq!(stats.git_errors, 1);
    assert_eq!(stats.total_cached_items(), 3); // 2 summaries + 1 output
    assert_eq!(stats.total_active_tasks(), 1);
    assert_eq!(stats.total_errors(), 1);
    assert!(!stats.is_healthy()); // Should be unhealthy due to error
}

#[test]
fn test_shared_state_manager_lifecycle() {
    let manager = SharedStateManager::new();
    
    // Test multiple initialization calls (should be safe)
    assert!(manager.initialize().is_ok());
    assert!(manager.initialize().is_ok());
    
    // Add some data
    manager.git_state().set_view_mode(5);
    manager.llm_state().cache_summary("test".to_string(), "Test summary".to_string());
    manager.monitor_state().update_output("test".to_string(), "Test output".to_string());
    
    // Test cleanup (should preserve data but clear errors and tasks)
    manager.git_state().set_error("test_error".to_string(), "Error".to_string());
    manager.llm_state().start_summary_task("test_task".to_string());
    
    assert!(manager.cleanup().is_ok());
    
    // Data should still be there
    assert_eq!(manager.git_state().get_view_mode(), 5);
    assert!(manager.llm_state().get_cached_summary("test").is_some());
    assert!(manager.monitor_state().get_output("test").is_some());
    
    // But errors and tasks should be cleared
    assert!(manager.git_state().get_error("test_error").is_none());
    assert!(!manager.llm_state().is_summary_loading("test_task"));
    
    // Test shutdown (should clear everything)
    assert!(manager.shutdown().is_ok());
    
    // All data should be cleared
    assert!(manager.llm_state().get_cached_summary("test").is_none());
    assert!(manager.monitor_state().get_output("test").is_none());
    
    // Multiple shutdown calls should be safe
    assert!(manager.shutdown().is_ok());
}

#[test]
fn test_shared_state_manager_stress_test() {
    let manager = Arc::new(SharedStateManager::new());
    let _ = manager.initialize();
    
    let mut handles = vec![];
    
    // Create multiple threads that perform various operations
    for thread_id in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("thread_{}_item_{}", thread_id, i);
                
                // Git operations
                manager_clone.git_state().set_view_mode((i % 256) as u8);
                if i % 10 == 0 {
                    manager_clone.git_state().set_error(key.clone(), format!("Error {}", i));
                }
                
                // LLM operations
                manager_clone.llm_state().cache_summary(key.clone(), format!("Summary {}", i));
                manager_clone.llm_state().start_summary_task(key.clone());
                
                // Small delay to increase chance of concurrent access
                thread::sleep(Duration::from_millis(1));
                
                manager_clone.llm_state().complete_summary_task(&key);
                
                // Monitor operations
                manager_clone.monitor_state().update_output(key.clone(), format!("Output {}", i));
                manager_clone.monitor_state().update_timing_with_elapsed(key, i);
            }
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete successfully");
    }
    
    // Verify the system is still functional
    let stats = manager.get_statistics();
    assert!(stats.llm_summaries_cached > 0);
    assert!(stats.monitor_outputs > 0);
    
    // Test that we can still perform operations
    manager.git_state().set_view_mode(42);
    assert_eq!(manager.git_state().get_view_mode(), 42);
    
    // Cleanup should work
    assert!(manager.cleanup().is_ok());
    assert!(manager.shutdown().is_ok());
}#
[test]
fn test_llm_main_thread_polling_integration() {
    let manager = SharedStateManager::new();
    let _ = manager.initialize();
    
    // Test advice polling from shared state
    let advice_key = "main_advice";
    let test_advice = "This is test advice from LLM";
    
    // Initially no advice should be available
    assert!(manager.llm_state().get_current_advice(advice_key).is_none());
    
    // Simulate LLM worker updating advice in shared state
    manager.llm_state().update_advice(advice_key.to_string(), test_advice.to_string());
    
    // Main thread should be able to read the advice
    let retrieved_advice = manager.llm_state().get_current_advice(advice_key);
    assert!(retrieved_advice.is_some());
    assert_eq!(retrieved_advice.unwrap(), test_advice);
    
    // Test error handling for advice generation
    let error_msg = "Failed to generate advice";
    manager.llm_state().set_error("advice_generation".to_string(), error_msg.to_string());
    
    let error = manager.llm_state().get_error("advice_generation");
    assert!(error.is_some());
    assert_eq!(error.unwrap(), error_msg);
    
    // Clear error
    assert!(manager.llm_state().clear_error("advice_generation"));
    assert!(manager.llm_state().get_error("advice_generation").is_none());
}

#[test]
fn test_llm_summary_cache_polling_integration() {
    let manager = SharedStateManager::new();
    let _ = manager.initialize();
    
    let commit_sha = "abc123def456";
    let summary = "This commit adds new functionality";
    
    // Initially no summary should be cached
    assert!(manager.llm_state().get_cached_summary(commit_sha).is_none());
    
    // Simulate LLM worker caching a summary
    manager.llm_state().cache_summary(commit_sha.to_string(), summary.to_string());
    
    // Main thread should be able to read the cached summary
    let cached_summary = manager.llm_state().get_cached_summary(commit_sha);
    assert!(cached_summary.is_some());
    assert_eq!(cached_summary.unwrap(), summary);
    
    // Test multiple summaries
    let commit_sha2 = "def456ghi789";
    let summary2 = "This commit fixes a bug";
    
    manager.llm_state().cache_summary(commit_sha2.to_string(), summary2.to_string());
    
    // Both summaries should be available
    assert_eq!(manager.llm_state().get_cached_summary(commit_sha).unwrap(), summary);
    assert_eq!(manager.llm_state().get_cached_summary(commit_sha2).unwrap(), summary2);
}

#[test]
fn test_llm_task_coordination_integration() {
    let manager = SharedStateManager::new();
    let _ = manager.initialize();
    
    let commit_sha = "task_test_commit";
    
    // Initially no task should be active
    assert!(!manager.llm_state().is_summary_loading(commit_sha));
    assert_eq!(manager.llm_state().active_summary_task_count(), 0);
    
    // Start a summary task
    manager.llm_state().start_summary_task(commit_sha.to_string());
    
    // Task should now be active
    assert!(manager.llm_state().is_summary_loading(commit_sha));
    assert_eq!(manager.llm_state().active_summary_task_count(), 1);
    
    // Complete the task
    manager.llm_state().complete_summary_task(commit_sha);
    
    // Task should no longer be active
    assert!(!manager.llm_state().is_summary_loading(commit_sha));
    assert_eq!(manager.llm_state().active_summary_task_count(), 0);
    
    // Test advice task coordination
    let advice_task_id = "advice_task_1";
    
    assert!(!manager.llm_state().is_advice_loading(advice_task_id));
    assert_eq!(manager.llm_state().active_advice_task_count(), 0);
    
    manager.llm_state().start_advice_task(advice_task_id.to_string());
    
    assert!(manager.llm_state().is_advice_loading(advice_task_id));
    assert_eq!(manager.llm_state().active_advice_task_count(), 1);
    
    manager.llm_state().complete_advice_task(advice_task_id);
    
    assert!(!manager.llm_state().is_advice_loading(advice_task_id));
    assert_eq!(manager.llm_state().active_advice_task_count(), 0);
}

#[test]
fn test_llm_concurrent_polling_and_updates() {
    let manager = Arc::new(SharedStateManager::new());
    let _ = manager.initialize();
    
    let mut handles = vec![];
    
    // Simulate main thread polling
    let manager_main = Arc::clone(&manager);
    let main_handle = thread::spawn(move || {
        for i in 0..50 {
            // Poll for advice
            let _advice = manager_main.llm_state().get_current_advice("main_advice");
            
            // Poll for summaries
            let commit_sha = format!("commit_{}", i % 5);
            let _summary = manager_main.llm_state().get_cached_summary(&commit_sha);
            
            // Check for errors
            let _error = manager_main.llm_state().get_error("advice_generation");
            
            thread::sleep(Duration::from_millis(1));
        }
    });
    handles.push(main_handle);
    
    // Simulate LLM workers updating shared state
    for worker_id in 0..3 {
        let manager_worker = Arc::clone(&manager);
        let worker_handle = thread::spawn(move || {
            for i in 0..20 {
                let commit_sha = format!("commit_{}", i % 5);
                let summary = format!("Summary from worker {} for iteration {}", worker_id, i);
                
                // Cache summary
                manager_worker.llm_state().cache_summary(commit_sha.clone(), summary);
                
                // Update advice
                let advice = format!("Advice from worker {} iteration {}", worker_id, i);
                manager_worker.llm_state().update_advice("main_advice".to_string(), advice);
                
                // Simulate task coordination
                manager_worker.llm_state().start_summary_task(commit_sha.clone());
                thread::sleep(Duration::from_millis(1));
                manager_worker.llm_state().complete_summary_task(&commit_sha);
                
                thread::sleep(Duration::from_millis(2));
            }
        });
        handles.push(worker_handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete successfully");
    }
    
    // Verify system is still functional
    let stats = manager.get_statistics();
    assert!(stats.llm_summaries_cached > 0);
    
    // Verify we can still read data
    let advice = manager.llm_state().get_current_advice("main_advice");
    assert!(advice.is_some());
    
    // Cleanup
    assert!(manager.cleanup().is_ok());
}