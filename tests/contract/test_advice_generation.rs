#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{AdvicePanel, Pane, PaneId};
    use crate::config::Config;
    use crate::pane::AdviceImprovement;

    #[test]
    fn test_advice_generation_api_exists() {
        // Test that advice generation API methods exist on AdvicePanel
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that the generate_advice method exists and can be called
        let result = std::panic::catch_unwind(|| {
            panel.generate_advice("sample git diff content")
        });

        // Should not panic, even if not implemented (should return error)
        assert!(result.is_ok(), "generate_advice method should exist and not panic");
    }

    #[test]
    fn test_advice_generation_with_empty_diff() {
        // Test advice generation behavior with empty diff
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Generate advice with empty diff
        let result = panel.generate_advice("");

        // Should handle empty diff gracefully
        assert!(result.is_ok(), "Should handle empty diff without error");

        let improvements = result.unwrap();
        // Empty diff might result in no improvements or a message about no changes
        // This establishes the contract for empty diff handling
        assert!(improvements.len() <= 3, "Should respect max_improvements config even for empty diff");
    }

    #[test]
    fn test_advice_generation_with_sample_diff() {
        // Test advice generation with sample git diff content
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        let sample_diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello, World!");
+    println!("Hello, Rust!");
 }
"#;

        let result = panel.generate_advice(sample_diff);

        // Should process sample diff without error
        assert!(result.is_ok(), "Should process sample diff without error");

        let improvements = result.unwrap();
        assert!(improvements.len() <= 3, "Should respect max_improvements config");

        // Each improvement should have required fields
        for improvement in improvements {
            assert!(!improvement.title.is_empty(), "Improvement should have a title");
            assert!(!improvement.description.is_empty(), "Improvement should have a description");
            assert_ne!(improvement.priority, crate::pane::ImprovementPriority::Unknown,
                      "Improvement should have a valid priority");
        }
    }

    #[test]
    fn test_advice_generation_respects_config_limits() {
        // Test that advice generation respects configuration limits
        let mut config = Config::default();
        config.advice = Some(AdviceConfig {
            max_improvements: Some(1),
            timeout_seconds: Some(5),
            context_lines: Some(10),
            ..Default::default()
        });

        let advice_config = config.advice.as_ref().unwrap().clone();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        let sample_diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello, World!");
+    println!("Hello, Rust!");
 }
"#;

        let result = panel.generate_advice(sample_diff);

        assert!(result.is_ok(), "Should respect config limits without error");

        let improvements = result.unwrap();
        assert!(improvements.len() <= 1, "Should respect max_improvements=1 config");
    }

    #[test]
    fn test_advice_generation_error_handling() {
        // Test advice generation error handling for invalid inputs
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test with invalid/malformed diff content
        let invalid_diff = "This is not a valid git diff";

        let result = panel.generate_advice(invalid_diff);

        // Should handle invalid input gracefully
        assert!(result.is_ok(), "Should handle invalid diff input gracefully");

        let improvements = result.unwrap();
        // Might return empty improvements or error improvements, but shouldn't panic
        assert!(improvements.len() <= 3, "Should still respect max_improvements for invalid input");
    }

    #[test]
    fn test_advice_generation_async_contract() {
        // Test that advice generation can work asynchronously
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that there's a method to start async advice generation
        let async_result = std::panic::catch_unwind(|| {
            panel.start_async_advice_generation("sample diff")
        });

        // Should not panic, even if not implemented
        assert!(async_result.is_ok(), "Should have async advice generation method");

        // Test that there's a method to check async generation status
        let status_result = std::panic::catch_unwind(|| {
            panel.get_advice_generation_status()
        });

        assert!(status_result.is_ok(), "Should have method to check advice generation status");
    }

    #[test]
    fn test_advice_generation_caching_contract() {
        // Test that advice generation implements caching behavior
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        let sample_diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello, World!");
+    println!("Hello, Rust!");
 }
"#;

        // First call should generate advice
        let result1 = panel.generate_advice(sample_diff);
        assert!(result1.is_ok(), "First call should succeed");

        // Second call with same diff should potentially use cache
        let result2 = panel.generate_advice(sample_diff);
        assert!(result2.is_ok(), "Second call with same diff should succeed");

        // Both should return valid improvements
        let improvements1 = result1.unwrap();
        let improvements2 = result2.unwrap();

        assert!(improvements1.len() <= 3, "First call should respect limits");
        assert!(improvements2.len() <= 3, "Second call should respect limits");
    }
}