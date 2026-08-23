# `yami`

A lightweight, zero-copy, minimal-allocation YAML parser in Rust.

## Features

- **Zero-Copy Scalars**: Plain and quoted strings, numbers, and booleans borrow directly as `&'a str` from the input slice, eliminating heap string allocations.
- **Strict, Unambiguous Parsing**: Block mappings, block sequences, compact list mappings (`- key: val`), and flow structures (`[...]` and `{...}`).
- **Ergonomic Access**: Direct indexing (`&doc["key"]`, `&doc[0]`), accessor methods, and typed scalar conversions (`to_bool()`, `to_i64()`, `to_f64()`).
- **Structured Diagnostics**: Precise 1-indexed `(line, column)` source locations powered by `thiserror`.

## Quick Start

```rust
use yami::{parse, Yaml};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
server:
  host: localhost
  port: 8080
  debug: true
tags: [fast, zero-copy, safe]
"#;

    let doc = parse(input)?;

    assert_eq!(doc["server"]["host"], Yaml::Scalar("localhost"));
    assert_eq!(doc["server"]["port"].to_i64()?, 8080);
    assert!(doc["server"]["debug"].to_bool()?);

    let tags = doc["tags"].as_sequence().unwrap();
    assert_eq!(tags[0], Yaml::Scalar("fast"));

    Ok(())
}
```

## Data Types

```rust
pub enum Yaml<'a> {
    Scalar(&'a str),
    Sequence(Vec<Yaml<'a>>),
    Mapping(Vec<Entry<'a>>),
}

pub struct Entry<'a> {
    pub key: Yaml<'a>,
    pub value: Yaml<'a>,
}
```

## Running Benchmarks

```bash
cargo run --release --example benchmark
```

## Testing

```bash
# Run unit and integration tests
cargo test

# Run snapshot tests with insta
cargo test --test snapshot_tests

# Run property-based fuzz tests with quickcheck
cargo test --test property_tests

# Run doctests including compile_fail lifetime borrow tests
cargo test --doc
```
