# `yami`

<div align="center">

[![CI](https://github.com/shadowmkj/yami/actions/workflows/ci.yml/badge.svg)](https://github.com/shadowmkj/yami/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/yami.svg)](https://crates.io/crates/yami)
[![Documentation](https://docs.rs/yami/badge.svg)](https://docs.rs/yami)
[![Downloads](https://img.shields.io/crates/d/yami.svg)](https://crates.io/crates/yami)
[![Codecov](https://codecov.io/gh/shadowmkj/yami/branch/main/graph/badge.svg)](https://codecov.io/gh/shadowmkj/yami)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust MSRV](https://img.shields.io/badge/rustc-1.80+-blue.svg)](#requirements)

**A lightweight, zero-copy, minimal-allocation YAML parser in Rust.**

</div>

---

## Overview

`yami` is an ultra-fast, zero-copy YAML parser designed for minimal heap allocations, deterministic behavior, and strict parsing semantics.

Unlike traditional YAML libraries that allocate owned `String`s for every scalar, key, and value in a document, `yami` returns an Abstract Syntax Tree (AST) where **all scalars borrow directly from the input text slice (`&'a str`)**. This eliminates heap churn and enables zero-allocation deserialization into domain structs.

### Background & Motivation

In the Rust ecosystem, standard YAML parsers like `serde_yaml` cannot deserialize into structs containing borrowed string slices (`&'a str`) due to internal stream buffering and `DeserializeOwned` trait bounds:

```rust
// Attempting zero-copy deserialization with serde_yaml:
struct App<'a> {
    name: &'a str,
}

let app: App = serde_yaml::from_str(&yaml_str).unwrap();
//  ^^^ ERROR: the trait `for<'de> Deserialize<'de>` is not implemented for `App<'_>`
//      note: required by `serde_yaml::from_str` due to `DeserializeOwned` requirements
```

This limitation is documented in [dtolnay/serde-yaml#94](https://github.com/dtolnay/serde-yaml/issues/94) ("*Can't deserialize borrowed str with from_str*") and highlighted in [dtolnay/request-for-implementation#9](https://github.com/dtolnay/request-for-implementation/issues/9) ("*Minimal YAML parser*").

`yami` was built specifically to solve this problem:
1. **Zero-Copy AST**: Directly parses `&'a str` into `Yaml<'a>` with scalars borrowing from the source buffer without heap allocations.
2. **Strict Semantics**: Inspired by [StrictYAML](https://github.com/crdoconnor/strictyaml), `yami` eliminates the complexity and ambiguities of full YAML 1.2 (such as arbitrary object tags and silent type coercions), focusing on fast, clean, and deterministic configuration parsing.

### Why `yami`?

- **⚡ Zero-Copy Scalars**: All plain scalars, quoted strings (`'...'` / `"..."`), booleans, and numbers borrow directly as `&'a str` from the input buffer.
- **🚀 Ultra-Fast Throughput**: Parses real-world configuration payloads at **~440,000 parses/sec** (~2.26 µs per document, ~260 MB/sec throughput).
- **🛡️ Lifetime Safe**: Verified with `compile_fail` doctests to ensure borrowed AST nodes cannot outlive their source buffer.
- **🎯 Precise Diagnostics**: Reports exact 1-indexed `(line, column)` coordinates and structured error variants via `thiserror`.
- **📐 Strict & Unambiguous**: Rejects ambiguous constructs such as tabs in indentation positions (`ErrorKind::TabInIndentation`) and detects duplicate keys (`ErrorKind::DuplicateKey`).
- **🧩 Block & Flow Hybrid**: Seamlessly parses block mappings, block sequences, compact mappings (`- key: val`), and JSON-style inline flow collections (`[...]` and `{...}`).

---

## Installation

Add `yami` to your `Cargo.toml`:

```bash
cargo add yami
```

Or manually specify it in `Cargo.toml`:

```toml
[dependencies]
yami = "0.1.0"
```

---

## Quick Start

```rust
use yami::{parse, Yaml};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
service:
  name: auth-microservice
  port: 8080
  debug: true

database:
  host: db.internal
  replicas:
    - replica-1.internal
    - replica-2.internal

tags: [security, fast, zero-copy]
"#;

    // Parse into zero-copy AST
    let doc = parse(input)?;

    // Access nested values via ergonomic indexing
    assert_eq!(doc["service"]["name"], Yaml::Scalar("auth-microservice"));
    assert_eq!(doc["service"]["port"].to_i64()?, 8080);
    assert!(doc["service"]["debug"].to_bool()?);

    // Iterate over sequence elements
    let replicas = doc["database"]["replicas"].as_sequence().unwrap();
    assert_eq!(replicas[0], Yaml::Scalar("replica-1.internal"));

    // Flow sequences are parsed seamlessly
    let tags = doc["tags"].as_sequence().unwrap();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], Yaml::Scalar("security"));

    Ok(())
}
```

---

## Zero-Copy Struct Extraction

You can extract configuration data directly into Rust domain models without allocating any `String` heap buffers:

```rust
use yami::{parse, Yaml};

#[derive(Debug, PartialEq)]
pub struct ServerConfig<'a> {
    pub name: &'a str,
    pub host: &'a str,
    pub port: i64,
    pub debug: bool,
    pub replicas: Vec<&'a str>,
}

impl<'a> ServerConfig<'a> {
    pub fn from_yaml(doc: &Yaml<'a>) -> Result<Self, Box<dyn std::error::Error>> {
        let name = doc["service"]["name"].as_scalar().ok_or("missing name")?;
        let host = doc["service"]["host"].as_scalar().ok_or("missing host")?;
        let port = doc["service"]["port"].to_i64()?;
        let debug = doc["service"]["debug"].to_bool()?;

        let replicas = doc["database"]["replicas"]
            .as_sequence()
            .ok_or("missing replicas")?
            .iter()
            .filter_map(|item| item.as_scalar())
            .collect();

        Ok(Self { name, host, port, debug, replicas })
    }
}
```

---

## Core Data Model

```rust
pub enum Yaml<'a> {
    /// Zero-copy scalar slice into the input text
    Scalar(&'a str),
    /// Sequence of YAML nodes (block `- ` or flow `[...]`)
    Sequence(Vec<Yaml<'a>>),
    /// Key-value association mapping (block `key: val` or flow `{...}`)
    Mapping(Vec<Entry<'a>>),
}

pub struct Entry<'a> {
    pub key: Yaml<'a>,
    pub value: Yaml<'a>,
}
```

### Navigation & Helper Methods

| Method | Return Type | Description |
|---|---|---|
| `doc["key"]` | `&Yaml<'a>` | Index mapping by key (returns `&Yaml::Scalar("")` if missing) |
| `doc[index]` | `&Yaml<'a>` | Index sequence by position (returns `&Yaml::Scalar("")` if out-of-bounds) |
| `doc.get("key")` | `Option<&Yaml<'a>>` | Lookup mapping value by key |
| `doc.as_scalar()` | `Option<&'a str>` | Borrows scalar slice |
| `doc.as_sequence()` | `Option<&[Yaml<'a>]>` | Borrows sequence slice |
| `doc.as_mapping()` | `Option<&[Entry<'a>]>` | Borrows mapping entries slice |
| `doc.to_bool()` | `Result<bool, YamlError>` | Parses `true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0` |
| `doc.to_i64()` | `Result<i64, YamlError>` | Parses integer scalar |
| `doc.to_f64()` | `Result<f64, YamlError>` | Parses floating-point scalar |

---

## Error Handling & Diagnostics

Errors provide structured variants via `thiserror` and 1-indexed `(line, column)` source locations:

```rust
use yami::{parse, ErrorKind};

let malformed = "
server:
\thost: localhost  # Tabs in indentation are strictly forbidden
";

match parse(malformed) {
    Ok(_) => unreachable!(),
    Err(err) => {
        println!("Error: {}", err);
        // Output: yaml parse error at line 3, column 1: tab character is not allowed for indentation

        assert!(matches!(err.kind, ErrorKind::TabInIndentation));
        assert_eq!(err.position.line, 3);
        assert_eq!(err.position.column, 1);
    }
}
```

---

## Benchmarks & Performance

Run the release benchmark on your machine:

```bash
cargo run --release --example benchmark
```

### Benchmark Results (Apple Silicon M-Series)

```text
============================================================
  yami Zero-Copy YAML Parser Benchmark
============================================================
Payload size:     623 bytes
Iterations:       100,000
Total time:       226.41 ms
Time per parse:   2.26 µs (2264 ns)
Throughput:       441,683 parses/sec
Bandwidth:        262.42 MB/sec
============================================================
```

---

## Development & Testing

```bash
# Run unit and integration tests
cargo test

# Run snapshot regression tests with insta
cargo test --test snapshot_tests

# Run property-based fuzz tests with quickcheck
cargo test --test property_tests

# Run doctests (including compile_fail lifetime tests)
cargo test --doc

# Run linter checks
cargo clippy --all-targets --all-features -- -D warnings

# Check code formatting
cargo fmt --all -- --check
```

---

## Requirements

- **Minimum Supported Rust Version (MSRV)**: Rust `1.80.0` or later.

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
