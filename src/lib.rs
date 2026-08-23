//! # `yami` - Minimal, Zero-Copy YAML Parser for Rust
//!
//! `yami` is a lightweight, zero-copy YAML parser designed for minimal allocations
//! and strict, unambiguous parsing semantics. Scalars borrow directly from the
//! input string as `&'a str`, eliminating heap string allocations during parsing.
//!
//! ## Background & Motivation
//!
//! Standard YAML parsers in Rust (such as `serde_yaml`) enforce `DeserializeOwned`
//! trait bounds that prevent deserializing directly into types borrowing string slices
//! (`&'a str`) from the input document. This limitation is discussed in
//! [dtolnay/serde-yaml#94](https://github.com/dtolnay/serde-yaml/issues/94) and prompted
//! the call for a minimal zero-copy YAML parser in
//! [dtolnay/request-for-implementation#9](https://github.com/dtolnay/request-for-implementation/issues/9).
//!
//! `yami` provides a direct, zero-copy AST representation where every scalar is an
//! unallocated `&'a str` slice into the source text.
//!
//! ## Core Types
//! - [`Yaml<'a>`]: Represents any YAML node (`Scalar(&'a str)`, `Sequence(Vec<Yaml<'a>>) `, `Mapping(Vec<Entry<'a>>)`).
//! - [`Entry<'a>`]: A key-value pair inside a YAML mapping.
//! - [`YamlError`]: Precise parsing errors including 1-indexed `(line, column)` positions.

pub mod ast;
pub mod error;
pub mod parser;
pub mod scanner;

pub use ast::{Entry, Yaml};
pub use error::{ErrorKind, Position, Result, YamlError};
pub use parser::parse;
pub use scanner::{Line, Scanner};
