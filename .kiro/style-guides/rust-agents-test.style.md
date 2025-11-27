---
style-guide:
  description: "Rust Style Guide - Test Phase (Condensed)"
  prefer-in-contexts:
    - "Rust test development"
    - "TDD red phase"
---

# Rust Style Guide - Test Phase

Rules for writing tests. Use during the "red" phase of TDD.

## Test Structure

### Given/When/Then Comments

Every test needs cucumber-style comments:

```rust
#[test]
fn rejects_empty_username() {
    // Given an empty string
    let input = "";
    
    // When attempting to create a Username
    let result = Username::new(input);
    
    // Then it should fail with a validation error
    assert!(matches!(result, Err(UsernameError::Empty)));
}
```

### Test Module Location

Unit tests go in the same file, at the bottom:

```rust
// Production code above...

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test_something() {
        // ...
    }
}
```

Integration tests go in `tests/` directory.

## Test Crates

| Crate | Purpose |
|-------|---------|
| `test-case` | Parameterized tests |
| `mockall` | Trait mocks |
| `tempfile` | Temporary files/dirs |
| `maplit` | Collection literals |

## Parameterized Tests with test_case

**Prefer `test_case` over individual test functions** when you have:
- Multiple inputs testing the same success behavior
- Multiple inputs testing the same error case  
- Input/output pairs that follow a pattern

**Rule of thumb**: If you're copy-pasting a test and only changing the input, use `test_case` instead.

```rust
use test_case::test_case;

// Testing multiple invalid inputs
#[test_case("" ; "empty string")]
#[test_case("   " ; "whitespace only")]
#[test_case("\t\n" ; "tabs and newlines")]
fn rejects_blank_input(input: &str) {
    let result = Username::new(input);
    assert!(result.is_err());
}

// Testing input/output transformations
#[test_case("./foo", "foo" ; "strips leading dot-slash")]
#[test_case("foo/../bar", "bar" ; "resolves parent directory")]
#[test_case(".", "." ; "preserves dot")]
fn canonicalizes_path(input: &str, expected: &str) {
    let result = ContextId::try_new(input).unwrap();
    assert_eq!(result.as_str(), expected);
}
```

## Mocking with mockall

Add `#[automock]` to traits for testing:

```rust
#[automock]
#[async_trait]
pub trait Repository {
    async fn get(&self, id: &str) -> Result<Option<Item>, Self::Error>;
}

#[test]
fn uses_repository() {
    let mut mock = MockRepository::new();
    mock.expect_get()
        .with(predicate::eq("123"))
        .returning(|_| Ok(Some(Item::default())));
    
    let result = service_under_test(mock);
    assert!(result.is_ok());
}
```

## Assertions

Prefer `assert!(matches!())` for pattern matching:

```rust
// Good
assert!(matches!(result, Err(MyError::NotFound { .. })));

// Avoid verbose match blocks
```

Compare entire structs, not individual fields:

```rust
// Good
assert_eq!(actual, expected);

// Avoid
assert_eq!(actual.id, expected.id);
assert_eq!(actual.name, expected.name);
```

## JSON in Tests

Use `json!` macro, fully qualified:

```rust
let value: MyType = serde_json::from_value(json!({
    "name": "test",
    "count": 42
})).unwrap();
```

## What NOT to Test

- Compiler-generated derives (Debug, PartialEq)
- Trivial getters/setters
- Language features

Focus tests on business logic and domain rules.

## Allowed in Tests

- `unwrap()` and `expect()` are fine in test code
- Direct struct construction (when testing the struct itself)
