//! Abstract Syntax Tree (AST) representations for parsed YAML data.
//!
//! The AST is designed for zero-copy borrowing from the source string.
//! String scalars, booleans, and numeric representations are retained as
//! slices `&'a str` into the original input buffer, avoiding heap allocations.

use crate::error::{ErrorKind, Position, Result, YamlError};
use std::fmt;
use std::ops::Index;

// ==============================================================================
// AST Definitions
// ==============================================================================

/// A key-value association within a YAML mapping.
///
/// In YAML, keys are commonly string scalars, but can technically be any YAML
/// node. Both key and value share the lifetime `'a` of the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    /// The entry's key node.
    pub key: Yaml<'a>,
    /// The entry's associated value node.
    pub value: Yaml<'a>,
}

impl<'a> Entry<'a> {
    /// Creates a new mapping entry from a key and value pair.
    #[inline]
    pub const fn new(key: Yaml<'a>, value: Yaml<'a>) -> Self {
        Self { key, value }
    }

    /// Borrows the key node.
    #[inline]
    pub const fn key(&self) -> &Yaml<'a> {
        &self.key
    }

    /// Borrows the value node.
    #[inline]
    pub const fn value(&self) -> &Yaml<'a> {
        &self.value
    }

    /// Consumes the entry and returns the underlying key and value.
    #[inline]
    pub fn into_parts(self) -> (Yaml<'a>, Yaml<'a>) {
        (self.key, self.value)
    }
}

/// Represents any YAML node, borrowing string slices directly from the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Yaml<'a> {
    /// A raw scalar value (unquoted text, quoted string, boolean literal, or number).
    Scalar(&'a str),

    /// A sequence of YAML nodes (block style `- item` or flow style `[a, b]`).
    Sequence(Vec<Yaml<'a>>),

    /// An ordered collection of key-value pairs (block style `k: v` or flow style `{k: v}`).
    Mapping(Vec<Entry<'a>>),
}

// ==============================================================================
// AST Inspection and Navigation Methods
// ==============================================================================

impl<'a> Yaml<'a> {
    /// Returns `true` if this node is a scalar.
    #[inline]
    pub const fn is_scalar(&self) -> bool {
        matches!(self, Self::Scalar(_))
    }

    /// Returns `true` if this node is a sequence.
    #[inline]
    pub const fn is_sequence(&self) -> bool {
        matches!(self, Self::Sequence(_))
    }

    /// Returns `true` if this node is a mapping.
    #[inline]
    pub const fn is_mapping(&self) -> bool {
        matches!(self, Self::Mapping(_))
    }

    /// Returns the borrowed scalar slice if this node is a `Yaml::Scalar`.
    #[inline]
    pub const fn as_scalar(&self) -> Option<&'a str> {
        match self {
            Self::Scalar(s) => Some(*s),
            _ => None,
        }
    }

    /// Alias for `as_scalar()` for ergonomic string access.
    #[inline]
    pub const fn as_str(&self) -> Option<&'a str> {
        self.as_scalar()
    }

    /// Returns a slice of sequence elements if this node is a `Yaml::Sequence`.
    #[inline]
    pub fn as_sequence(&self) -> Option<&[Yaml<'a>]> {
        match self {
            Self::Sequence(vec) => Some(vec.as_slice()),
            _ => None,
        }
    }

    /// Returns a slice of mapping entries if this node is a `Yaml::Mapping`.
    #[inline]
    pub fn as_mapping(&self) -> Option<&[Entry<'a>]> {
        match self {
            Self::Mapping(entries) => Some(entries.as_slice()),
            _ => None,
        }
    }

    /// Looks up a value in a mapping by its scalar key string.
    ///
    /// Performs a linear scan over the mapping entries. Returns `None` if the node
    /// is not a mapping or if no entry has a matching scalar key.
    pub fn get(&self, key: &str) -> Option<&Yaml<'a>> {
        match self {
            Self::Mapping(entries) => entries.iter().find_map(|entry| match &entry.key {
                Self::Scalar(k) if *k == key => Some(&entry.value),
                _ => None,
            }),
            _ => None,
        }
    }

    /// Looks up a mutable reference to a value in a mapping by its scalar key string.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Yaml<'a>> {
        match self {
            Self::Mapping(entries) => entries.iter_mut().find_map(|entry| match &entry.key {
                Self::Scalar(k) if *k == key => Some(&mut entry.value),
                _ => None,
            }),
            _ => None,
        }
    }

    /// Looks up an element in a sequence by zero-based index.
    pub fn get_idx(&self, index: usize) -> Option<&Yaml<'a>> {
        match self {
            Self::Sequence(items) => items.get(index),
            _ => None,
        }
    }

    /// Returns the number of elements in a sequence or entries in a mapping,
    /// or `1` for scalars.
    pub fn len(&self) -> usize {
        match self {
            Self::Scalar(_) => 1,
            Self::Sequence(items) => items.len(),
            Self::Mapping(entries) => entries.len(),
        }
    }

    /// Returns `true` if the node is an empty sequence, empty mapping, or empty scalar.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Scalar(s) => s.is_empty(),
            Self::Sequence(items) => items.is_empty(),
            Self::Mapping(entries) => entries.is_empty(),
        }
    }

    // ==============================================================================
    // Typed Scalar Conversion Helpers
    // ==============================================================================

    /// Parses the scalar value as a boolean (`true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0`).
    pub fn to_bool(&self) -> Result<bool> {
        let raw = self.as_scalar().ok_or_else(|| {
            YamlError::new(
                ErrorKind::Custom("expected scalar for boolean conversion".into()),
                Position::start(),
            )
        })?;

        match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Ok(true),
            "false" | "no" | "off" | "0" => Ok(false),
            other => Err(YamlError::new(
                ErrorKind::Custom(format!("cannot parse '{other}' as boolean")),
                Position::start(),
            )),
        }
    }

    /// Parses the scalar value as an `i64` integer.
    pub fn to_i64(&self) -> Result<i64> {
        let raw = self.as_scalar().ok_or_else(|| {
            YamlError::new(
                ErrorKind::Custom("expected scalar for integer conversion".into()),
                Position::start(),
            )
        })?;

        raw.trim().parse::<i64>().map_err(|e| {
            YamlError::new(
                ErrorKind::Custom(format!("cannot parse '{raw}' as integer: {e}")),
                Position::start(),
            )
        })
    }

    /// Parses the scalar value as an `f64` floating point number.
    pub fn to_f64(&self) -> Result<f64> {
        let raw = self.as_scalar().ok_or_else(|| {
            YamlError::new(
                ErrorKind::Custom("expected scalar for float conversion".into()),
                Position::start(),
            )
        })?;

        raw.trim().parse::<f64>().map_err(|e| {
            YamlError::new(
                ErrorKind::Custom(format!("cannot parse '{raw}' as float: {e}")),
                Position::start(),
            )
        })
    }
}

