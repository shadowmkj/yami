//! Block-style YAML mapping parser for key-value structures.
//!
//! Handles indentation-based mapping hierarchies, scalar keys and values,
//! nested mappings and sequences, flow values inside mappings, and compact entries.

use crate::ast::{Entry, Yaml};
use crate::error::{ErrorKind, Position, Result, YamlError};
use crate::parser::flow::{parse_flow_value, FlowCursor};
use crate::parser::parse_node;
use crate::parser::sequence::{parse_scalar_or_quoted, split_mapping_key_val};
use crate::scanner::{Line, Scanner};

/// Parses a block mapping at the specified indentation level.
pub fn parse_block_mapping<'a>(scanner: &mut Scanner<'a>, expected_indent: usize) -> Result<Yaml<'a>> {
    let mut entries = Vec::new();

    while let Some(line) = scanner.peek() {
        if line.is_document_separator() {
            break;
        }

        // Mapping terminates when indentation drops below the expected mapping indentation
        if line.indent < expected_indent {
            break;
        }

        // If at the same indentation level, it must be a mapping entry `key: value`
        if line.indent == expected_indent {
            if line.is_sequence_item() {
                // Not a mapping entry at this indentation
                break;
            }

            if let Some((key_part, val_part)) = split_mapping_key_val(line.content) {
                let line_info = scanner.next_line().expect("peeked line must exist");
                let entry = parse_mapping_entry(scanner, line_info, key_part, val_part, expected_indent)?;

                // Check for duplicate keys
                if let Yaml::Scalar(key_name) = entry.key
                    && entries.iter().any(|e: &Entry<'a>| match e.key {
                        Yaml::Scalar(k) => k == key_name,
                        _ => false,
                    })
                {
                    return Err(YamlError::new(
                        ErrorKind::DuplicateKey(key_name.to_string()),
                        line_info.start_position(),
                    ));
                }

                entries.push(entry);
            } else {
                // Line does not contain a valid mapping key-value separator
                let pos = line.start_position();
                return Err(YamlError::new(ErrorKind::ExpectedColon, pos));
            }
        } else {
            // Indented deeper without a parent key is an indentation error
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

    Ok(Yaml::Mapping(entries))
}

/// Parses a block mapping that already begins with an existing first entry (e.g. from compact list syntax).
pub fn parse_block_mapping_with_first_entry<'a>(
    scanner: &mut Scanner<'a>,
    expected_indent: usize,
    first_entry: Entry<'a>,
) -> Result<Yaml<'a>> {
    let mut entries = vec![first_entry];

    while let Some(line) = scanner.peek() {
        if line.is_document_separator() {
            break;
        }

        let line_indent = line.indent;
        // Terminate when dedenting below expected indentation
        if line_indent < expected_indent {
            break;
        }

        if line.is_sequence_item() {
            break;
        }

        if let Some((key_part, val_part)) = split_mapping_key_val(line.content) {
            let line_info = scanner.next_line().expect("peeked line must exist");
            let entry = parse_mapping_entry(scanner, line_info, key_part, val_part, line_indent)?;

            if let Yaml::Scalar(key_name) = entry.key
                && entries.iter().any(|e: &Entry<'a>| match e.key {
                    Yaml::Scalar(k) => k == key_name,
                    _ => false,
                })
            {
                return Err(YamlError::new(
                    ErrorKind::DuplicateKey(key_name.to_string()),
                    line_info.start_position(),
                ));
            }

            entries.push(entry);
        } else {
            break;
        }
    }

    Ok(Yaml::Mapping(entries))
}

/// Parses a single key-value entry, resolving inline values, flow structures, or nested blocks.
fn parse_mapping_entry<'a>(
    scanner: &mut Scanner<'a>,
    line_info: Line<'a>,
    key_part: &'a str,
    val_part: &'a str,
    map_indent: usize,
) -> Result<Entry<'a>> {
    let key_node = parse_scalar_or_quoted(key_part);
    let val_trimmed = val_part.trim();

    let value_node = if val_trimmed.is_empty() {
        // Value is defined on following lines indented deeper
        if let Some(next_line) = scanner.peek()
            && !next_line.is_document_separator()
            && next_line.indent > map_indent
        {
            let next_indent = next_line.indent;
            parse_node(scanner, next_indent)?
        } else {
            Yaml::Scalar("")
        }
    } else if val_trimmed.starts_with('[') || val_trimmed.starts_with('{') {
        // Inline flow structure as mapping value
        let mut cursor = FlowCursor::new(
            val_trimmed,
            Position::new(
                line_info.line_number,
                line_info.start_column + key_part.len() + 2,
                line_info.byte_offset + key_part.len() + 2,
            ),
        );
        parse_flow_value(&mut cursor)?
    } else {
        // Simple scalar value
        parse_scalar_or_quoted(val_trimmed)
    };

    Ok(Entry::new(key_node, value_node))
}
