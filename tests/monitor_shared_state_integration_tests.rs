use std::sync::Arc;
use std::time::Duration;
use grw::shared_state::{MonitorSharedState, MonitorTiming};

#[test]
fn test_monitor_shared_state_integration() {
    let monitor_state = Arc::new(MonitorSharedState::new());
    
    // Test basic output storage and retrieval
    let command_key = "test_command";
    let test_output = "$ echo hello\nhello";
    
    monitor_state.update_output(command_key.to_string(), test_output.to_string());
    
    let retrieved_output = monitor_state.get_output(command_key);
    assert_eq!(retrieved_output, Some(test_output.to_string()));
}

#[test]
fn test_monitor_timing_integration() {
    let monitor_state = Arc::new(MonitorSharedState::new());
    
    let command_key = "timing_test_command";
    let elapsed_ms = 150;
    
    // Update timing information
    monitor_state.update_timing_with_elapsed(command_key.to_string(), elapsed_ms);
    
    // Retrieve timing information
    let timing = monitor_state.get_timing(command_key);
    assert!(timing.is_some());
    
    let timing = timing.unwrap();
    assert_eq!(timing.elapsed, elapsed_ms);
    assert!(timing.has_run);
    assert!(timing.last_run > 0); // Should have a valid timestamp
}

#[test]
fn test_monitor_error_handling_integration() {
    let monitor_state = Arc::new(MonitorSharedState::new());
    
    let command_key = "error_test_command";
    let error_output = "$ false\nCommand failed: exit status 1";
    let error_message = "Command execution failed";
    
    // Store error output and error state
    monitor_state.update_output(command_key.to_string(), error_output.to_string());
    monitor_state.set_error(command_key.to_string(), error_message.to_string());
    
    // Verify both output and error are stored
    let retrieved_output = monitor_state.get_output(command_key);
    let retrieved_error = monitor_state.get_error(command_key);
    
    assert_eq!(retrieved_output, Some(error_output.to_string()));
    assert_eq!(retrieved_error, Some(error_message.to_string()));
    
    // Clear error and verify it's gone
    let cleared = monitor_state.clear_error(command_key);
    assert!(cleared);
    
    let error_after_clear = monitor_state.get_error(command_key);
    assert_eq!(error_after_clear, None);
    
    // Output should still be there
    let output_after_clear = monitor_state.get_output(command_key);
    assert_eq!(output_after_clear, Some(error_output.to_string()));
}

#[test]
fn test_monitor_multiple_commands_integration() {
    let monitor_state = Arc::new(MonitorSharedState::new());
    
    // Test multiple commands can coexist
    let commands = vec![
        ("cmd1", "$ echo first\nfirst"),
        ("cmd2", "$ echo second\nsecond"),
        ("cmd3", "$ echo third\nthird"),
    ];
    
    // Store outputs for all commands
    for (key, output) in &commands {
        monitor_state.update_output(key.to_string(), output.to_string());
        monitor_state.update_timing_with_elapsed(key.to_string(), 100);
    }
    
    // Verify all outputs are retrievable
    for (key, expected_output) in &commands {
        let retrieved_output = monitor_state.get_output(key);
        assert_eq!(retrieved_output, Some(expected_output.to_string()));
        
        let timing = monitor_state.get_timing(key);
        assert!(timing.is_some());
        assert!(timing.unwrap().has_run);
    }
    
    // Verify counts
    assert_eq!(monitor_state.output_count(), 3);
    assert_eq!(monitor_state.timing_count(), 3);
}

#[test]
fn test_monitor_timing_staleness_integration() {
    let monitor_state = Arc::new(MonitorSharedState::new());
    
    let command_key = "staleness_test";
    
    // Create timing that should be considered stale
    let mut timing = MonitorTiming::new();
    timing.last_run = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() - 3600; // 1 hour ago
    timing.elapsed = 100;
    timing.has_run = true;
    
    monitor_state.update_timing(command_key.to_string(), timing);
    
    // Check staleness with 30 minute threshold (1800 seconds)
    assert!(monitor_state.has_stale_timing(1800));
    
    // Check staleness with 2 hour threshold (7200 seconds)
    assert!(!monitor_state.has_stale_timing(7200));
}

#[cfg(test)]
mod async_monitor_integration_tests {
    use super::*;
    use grw::monitor::{AsyncMonitorCommand, MonitorResult};
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_async_monitor_command_shared_state_integration() {
        let monitor_state = Arc::new(MonitorSharedState::new());
        
        // Create monitor command that runs a simple echo
        let mut monitor = AsyncMonitorCommand::new(
            "echo 'test output'".to_string(),
            1, // 1 second interval
            monitor_state.clone()
        );
        
        // Wait a bit for the command to run
        sleep(Duration::from_millis(1500)).await;
        
        // Check that output appears in shared state
        let command_key = monitor.get_command_key();
        let output = monitor_state.get_output(command_key);
        assert!(output.is_some());
        
        let output_str = output.unwrap();
        assert!(output_str.contains("test output"));
        assert!(output_str.contains("$ echo 'test output'"));
        
        // Check timing information
        let timing = monitor_state.get_timing(command_key);
        assert!(timing.is_some());
        
        let timing = timing.unwrap();
        assert!(timing.has_run);
        assert!(timing.last_run > 0);
        
        // Verify monitor methods still work
        assert!(monitor.has_run_yet());
        assert!(monitor.get_elapsed_since_last_run().is_some());
        
        // Test try_get_result still works
        let result = monitor.try_get_result();
        assert!(result.is_some());
        
        match result.unwrap() {
            MonitorResult::Success(output) => {
                assert!(output.contains("test output"));
            }
            MonitorResult::Error(_) => panic!("Expected success result"),
        }
    }
}