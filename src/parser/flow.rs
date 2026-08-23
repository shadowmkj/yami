//! Flow-style YAML parser handling JSON-compatible collections (`[...]` and `{...}`).
//!
//! Parses flow sequences and flow mappings with zero string allocations, borrowing
//! scalar tokens directly from the input text slice `'a`.

use crate::ast::{Entry, Yaml};
use crate::error::{ErrorKind, Position, Result, YamlError};

/// Cursor tracking character index and source position inside a flow text segment.
#[derive(Debug, Clone)]
pub struct FlowCursor<'a> {
    /// Initial slice for this flow fragment.
    initial: &'a str,
    /// Slice currently being parsed.
    rest: &'a str,
    /// Base position of `initial` in the source text.
    base_position: Position,
}

impl<'a> FlowCursor<'a> {
    /// Creates a new flow cursor at the given position.
    pub fn new(slice: &'a str, base_position: Position) -> Self {
        Self {
            initial: slice,
            rest: slice,
            base_position,
        }
    }

    /// Returns the current source position.
    pub fn current_position(&self) -> Position {
        let consumed = self.initial.len() - self.rest.len();
        let prefix = &self.initial[..consumed];
        let lines_count = prefix.matches('\n').count();

        if lines_count == 0 {
            let char_cols = prefix.chars().count();
            Position::new(
                self.base_position.line,
                self.base_position.column + char_cols,
                self.base_position.offset + consumed,
            )
        } else {
            let last_newline = prefix.rfind('\n').unwrap_or(0);
            let after_nl = &prefix[last_newline + 1..];
            let char_cols = after_nl.chars().count();
            Position::new(
                self.base_position.line + lines_count,
                1 + char_cols,
                self.base_position.offset + consumed,
            )
        }
    }

    /// Skips whitespace and full comments within flow syntax.
    pub fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.rest = self.rest.trim_start_matches(|c: char| c.is_whitespace());
            if self.rest.starts_with('#') {
                if let Some(pos) = self.rest.find('\n') {
                    self.rest = &self.rest[pos + 1..];
                } else {
                    self.rest = "";
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Returns the next character without advancing the cursor.
    #[inline]
    pub fn peek_char(&self) -> Option<char> {
        self.rest.chars().next()
    }

    /// Advances the cursor by one character if it matches expected character.
    pub fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.rest = &self.rest[expected.len_utf8()..];
            true
        } else {
            false
        }
    }

    /// Returns the remaining unconsumed slice.
    #[inline]
    pub const fn remaining(&self) -> &'a str {
        self.rest
    }
}

// ==============================================================================
// Flow Value Parsing
// ==============================================================================

/// Parses any flow value (scalar, flow sequence, or flow mapping).
pub fn parse_flow_value<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    cursor.skip_whitespace_and_comments();

    match cursor.peek_char() {
        Some('[') => parse_flow_sequence(cursor),
        Some('{') => parse_flow_mapping(cursor),
        Some('\'') => parse_single_quoted_scalar(cursor),
        Some('"') => parse_double_quoted_scalar(cursor),
        Some(']' | '}' | ',') => {
            let pos = cursor.current_position();
            Err(YamlError::new(ErrorKind::ExpectedValue, pos))
        }
        Some(_) => parse_plain_flow_scalar(cursor),
        None => {
            let pos = cursor.current_position();
            Err(YamlError::new(ErrorKind::UnexpectedEof, pos))
        }
    }
}

/// Parses a flow sequence enclosed in brackets `[...]`.
pub fn parse_flow_sequence<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let start_pos = cursor.current_position();
    if !cursor.consume_char('[') {
        return Err(YamlError::new(
            ErrorKind::UnexpectedCharacter(cursor.peek_char().unwrap_or('\0')),
            start_pos,
        ));
    }

    let mut items = Vec::new();

    loop {
        cursor.skip_whitespace_and_comments();

        if cursor.consume_char(']') {
            return Ok(Yaml::Sequence(items));
        }

        if cursor.rest.is_empty() {
            return Err(YamlError::new(ErrorKind::UnclosedDelimiter('['), start_pos));
        }

        let item = parse_flow_value(cursor)?;
        items.push(item);

        cursor.skip_whitespace_and_comments();

        if cursor.consume_char(',') {
            cursor.skip_whitespace_and_comments();
            if cursor.consume_char(']') {
                // Trailing comma allowed in flow sequence: [a, b,]
                return Ok(Yaml::Sequence(items));
            }
        } else if cursor.consume_char(']') {
            return Ok(Yaml::Sequence(items));
        } else if cursor.rest.is_empty() {
            return Err(YamlError::new(ErrorKind::UnclosedDelimiter('['), start_pos));
        } else {
            let pos = cursor.current_position();
            return Err(YamlError::new(
                ErrorKind::InvalidFlowSyntax("expected ',' or ']' in flow sequence"),
                pos,
            ));
        }
    }
}

