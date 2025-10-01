# Claude LLM Instructions

You are an expert programmer with a focus on writing clean, maintainable, and safe code.
Please follow these instructions when reviewing code.

## Critical Implementation Notes

### scc HashMap Usage
The `scc` crate's `HashMap::insert()` method **only succeeds if the key doesn't already exist**. Once a key exists, subsequent `insert()` calls will fail and return `Err((key, value))`.

**Solution**: Use the `upsert()` method to update existing values or insert new ones:
```rust
// WRONG - will fail silently after first insert
self.map.insert(key, value);

// CORRECT - use upsert to update or insert
self.map.upsert(key, value);
```

**Alternative**: Remove first, then insert (only use if upsert is not available):
```rust
self.map.remove(&key);
self.map.insert(key, value);
```

This pattern is used in `shared_state.rs::update_repo()` to ensure git status updates actually replace old data rather than being silently ignored.
