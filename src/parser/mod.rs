//! Parser modules for YAML block and flow constructs.
//!
//! Provides recursive-descent parsing over scanned YAML lines and flow segments,
//! building an AST where all scalars are zero-copy slices borrowed from the source buffer.

pub mod flow;
pub mod mapping;
pub mod sequence;

use crate::ast::Yaml;
use crate::error::{ErrorKind, Result, YamlError};
use crate::parser::flow::{FlowCursor, parse_flow_value};
use crate::parser::mapping::parse_block_mapping;
use crate::parser::sequence::{
    parse_block_sequence, parse_scalar_or_quoted, split_mapping_key_val,
};
use crate::scanner::Scanner;

/// Parses any YAML node at the specified minimum indentation level.
pub fn parse_node<'a>(scanner: &mut Scanner<'a>, min_indent: usize) -> Result<Yaml<'a>> {
    let line = match scanner.peek() {
        Some(l) => *l,
        None => return Ok(Yaml::Scalar("")),
    };

    if line.is_document_separator() {
        return Ok(Yaml::Scalar(""));
    }

    if line.indent < min_indent {
        let pos = line.start_position();
        return Err(YamlError::new(
            ErrorKind::InvalidIndentation {
                expected: min_indent,
                found: line.indent,
            },
            pos,
        ));
    }

    let current_indent = line.indent;

    if line.is_sequence_item() {
        return parse_block_sequence(scanner, current_indent);
    }

    if line.content.starts_with('[') || line.content.starts_with('{') {
        let line_info = scanner.next_line().expect("line exists");
        let mut cursor = FlowCursor::new(line_info.content, line_info.start_position());
        let val = parse_flow_value(&mut cursor)?;
        return Ok(val);
    }

    if split_mapping_key_val(line.content).is_some() {
        return parse_block_mapping(scanner, current_indent);
    }

    let line_info = scanner.next_line().expect("line exists");
    Ok(parse_scalar_or_quoted(line_info.content))
}

/// Parses a YAML string into a zero-copy [`Yaml<'a>`] value.
///
/// # Examples
///
/// ```rust
/// use yami::{parse, Yaml};
///
/// let yaml_text = "
/// server:
///   host: localhost
///   port: 8080
/// ";
///
/// let doc = parse(yaml_text).expect("valid yaml should parse");
/// assert_eq!(doc["server"]["host"], Yaml::Scalar("localhost"));
/// assert_eq!(doc["server"]["port"].to_i64().unwrap(), 8080);
/// ```
pub fn parse<'a>(input: &'a str) -> Result<Yaml<'a>> {
    let mut scanner = Scanner::new(input)?;

    if scanner.is_empty() {
        return Ok(Yaml::Scalar(""));
    }

    // Skip optional document start marker `---`
    if let Some(first_line) = scanner.peek()
        && first_line.content == "---"
    {
        scanner.next_line();
    }

    if scanner.is_empty() {
        return Ok(Yaml::Scalar(""));
    }

    let root_node = parse_node(&mut scanner, 0)?;

    // Skip optional document end marker `...` or subsequent `---`
    if let Some(next_line) = scanner.peek()
        && next_line.is_document_separator()
    {
        scanner.next_line();
    }

    Ok(root_node)
}

// ==============================================================================
// Unit Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_mapping() {
        let input = "
name: yami
version: 1.0
fast: true
";
        let doc = parse(input).expect("should parse simple mapping");
        assert_eq!(doc.get("name"), Some(&Yaml::Scalar("yami")));
        assert_eq!(doc["version"], Yaml::Scalar("1.0"));
        assert!(doc["fast"].to_bool().unwrap());
    }

    #[test]
    fn test_parse_simple_sequence() {
        let input = "
- rust
- zero-copy
- fast
";
        let doc = parse(input).expect("should parse sequence");
        assert_eq!(
            doc,
            Yaml::Sequence(vec![
                Yaml::Scalar("rust"),
                Yaml::Scalar("zero-copy"),
                Yaml::Scalar("fast"),
            ])
        );
    }

    #[test]
    fn test_parse_nested_mapping_and_sequence() {
        let input = "
database:
  host: 127.0.0.1
  ports:
    - 5432
    - 5433
";
        let doc = parse(input).expect("should parse nested struct");
        assert_eq!(doc["database"]["host"], Yaml::Scalar("127.0.0.1"));
        assert_eq!(
            doc["database"]["ports"],
            Yaml::Sequence(vec![Yaml::Scalar("5432"), Yaml::Scalar("5433")])
        );
    }

    #[test]
    fn test_parse_sequence_of_mappings() {
        let input = "
users:
  - id: 1
    name: Alice
  - id: 2
    name: Bob
";
        let doc = parse(input).expect("should parse sequence of mappings");
        let users = doc["users"].as_sequence().expect("users sequence");
        assert_eq!(users.len(), 2);
        assert_eq!(users[0]["name"], Yaml::Scalar("Alice"));
        assert_eq!(users[1]["name"], Yaml::Scalar("Bob"));
    }

    #[test]
    fn test_parse_mixed_block_and_flow() {
        let input = "
config:
  tags: [fast, safe, zero-copy]
  options: { debug: false, level: 3 }
";
        let doc = parse(input).expect("should parse mixed structures");
        assert_eq!(
            doc["config"]["tags"],
            Yaml::Sequence(vec![
                Yaml::Scalar("fast"),
                Yaml::Scalar("safe"),
                Yaml::Scalar("zero-copy"),
            ])
        );
        assert!(!doc["config"]["options"]["debug"].to_bool().unwrap());
        assert_eq!(doc["config"]["options"]["level"].to_i64().unwrap(), 3);
    }

    #[test]
    fn test_parse_duplicate_key_error() {
        let input = "
key: first
key: duplicate
";
        let err = parse(input).expect_err("should reject duplicate key");
        assert!(matches!(err.kind, ErrorKind::DuplicateKey(_)));
    }

    #[test]
    fn test_parse_empty_document_and_comments() {
        let input = "# Only comments\n# Second comment\n";
        let doc = parse(input).expect("should parse empty comment document");
        assert_eq!(doc, Yaml::Scalar(""));
    }
}
