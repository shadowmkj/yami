//! Block-style YAML sequence parser for list items starting with `-`.
//!
//! Handles indentation-based list structures, nested sequences, flow elements inside
//! lists, and compact mapping entries (`- key: value`).

use crate::ast::{Entry, Yaml};
use crate::error::{ErrorKind, Position, Result, YamlError};
use crate::parser::flow::{parse_flow_value, FlowCursor};
use crate::parser::mapping::parse_block_mapping_with_first_entry;
use crate::parser::parse_node;
use crate::scanner::{Line, Scanner};

/// Parses a block sequence at the specified indentation level.
pub fn parse_block_sequence<'a>(scanner: &mut Scanner<'a>, expected_indent: usize) -> Result<Yaml<'a>> {
    let mut items = Vec::new();

    while let Some(line) = scanner.peek() {
        if line.is_document_separator() {
            break;
        }

        // Sequences terminate when indentation drops below the expected sequence indentation
        if line.indent < expected_indent {
            break;
        }

        // If at the same indentation level, it must be another sequence item `- `
        if line.indent == expected_indent {
            if !line.is_sequence_item() {
                break;
            }

            let line = scanner.next_line().expect("peeked line must exist");
            let item = parse_sequence_item(scanner, line, expected_indent)?;
            items.push(item);
        } else {
            // Indented deeper without a sequence marker is either invalid or part of previous item
            let pos = line.start_position();
            return Err(YamlError::new(
                ErrorKind::InvalidIndentation {
                    expected: expected_indent,
                    found: line.indent,
                },
                pos,
            ));
        }
    }

    Ok(Yaml::Sequence(items))
}

/// Parses the contents of a single sequence item following `-`.
fn parse_sequence_item<'a>(
    scanner: &mut Scanner<'a>,
    line: Line<'a>,
    seq_indent: usize,
) -> Result<Yaml<'a>> {
    // Strip leading `-` and following spaces
    let after_dash = if line.content == "-" {
        ""
    } else if let Some(rest) = line.content.strip_prefix("- ") {
        rest.trim_start()
    } else if let Some(rest) = line.content.strip_prefix("-\t") {
        rest.trim_start()
    } else {
        &line.content[1..]
    };

    if after_dash.is_empty() {
        // Bare dash `-` followed by newline.
        // Check if there is a nested block indented deeper.
        if let Some(next_line) = scanner.peek()
            && !next_line.is_document_separator()
            && next_line.indent > seq_indent
        {
            let next_indent = next_line.indent;
            return parse_node(scanner, next_indent);
        }
        return Ok(Yaml::Scalar(""));
    }

    // Check if the item starts with a flow sequence or mapping
    if after_dash.starts_with('[') || after_dash.starts_with('{') {
        let mut cursor = FlowCursor::new(
            after_dash,
            Position::new(
                line.line_number,
                line.start_column + 2,
                line.byte_offset + 2,
            ),
        );
        let val = parse_flow_value(&mut cursor)?;
        return Ok(val);
    }

    // Check for compact mapping syntax on a sequence item: `- key: value`
    if let Some((key_part, val_part)) = split_mapping_key_val(after_dash) {
        let key_node = parse_scalar_or_quoted(key_part);
        let val_trimmed = val_part.trim();

        let value_node = if val_trimmed.is_empty() {
            // Value is on the next line indented deeper
            if let Some(next_line) = scanner.peek()
                && !next_line.is_document_separator()
                && next_line.indent > seq_indent
            {
                let next_indent = next_line.indent;
                parse_node(scanner, next_indent)?
            } else {
                Yaml::Scalar("")
            }
        } else if val_trimmed.starts_with('[') || val_trimmed.starts_with('{') {
            let mut cursor = FlowCursor::new(
                val_trimmed,
                Position::new(
                    line.line_number,
                    line.start_column + 2 + key_part.len() + 1,
                    line.byte_offset + 2 + key_part.len() + 1,
                ),
            );
            parse_flow_value(&mut cursor)?
        } else {
            parse_scalar_or_quoted(val_trimmed)
        };

        let first_entry = Entry::new(key_node, value_node);
        // Additional mapping keys for this compact entry will be indented at > seq_indent
        let compact_map_indent = seq_indent + 2;
        return parse_block_mapping_with_first_entry(scanner, compact_map_indent, first_entry);
    }

    // Plain or quoted scalar sequence item
    Ok(parse_scalar_or_quoted(after_dash))
}

/// Helper function to split a string into `key` and `value` if it contains `: ` or ends with `:`.
pub fn split_mapping_key_val(text: &str) -> Option<(&str, &str)> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = None;

    for (idx, ch) in text.char_indices() {
        match ch {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote && prev_char != Some('\\') => {
                in_double_quote = !in_double_quote;
            }
            ':' if !in_single_quote && !in_double_quote => {
                let rest = &text[idx + 1..];
                if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                    let key = &text[..idx];
                    let val = if rest.is_empty() { "" } else { &rest[1..] };
                    return Some((key, val));
                }
            }
            _ => {}
        }
        prev_char = Some(ch);
    }

    None
}

/// Parses plain or quoted scalar slice into `Yaml::Scalar`.
pub fn parse_scalar_or_quoted(text: &str) -> Yaml<'_> {
    let trimmed = text.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
        || (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
    {
        Yaml::Scalar(&trimmed[1..trimmed.len() - 1])
    } else {
        Yaml::Scalar(trimmed)
    }
}