// ==============================================================================
// Indexing Operator Implementations
// ==============================================================================

impl<'a> Index<&str> for Yaml<'a> {
    type Output = Yaml<'a>;

    #[inline]
    fn index(&self, key: &str) -> &Self::Output {
        self.get(key)
            .unwrap_or_else(|| panic!("key '{key}' not found in YAML mapping"))
    }
}

impl<'a> Index<usize> for Yaml<'a> {
    type Output = Yaml<'a>;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get_idx(index)
            .unwrap_or_else(|| panic!("index {index} out of bounds for YAML sequence"))
    }
}

// ==============================================================================
// Display Implementation
// ==============================================================================

impl<'a> fmt::Display for Yaml<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_yaml(self, f, 0)
    }
}

fn format_yaml(val: &Yaml<'_>, f: &mut fmt::Formatter<'_>, indent_level: usize) -> fmt::Result {
    let indent = "  ".repeat(indent_level);
    match val {
        Yaml::Scalar(s) => write!(f, "{s}"),
        Yaml::Sequence(items) => {
            if items.is_empty() {
                write!(f, "[]")
            } else {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{indent}- ")?;
                    match item {
                        Yaml::Scalar(s) => write!(f, "{s}")?,
                        Yaml::Sequence(_) | Yaml::Mapping(_) => {
                            writeln!(f)?;
                            format_yaml(item, f, indent_level + 1)?;
                        }
                    }
                }
                Ok(())
            }
        }
        Yaml::Mapping(entries) => {
            if entries.is_empty() {
                write!(f, "{{}}")
            } else {
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{indent}{}: ", entry.key)?;
                    match &entry.value {
                        Yaml::Scalar(s) => write!(f, "{s}")?,
                        Yaml::Sequence(_) | Yaml::Mapping(_) => {
                            writeln!(f)?;
                            format_yaml(&entry.value, f, indent_level + 1)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

// ==============================================================================
// Compile-Fail Verification Tests
// ==============================================================================

/// Compile-fail test verifying that `Yaml<'a>` cannot outlive the borrowed `&'a str` buffer.
///
/// This doctest validates borrow checker lifetime safety: returning a `Yaml` node
/// that borrows from a locally scoped `String` that gets dropped must fail compilation
/// with lifetime borrow error `E0515`.
///
/// ```compile_fail
/// use yami::{parse, Yaml};
///
/// fn parse_dropped_string() -> Yaml<'static> {
///     let local_string = String::from("key: value");
///     let parsed = parse(&local_string).unwrap();
///     parsed // compile_fail: `local_string` is dropped at end of function while still borrowed
/// }
/// ```
#[allow(dead_code)]
fn _compile_fail_lifetime_enforcement() {}

// ==============================================================================
// Unit Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_helpers() {
        let scalar = Yaml::Scalar("42");
        assert!(scalar.is_scalar());
        assert!(!scalar.is_sequence());
        assert!(!scalar.is_mapping());
        assert_eq!(scalar.as_scalar(), Some("42"));
        assert_eq!(scalar.as_str(), Some("42"));
        assert_eq!(scalar.to_i64().expect("should parse 42 as i64"), 42);
        assert!((scalar.to_f64().expect("should parse 42 as f64") - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_boolean_conversions() {
        assert!(Yaml::Scalar("true").to_bool().expect("true should parse"));
        assert!(Yaml::Scalar("yes").to_bool().expect("yes should parse"));
        assert!(Yaml::Scalar("ON").to_bool().expect("ON should parse"));
        assert!(Yaml::Scalar("1").to_bool().expect("1 should parse"));

        assert!(!Yaml::Scalar("false")
            .to_bool()
            .expect("false should parse"));
        assert!(!Yaml::Scalar("no").to_bool().expect("no should parse"));
        assert!(!Yaml::Scalar("off").to_bool().expect("off should parse"));
        assert!(!Yaml::Scalar("0").to_bool().expect("0 should parse"));

        assert!(Yaml::Scalar("invalid").to_bool().is_err());
    }

    #[test]
    fn test_sequence_helpers() {
        let seq = Yaml::Sequence(vec![
            Yaml::Scalar("first"),
            Yaml::Scalar("second"),
            Yaml::Scalar("third"),
        ]);
        assert!(seq.is_sequence());
        assert_eq!(seq.len(), 3);
        assert_eq!(seq.get_idx(1), Some(&Yaml::Scalar("second")));
        assert_eq!(seq[0], Yaml::Scalar("first"));
        assert_eq!(seq[2], Yaml::Scalar("third"));
    }

    #[test]
    fn test_mapping_helpers() {
        let mapping = Yaml::Mapping(vec![
            Entry::new(Yaml::Scalar("name"), Yaml::Scalar("Alice")),
            Entry::new(Yaml::Scalar("age"), Yaml::Scalar("30")),
        ]);

        assert!(mapping.is_mapping());
        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping.get("name"), Some(&Yaml::Scalar("Alice")));
        assert_eq!(mapping["age"], Yaml::Scalar("30"));
        assert_eq!(
            mapping["age"]
                .to_i64()
                .expect("age should parse as integer"),
            30
        );
        assert_eq!(mapping.get("nonexistent"), None);
    }

    #[test]
    fn test_display_formatting() {
        let mapping = Yaml::Mapping(vec![
            Entry::new(Yaml::Scalar("host"), Yaml::Scalar("localhost")),
            Entry::new(Yaml::Scalar("port"), Yaml::Scalar("8080")),
        ]);
        let formatted = mapping.to_string();
        assert!(formatted.contains("host: localhost"));
        assert!(formatted.contains("port: 8080"));
    }
}
