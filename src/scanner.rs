//! Zero-copy scanner and indentation tracker for YAML lines and tokens.
//!
//! Scans the source `&'a str` into structured line entries with space indentation levels,
//! line and column tracking, and comment stripping, without performing any heap allocations
//! for string content.

use crate::error::{ErrorKind, Position, Result, YamlError};

// ==============================================================================
// Scanned Line Model
// ==============================================================================

/// Represents a single non-empty, non-comment line in the YAML source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    /// The trimmed line content borrowing directly from the input buffer.
    pub content: &'a str,
    /// Number of leading space characters determining the indentation level.
    pub indent: usize,
    /// 1-based source line number.
    pub start_column: usize,
    /// 1-based source column number where content starts (i.e. `1 + indent`).
    pub line_number: usize,
    /// 0-based byte offset within the input text where `content` begins.
    pub byte_offset: usize,
}

impl<'a> Line<'a> {
    /// Returns the start source position of this line's content.
    #[inline]
    pub const fn start_position(&self) -> Position {
        Position::new(self.line_number, self.start_column, self.byte_offset)
    }

    /// Checks if this line starts with a block sequence indicator (`-` followed by space or EOL).
    #[inline]
    pub fn is_sequence_item(&self) -> bool {
        self.content == "-" || self.content.starts_with("- ") || self.content.starts_with("-\t")
    }

    /// Checks if this line represents a YAML document separator (`---` or `...`).
    #[inline]
    pub fn is_document_separator(&self) -> bool {
        self.content == "---" || self.content == "..."
    }
}

// ==============================================================================
// Scanner Implementation
// ==============================================================================

/// A line-by-line scanner that extracts structured YAML lines while tracking indentation
/// and enforcing YAML indentation safety rules (e.g. rejecting tabs in indentation).
#[derive(Debug, Clone)]
pub struct Scanner<'a> {
    /// The original input buffer.
    input: &'a str,
    /// Filtered non-empty, non-comment lines.
    lines: Vec<Line<'a>>,
    /// Current cursor index within `lines`.
    cursor: usize,
}

impl<'a> Scanner<'a> {
    /// Scans the given YAML input string into a sequence of parsed lines.
    ///
    /// Validates indentation rules (tabs are prohibited in indentation positions)
    /// and strips comments while preserving string quotes.
    pub fn new(input: &'a str) -> Result<Self> {
        let mut lines = Vec::new();
        let mut byte_offset = 0;

        for (line_number, raw_line) in (1..).zip(input.split('\n')) {
            // Trim carriage return if present (\r\n line ending)
            let trimmed_line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

            if let Some(line) = Self::process_line(trimmed_line, line_number, byte_offset)? {
                lines.push(line);
            }

            // Move offset past raw_line + newline character
            byte_offset += raw_line.len() + 1;
        }

        Ok(Self {
            input,
            lines,
            cursor: 0,
        })
    }

    /// Processes a single raw line of text: computes indentation, rejects tabs,
    /// strips trailing comments, and returns `Some(Line)` if non-empty.
    fn process_line(
        raw: &'a str,
        line_number: usize,
        line_start_offset: usize,
    ) -> Result<Option<Line<'a>>> {
        let mut indent = 0;
        let mut chars_iter = raw.char_indices();

        // Calculate leading space indentation and strictly enforce space-only indentation rules
        for (idx, ch) in chars_iter.by_ref() {
            match ch {
                ' ' => indent += 1,
                '\t' => {
                    // YAML standard explicitly forbids tab characters for indentation
                    return Err(YamlError::new(
                        ErrorKind::TabInIndentation,
                        Position::new(line_number, idx + 1, line_start_offset + idx),
                    ));
                }
                _ => break,
            }
        }

        // Entire line is whitespace
        if indent == raw.len() {
            return Ok(None);
        }

        let content_start = indent;
        let after_indent = &raw[content_start..];

        // Strip inline comments while preserving # characters inside quotes
        let stripped_content = Self::strip_comments(after_indent);
        let trimmed_content = stripped_content.trim_end();

        if trimmed_content.is_empty() {
            return Ok(None);
        }

        // Subslice directly into raw input
        let start_column = indent + 1;
        let byte_offset = line_start_offset + content_start;

