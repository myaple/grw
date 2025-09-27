<!-- Sync Impact Report:
- Version change: none → 1.0.0 (initial version)
- Modified principles: All 5 principles defined from scratch
- Added sections: Technical Constraints, Development Standards, Governance
- Removed sections: None (template was empty)
- Templates updated: ✅ .specify/templates/plan-template.md (version reference updated)
- Templates pending: ⚠ None - all templates are consistent
- Follow-up TODOs: None
-->
# GRW Constitution

## Core Principles

### I. Terminal-First Architecture
GRW is fundamentally a terminal-based application that prioritizes the command-line interface experience. All functionality MUST be accessible via TUI with keyboard-driven navigation. The application MUST be self-contained and runnable in any terminal environment without requiring GUI dependencies. Terminal output MUST be the primary interface, with structured text formats for machine readability.

### II. Real-Time Monitoring
GRW MUST provide continuous, real-time monitoring of git repositories with minimal performance overhead. The application MUST update repository status automatically every 500ms using efficient, non-blocking operations. All monitoring MUST be responsive and never degrade the user experience or git repository performance. Real-time updates MUST be delivered through the shared state architecture for optimal performance.

### III. Test-Driven Development
All features MUST be developed following TDD principles: tests MUST be written before implementation, tests MUST fail initially, then implementation MUST make tests pass. This applies to all levels: unit tests, integration tests, and contract tests. The Red-Green-Refactor cycle MUST be strictly enforced for all code changes. Tests MUST validate both functionality and performance characteristics.

### IV. Performance & Observability
GRW MUST prioritize performance through efficient data structures and observability through comprehensive logging. The application MUST use lock-free concurrent data structures (scc crate) for shared state management. All operations MUST be measurable with performance metrics. Comprehensive logging MUST be available at multiple levels (INFO, DEBUG) with structured output. Performance bottlenecks MUST be identified and eliminated proactively.

### V. User Experience Excellence
GRW MUST provide an exceptional user experience through intuitive design, responsive performance, and thoughtful features. The interface MUST be accessible with vim-like keybindings that feel natural to terminal users. Features like themes, panel toggling, and intelligent status bars MUST enhance usability without compromising performance. Error handling MUST be graceful and informative, helping users understand and resolve issues quickly.

## Technical Constraints

### Technology Stack
- **Language**: Rust 1.70+ (performance, safety, concurrency)
- **UI Framework**: Ratatui 0.29 (terminal interface)
- **Concurrency**: Tokio 1.42 (async runtime)
- **Git Operations**: git2 0.19 (native git bindings)
- **Shared State**: scc 2.1 (lock-free data structures)
- **Configuration**: JSON-based config in ~/.config/grw/

### Performance Requirements
- **Update Frequency**: 500ms repository status refresh
- **Memory Usage**: Must remain under 100MB for typical repositories
- **Response Time**: All UI operations must respond within 16ms (60fps)
- **Startup Time**: Application must start within 1 second
- **Cache Limits**: Commits limited to 100-300 entries for memory efficiency

### Architecture Requirements
- **Shared State**: Must use lock-free concurrent data structures
- **Worker Pattern**: Background workers for git, LLM, and monitor operations
- **Error Handling**: Must use Result types and proper error propagation
- **Configuration**: Must support both CLI args and persistent config files
- **Logging**: Must follow XDG Base Directory specification (~/.local/state/grw/)

## Development Standards

### Code Quality
- **Formatting**: MUST use rustfmt for consistent code style
- **Linting**: MUST pass cargo clippy with all enabled rules
- **Testing**: MUST maintain comprehensive test coverage
- **Documentation**: MUST provide clear documentation for all public APIs
- **Dependencies**: MUST use minimal, well-maintained dependencies

### Git Workflow
- **Branch Naming**: Must follow feature-branch pattern with descriptive names
- **Commit Messages**: Must be clear, concise, and follow conventional commits
- **Pull Requests**: All changes MUST go through PR review process
- **Testing**: All PRs MUST pass all tests before merging
- **Performance**: Performance regressions MUST be addressed before merging

### Release Management
- **Versioning**: MUST follow Semantic Versioning (MAJOR.MINOR.PATCH)
- **Changelog**: MUST maintain comprehensive changelog for all releases
- **Breaking Changes**: MUST be clearly documented and communicated
- **Backward Compatibility**: MUST maintain compatibility within major versions
- **Dependencies**: MUST keep dependencies updated and secure

## Governance

### Amendment Process
- **Proposal**: Changes MUST be proposed with clear rationale
- **Review**: All amendments MUST undergo thorough review
- **Approval**: Constitution changes require maintainer approval
- **Documentation**: All changes MUST be properly documented
- **Communication**: Significant changes MUST be communicated to users

### Compliance
- **Constitution Supremacy**: This constitution supersedes all other practices
- **Validation**: All code changes MUST be validated against constitutional principles
- **Complexity**: Added complexity MUST be justified and documented
- **Review**: Regular reviews MUST ensure ongoing compliance
- **Evolution**: The constitution MUST evolve as the project matures

### Development Guidance
- **Runtime Guidance**: Use CLAUDE.md for specific development practices
- **Templates**: Use .specify/templates for consistent development artifacts
- **Standards**: Follow Rust best practices and community conventions
- **Performance**: Always prioritize performance and user experience
- **Maintainability**: Code MUST be maintainable and well-documented

**Version**: 1.0.0 | **Ratified**: 2025-09-27 | **Last Amended**: 2025-09-27