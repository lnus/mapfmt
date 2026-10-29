//! Reindent MapServer Mapfiles.
//!
//! Only leading whitespace changes (plus trailing whitespace, which is
//! stripped). The indent rules are the grammar's `indents.scm`: a block node
//! (`@indent`) indents the lines after its first line, and a line that starts
//! with `END` (`@outdent`) goes back one level.

use std::fmt;

use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The file doesn't parse; `row` and `column` are 0-based.
    Parse { row: usize, column: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { row, column } => {
                write!(f, "{}:{}: parse error", row + 1, column + 1)
            }
        }
    }
}

impl std::error::Error for Error {}

pub struct Formatted {
    pub output: Vec<u8>,
    /// The first parse error, if any. The output is still usable:
    /// tree-sitter recovers around errors, and only whitespace changes, but
    /// the indentation near the error may be off.
    pub error: Option<Error>,
}

/// Reindents `source` with `indent` spaces per level.
///
/// Works on bytes, so files in any ASCII-compatible encoding (Latin-1 is
/// common in mapfiles) come out unchanged apart from whitespace.
pub fn format(source: &[u8], indent: usize) -> Formatted {
    let language = tree_sitter_mapfile::LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("Error loading Mapfile parser");
    let tree = parser.parse(source, None).expect("parser has a language");

    let error = first_error(tree.root_node()).map(|node| {
        let pos = node.start_position();
        Error::Parse {
            row: pos.row,
            column: pos.column,
        }
    });

    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    let depths = depths(&tree, source, &language, lines.len());
    let verbatim = verbatim_rows(&tree, lines.len());

    let mut out = Vec::with_capacity(source.len());
    for (row, line) in lines.iter().enumerate() {
        if row > 0 {
            out.push(b'\n');
        }
        match verbatim[row] {
            Verbatim::All => out.extend_from_slice(line),
            keep => {
                let (line, cr): (&[u8], &[u8]) = match line.strip_suffix(b"\r") {
                    Some(line) => (line, b"\r"),
                    None => (line, b""),
                };
                let mut text = trim_start(line);
                if keep != Verbatim::Trailing {
                    text = trim_end(text);
                }
                if !text.is_empty() {
                    out.extend(std::iter::repeat_n(
                        b' ',
                        depths[row].max(0) as usize * indent,
                    ));
                }
                out.extend_from_slice(text);
                out.extend_from_slice(cr);
            }
        }
    }
    Formatted { output: out, error }
}

/// The first `ERROR` or `MISSING` node, in document order.
fn first_error(node: Node) -> Option<Node> {
    if !node.has_error() {
        return None;
    }
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).find_map(first_error);
    // `has_error` with no erroring child: the node itself is the problem.
    found.or(Some(node))
}

/// The indent level of each row.
fn depths(tree: &Tree, source: &[u8], language: &tree_sitter::Language, rows: usize) -> Vec<i32> {
    let query = Query::new(language, tree_sitter_mapfile::INDENTS_QUERY)
        .expect("Error compiling indents query");
    let names = query.capture_names();

    // Difference array: +1 at the row after a block starts, -1 after it ends.
    let mut delta = vec![0i32; rows + 1];
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), source);
    while let Some((m, i)) = captures.next() {
        let capture = m.captures[*i];
        let node = capture.node;
        let (start, end) = (node.start_position(), node.end_position());
        match names[capture.index as usize] {
            "indent" if end.row > start.row => {
                delta[start.row + 1] += 1;
                delta[end.row + 1] -= 1;
            }
            "outdent" if starts_line(source, node) => {
                delta[start.row] -= 1;
                delta[start.row + 1] += 1;
            }
            _ => {}
        }
    }

    let mut depth = 0;
    delta[..rows]
        .iter()
        .map(|d| {
            depth += d;
            depth
        })
        .collect()
}

/// Whether only whitespace comes before `node` on its line.
fn starts_line(source: &[u8], node: Node) -> bool {
    let before = &source[..node.start_byte()];
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    trim_start(&before[line_start..]).is_empty()
}

fn trim_start(line: &[u8]) -> &[u8] {
    let n = line
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    &line[n..]
}

