//! Error types and diagnostics for YAML parsing.
//!
//! Provides detailed source locations (line, column, and byte offset) to aid
//! in debugging malformed YAML inputs, with declarative error formatting via `thiserror`.

use std::fmt;
use thiserror::Error;

/// Represents a 1-indexed source code position within the input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number in characters.
    pub column: usize,
    /// 0-based byte offset from start of input.
    pub offset: usize,
}

impl Position {
    /// Creates a new source position.
    #[inline]
    pub const fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    /// Default starting position at the beginning of a document.
    #[inline]
    pub const fn start() -> Self {
        Self {
            line: 1,
            column: 1,
            offset: 0,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// The specific category of parsing error encountered.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErrorKind {
    /// Unexpected end of input when more tokens were expected.
    #[error("unexpected end of input")]
    UnexpectedEof,

    /// Indentation does not match expected block level or rule.
    #[error("invalid indentation: expected at least {expected} spaces, found {found}")]
    InvalidIndentation { expected: usize, found: usize },

    /// Tab character found in indentation where spaces are strictly required.
    #[error("tab characters are not allowed for indentation; use spaces")]
    TabInIndentation,

    /// An unexpected character was encountered.
    #[error("unexpected character '{0}'")]
    UnexpectedCharacter(char),

    /// Expected a colon `:` separating key and value.
    #[error("expected ':' separating mapping key and value")]
    ExpectedColon,

    /// Expected a mapping key or scalar identifier.
    #[error("expected mapping key")]
    ExpectedKey,

    /// Expected a value following a mapping key or sequence indicator.
    #[error("expected value")]
    ExpectedValue,

    /// Unclosed delimiter such as `[` or `{`.
    #[error("unclosed delimiter '{0}'")]
    UnclosedDelimiter(char),

    /// Malformed flow syntax (e.g. unclosed brackets or invalid separators).
    #[error("invalid flow syntax: {0}")]
    InvalidFlowSyntax(&'static str),

    /// Duplicate mapping key found in a mapping context.
    #[error("duplicate mapping key '{0}'")]
    DuplicateKey(String),

    /// General parsing error with custom message.
    #[error("{0}")]
    Custom(String),
}

/// A YAML parsing error complete with error classification and source location.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("yaml parse error at {position}: {kind}")]
pub struct YamlError {
    /// Detailed category and context of the error.
    pub kind: ErrorKind,
    /// Source position where the error occurred.
    pub position: Position,
}

impl YamlError {
    /// Creates a new `YamlError` at the specified position.
    #[inline]
    pub const fn new(kind: ErrorKind, position: Position) -> Self {
        Self { kind, position }
    }

    /// Convenience helper to create a custom error at a given position.
    pub fn custom(msg: impl Into<String>, position: Position) -> Self {
        Self {
            kind: ErrorKind::Custom(msg.into()),
            position,
        }
    }
}

/// Standard Result alias for YAML parsing operations.
pub type Result<T> = std::result::Result<T, YamlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let pos = Position::new(4, 12, 45);
        let err = YamlError::new(ErrorKind::TabInIndentation, pos);
        assert_eq!(
            err.to_string(),
            "yaml parse error at 4:12: tab characters are not allowed for indentation; use spaces"
        );
    }

    #[test]
    fn test_error_kinds_display() {
        let pos = Position::start();
        let eof_err = YamlError::new(ErrorKind::UnexpectedEof, pos);
        assert_eq!(
            eof_err.to_string(),
            "yaml parse error at 1:1: unexpected end of input"
        );

        let indent_err = YamlError::new(
            ErrorKind::InvalidIndentation {
                expected: 4,
                found: 2,
            },
            pos,
        );
        assert_eq!(
            indent_err.to_string(),
            "yaml parse error at 1:1: invalid indentation: expected at least 4 spaces, found 2"
        );
    }
}