/// Parses a flow mapping enclosed in braces `{...}`.
pub fn parse_flow_mapping<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let start_pos = cursor.current_position();
    if !cursor.consume_char('{') {
        return Err(YamlError::new(
            ErrorKind::UnexpectedCharacter(cursor.peek_char().unwrap_or('\0')),
            start_pos,
        ));
    }

    let mut entries = Vec::new();

    loop {
        cursor.skip_whitespace_and_comments();

        if cursor.consume_char('}') {
            return Ok(Yaml::Mapping(entries));
        }

        if cursor.rest.is_empty() {
            return Err(YamlError::new(ErrorKind::UnclosedDelimiter('{'), start_pos));
        }

        let key_node = parse_flow_key(cursor)?;
        cursor.skip_whitespace_and_comments();

        if !cursor.consume_char(':') {
            let pos = cursor.current_position();
            return Err(YamlError::new(ErrorKind::ExpectedColon, pos));
        }

        let value_node = parse_flow_value(cursor)?;
        entries.push(Entry::new(key_node, value_node));

        cursor.skip_whitespace_and_comments();

        if cursor.consume_char(',') {
            cursor.skip_whitespace_and_comments();
            if cursor.consume_char('}') {
                // Trailing comma allowed in flow mapping: {a: 1,}
                return Ok(Yaml::Mapping(entries));
            }
        } else if cursor.consume_char('}') {
            return Ok(Yaml::Mapping(entries));
        } else if cursor.rest.is_empty() {
            return Err(YamlError::new(ErrorKind::UnclosedDelimiter('{'), start_pos));
        } else {
            let pos = cursor.current_position();
            return Err(YamlError::new(
                ErrorKind::InvalidFlowSyntax("expected ',' or '}' in flow mapping"),
                pos,
            ));
        }
    }
}

/// Parses a mapping key inside a flow mapping.
fn parse_flow_key<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    cursor.skip_whitespace_and_comments();
    match cursor.peek_char() {
        Some('\'') => parse_single_quoted_scalar(cursor),
        Some('"') => parse_double_quoted_scalar(cursor),
        Some('[' | '{' | '}' | ']' | ',') => {
            let pos = cursor.current_position();
            Err(YamlError::new(ErrorKind::ExpectedKey, pos))
        }
        Some(_) => parse_plain_key_scalar(cursor),
        None => {
            let pos = cursor.current_position();
            Err(YamlError::new(ErrorKind::UnexpectedEof, pos))
        }
    }
}

/// Parses a plain scalar within flow collections up to delimiters `,`, `]`, `}`, or newline.
fn parse_plain_flow_scalar<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let mut end = 0;
    for (idx, ch) in cursor.rest.char_indices() {
        if matches!(ch, ',' | ']' | '}' | '\n' | '\r') {
            end = idx;
            break;
        }
        if ch == ':' {
            // Check if colon is followed by whitespace or flow delimiter
            let next_slice = &cursor.rest[idx + 1..];
            if next_slice.is_empty()
                || next_slice
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_whitespace() || matches!(c, ',' | ']' | '}'))
            {
                end = idx;
                break;
            }
        }
        end = idx + ch.len_utf8();
    }

    let raw = &cursor.rest[..end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        let pos = cursor.current_position();
        return Err(YamlError::new(ErrorKind::ExpectedValue, pos));
    }

    cursor.rest = &cursor.rest[end..];
    Ok(Yaml::Scalar(trimmed))
}