fn trim_end(line: &[u8]) -> &[u8] {
    let n = line
        .iter()
        .rev()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    &line[..line.len() - n]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verbatim {
    No,
    /// The row ends inside a multi-line token: reindent it, but don't touch
    /// its trailing whitespace.
    Trailing,
    /// The row is (or starts) inside a multi-line token: leave it as is.
    All,
}

/// Rows covered by multi-line tokens (strings, block comments, ...).
fn verbatim_rows(tree: &Tree, rows: usize) -> Vec<Verbatim> {
    let mut verbatim = vec![Verbatim::No; rows];
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let (start, end) = (node.start_position().row, node.end_position().row);
        if node.child_count() == 0 && end > start {
            if verbatim[start] == Verbatim::No {
                verbatim[start] = Verbatim::Trailing;
            }
            for row in &mut verbatim[start + 1..=end] {
                *row = Verbatim::All;
            }
        }
        if cursor.goto_first_child() || cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return verbatim;
            }
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(source: &str) -> String {
        let formatted = format(source.as_bytes(), 4);
        assert_eq!(formatted.error, None);
        String::from_utf8(formatted.output).unwrap()
    }

    #[test]
    fn reindents_nested_blocks() {
        let input = "MAP\nNAME \"x\"\n  LAYER\n        TYPE POLYGON\n\tCLASS\nSTYLE\nCOLOR 1 2 3\nEND\n      END\n END\nEND\n";
        let expected = "MAP\n    NAME \"x\"\n    LAYER\n        TYPE POLYGON\n        CLASS\n            STYLE\n                COLOR 1 2 3\n            END\n        END\n    END\nEND\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn is_idempotent() {
        let input = "MAP\n    LAYER\n        NAME \"a\"\n    END\nEND\n";
        assert_eq!(fmt(input), input);
    }

    #[test]
    fn indent_width() {
        assert_eq!(
            format(b"MAP\nNAME \"x\"\nEND", 2).output,
            b"MAP\n  NAME \"x\"\nEND"
        );
    }

    #[test]
    fn keeps_one_line_blocks() {
        let input = "MAP\nPROJECTION \"init=epsg:4326\" END\nEND\n";
        let expected = "MAP\n    PROJECTION \"init=epsg:4326\" END\nEND\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn strips_trailing_whitespace_and_blank_lines() {
        assert_eq!(
            fmt("MAP  \n   \nNAME \"x\"\t\nEND\n"),
            "MAP\n\n    NAME \"x\"\nEND\n"
        );
    }

    #[test]
    fn keeps_crlf() {
        assert_eq!(
            fmt("MAP\r\nNAME \"x\"\r\nEND\r\n"),
            "MAP\r\n    NAME \"x\"\r\nEND\r\n"
        );
    }

    #[test]
    fn leaves_multiline_tokens_alone() {
        let input = "MAP\n/* a\n   b  \n*/\nNAME \"x\"\nEND\n";
        let expected = "MAP\n    /* a\n   b  \n*/\n    NAME \"x\"\nEND\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn reports_parse_errors() {
        assert_eq!(
            format(b"MAP\nLAYER\nEND\n", 4).error,
            Some(Error::Parse { row: 2, column: 3 })
        );
    }

    #[test]
    fn formats_around_parse_errors() {
        // `$` isn't valid in an unquoted value; the error stays on that line.
        let formatted = format(
            b"MAP\nLAYER\nDATA $setlater\nCLASS\nNAME \"a\"\nEND\nEND\nEND\n",
            4,
        );
        assert_eq!(formatted.error, Some(Error::Parse { row: 2, column: 5 }));
        assert_eq!(
            String::from_utf8(formatted.output).unwrap(),
            "MAP\n    LAYER\n        DATA $setlater\n        CLASS\n            NAME \"a\"\n        END\n    END\nEND\n"
        );
    }

    #[test]
    fn keeps_non_utf8_bytes() {
        // "Tromsø" in Latin-1.
        let input = b"MAP\nNAME \"Troms\xf8\"\nEND\n";
        assert_eq!(
            format(input, 4).output,
            b"MAP\n    NAME \"Troms\xf8\"\nEND\n"
        );
    }

    #[test]
    fn formats_fragments() {
        assert_eq!(
            fmt("CLASS\nNAME \"a\"\nEND\nCLASS\nNAME \"b\"\nEND\n"),
            "CLASS\n    NAME \"a\"\nEND\nCLASS\n    NAME \"b\"\nEND\n"
        );
    }
}