        Ok(Some(Line {
            content: trimmed_content,
            indent,
            line_number,
            start_column,
            byte_offset,
        }))
    }

    /// Strips trailing inline comments (`# ...`) from a line, ensuring `#` characters
    /// inside single or double quoted substrings are not treated as comments.
    fn strip_comments(text: &'a str) -> &'a str {
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut prev_char: Option<char> = None;

        for (idx, ch) in text.char_indices() {
            match ch {
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote && prev_char != Some('\\') => {
                    in_double_quote = !in_double_quote;
                }
                '#' if !in_single_quote
                    && !in_double_quote
                    && (idx == 0 || prev_char.is_some_and(|c| c.is_whitespace())) =>
                {
                    return &text[..idx];
                }
                _ => {}
            }
            prev_char = Some(ch);
        }

        text
    }

    /// Returns a reference to the complete source text.
    #[inline]
    pub const fn input(&self) -> &'a str {
        self.input
    }

    /// Returns the total number of non-empty lines scanned.
    #[inline]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns `true` if no non-empty lines were found.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Peeks at the current line without advancing the cursor.
    #[inline]
    pub fn peek(&self) -> Option<&Line<'a>> {
        self.lines.get(self.cursor)
    }

    /// Advances the cursor and returns the next line.
    #[inline]
    pub fn next_line(&mut self) -> Option<Line<'a>> {
        if self.cursor < self.lines.len() {
            let line = self.lines[self.cursor];
            self.cursor += 1;
            Some(line)
        } else {
            None
        }
    }

    /// Peeks at the indentation level of the next line, or `None` if EOF.
    #[inline]
    pub fn peek_indent(&self) -> Option<usize> {
        self.peek().map(|l| l.indent)
    }

    /// Returns `true` if the cursor has consumed all lines.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.cursor >= self.lines.len()
    }

    /// Returns the current source position, or end-of-input position if at EOF.
    pub fn current_position(&self) -> Position {
        self.peek()
            .map(|l| l.start_position())
            .unwrap_or_else(|| Position::new(self.lines.len().max(1), 1, self.input.len()))
    }
}

// ==============================================================================
// Unit Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_line_extraction_and_indentation() {
        let yaml = "
# Comment line
key: value
  nested:
    - item 1
    - item 2
";
        let mut scanner = Scanner::new(yaml).expect("scanner should parse lines");
        assert_eq!(scanner.len(), 4);

        let l1 = scanner.next_line().expect("line 1");
        assert_eq!(l1.content, "key: value");
        assert_eq!(l1.indent, 0);
        assert_eq!(l1.line_number, 3);

        let l2 = scanner.next_line().expect("line 2");
        assert_eq!(l2.content, "nested:");
        assert_eq!(l2.indent, 2);

        let l3 = scanner.next_line().expect("line 3");
        assert_eq!(l3.content, "- item 1");
        assert_eq!(l3.indent, 4);

        let l4 = scanner.next_line().expect("line 4");
        assert_eq!(l4.content, "- item 2");
        assert_eq!(l4.indent, 4);

        assert!(scanner.is_eof());
    }

    #[test]
    fn test_tab_in_indentation_fails() {
        let yaml = "key:\n\tvalue: 123";
        let err = Scanner::new(yaml).expect_err("tab in indentation should fail");
        assert_eq!(err.kind, ErrorKind::TabInIndentation);
        assert_eq!(err.position.line, 2);
    }

    #[test]
    fn test_comment_stripping_preserves_quoted_strings() {
        let yaml = "url: \"http://example.com/#anchor\" # inline comment\ntext: 'a # b'";
        let mut scanner = Scanner::new(yaml).expect("should scan lines");

        let l1 = scanner.next_line().expect("line 1");
        assert_eq!(l1.content, "url: \"http://example.com/#anchor\"");

        let l2 = scanner.next_line().expect("line 2");
        assert_eq!(l2.content, "text: 'a # b'");
    }

    #[test]
    fn test_windows_crlf_newlines() {
        let yaml = "a: 1\r\nb: 2\r\n";
        let mut scanner = Scanner::new(yaml).expect("should handle CRLF");
        assert_eq!(scanner.len(), 2);
        assert_eq!(scanner.next_line().expect("line 1").content, "a: 1");
        assert_eq!(scanner.next_line().expect("line 2").content, "b: 2");
    }
}
