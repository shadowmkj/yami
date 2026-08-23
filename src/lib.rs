//! # `yami` - Minimal, Zero-Copy YAML Parser for Rust
//!
//! `yami` is a lightweight, zero-copy YAML parser designed for minimal allocations
//! and strict, unambiguous parsing semantics. Scalars borrow directly from the
//! input string as `&'a str`, eliminating heap string allocations during parsing.
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