/// Parses a plain key scalar within flow mapping up to `:`.
fn parse_plain_key_scalar<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let mut end = 0;
    for (idx, ch) in cursor.rest.char_indices() {
        if matches!(ch, ':' | ',' | '}' | '\n' | '\r') {
            end = idx;
            break;
        }
        end = idx + ch.len_utf8();
    }

    let raw = &cursor.rest[..end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        let pos = cursor.current_position();
        return Err(YamlError::new(ErrorKind::ExpectedKey, pos));
    }

    cursor.rest = &cursor.rest[end..];
    Ok(Yaml::Scalar(trimmed))
}

/// Parses single-quoted scalar `'...'`.
fn parse_single_quoted_scalar<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let start_pos = cursor.current_position();
    cursor.consume_char('\'');

    let mut end_offset = None;
    for (idx, ch) in cursor.rest.char_indices() {
        if ch == '\'' {
            end_offset = Some(idx);
            break;
        }
    }

    match end_offset {
        Some(idx) => {
            let inner = &cursor.rest[..idx];
            cursor.rest = &cursor.rest[idx + 1..];
            Ok(Yaml::Scalar(inner))
        }
        None => Err(YamlError::new(
            ErrorKind::UnclosedDelimiter('\''),
            start_pos,
        )),
    }
}

/// Parses double-quoted scalar `"..."`.
fn parse_double_quoted_scalar<'a>(cursor: &mut FlowCursor<'a>) -> Result<Yaml<'a>> {
    let start_pos = cursor.current_position();
    cursor.consume_char('"');

    let mut end_offset = None;
    let mut is_escaped = false;

    for (idx, ch) in cursor.rest.char_indices() {
        if is_escaped {
            is_escaped = false;
            continue;
        }
        if ch == '\\' {
            is_escaped = true;
            continue;
        }
        if ch == '"' {
            end_offset = Some(idx);
            break;
        }
    }

    match end_offset {
        Some(idx) => {
            let inner = &cursor.rest[..idx];
            cursor.rest = &cursor.rest[idx + 1..];
            Ok(Yaml::Scalar(inner))
        }
        None => Err(YamlError::new(ErrorKind::UnclosedDelimiter('"'), start_pos)),
    }
}

// ==============================================================================
// Unit Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_sequence_empty_and_simple() {
        let input = "[1, 2, 3]";
        let mut cursor = FlowCursor::new(input, Position::start());
        let val = parse_flow_sequence(&mut cursor).expect("should parse sequence");
        assert_eq!(
            val,
            Yaml::Sequence(vec![
                Yaml::Scalar("1"),
                Yaml::Scalar("2"),
                Yaml::Scalar("3"),
            ])
        );
    }

    #[test]
    fn test_flow_sequence_trailing_comma_and_quotes() {
        let input = "['apple', \"banana\", 'cherry',]";
        let mut cursor = FlowCursor::new(input, Position::start());
        let val = parse_flow_sequence(&mut cursor).expect("should parse sequence");
        assert_eq!(
            val,
            Yaml::Sequence(vec![
                Yaml::Scalar("apple"),
                Yaml::Scalar("banana"),
                Yaml::Scalar("cherry"),
            ])
        );
    }

    #[test]
    fn test_flow_mapping_simple_and_nested() {
        let input = "{ name: 'Alice', age: 30, tags: [rust, dev] }";
        let mut cursor = FlowCursor::new(input, Position::start());
        let val = parse_flow_mapping(&mut cursor).expect("should parse mapping");

        assert_eq!(
            val,
            Yaml::Mapping(vec![
                Entry::new(Yaml::Scalar("name"), Yaml::Scalar("Alice")),
                Entry::new(Yaml::Scalar("age"), Yaml::Scalar("30")),
                Entry::new(
                    Yaml::Scalar("tags"),
                    Yaml::Sequence(vec![Yaml::Scalar("rust"), Yaml::Scalar("dev")])
                ),
            ])
        );
    }

    #[test]
    fn test_unclosed_brackets_error() {
        let input = "[1, 2, 3";
        let mut cursor = FlowCursor::new(input, Position::start());
        let err = parse_flow_sequence(&mut cursor).expect_err("should fail unclosed bracket");
        assert_eq!(err.kind, ErrorKind::UnclosedDelimiter('['));
    }
}
