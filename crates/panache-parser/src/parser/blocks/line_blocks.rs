use crate::options::ParserOptions;
use crate::syntax::SyntaxKind;
use rowan::GreenNodeBuilder;

use super::blockquotes::strip_n_blockquote_markers;
use super::code_blocks::{emit_content_line_prefixes, strip_list_indent};
use super::container_prefix::advance_columns;
use crate::parser::utils::container_stack::byte_index_at_column;
use crate::parser::utils::helpers::strip_newline;
use crate::parser::utils::inline_emission;

/// Try to parse the start of a line block.
/// Returns Some(()) if this line starts a line block (| followed by space or end of line).
pub fn try_parse_line_block_start(line: &str) -> Option<()> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("| ") || trimmed == "|" {
        Some(())
    } else {
        None
    }
}

/// Parse a complete line block starting at current position.
/// Returns the new position after the line block.
///
/// Container-prefix parameters mirror the fenced-code threading pattern
/// (`parse_fenced_code_block` in `code_blocks.rs`). On the dispatch line
/// (`pos == start_pos`), `list_marker_consumed_on_line_0` selects between a
/// silent column-advance through the upstream-emitted list marker
/// (`advance_columns`) and a whitespace-only strip with WHITESPACE emission
/// (`strip_list_indent`). On subsequent lines, the list-content-indent is
/// always whitespace and is stripped via `strip_list_indent` so that blank
/// lines aren't eaten by the column-advance.
#[allow(clippy::too_many_arguments)]
pub fn parse_line_block(
    lines: &[&str],
    start_pos: usize,
    builder: &mut GreenNodeBuilder<'static>,
    config: &ParserOptions,
    bq_depth: usize,
    list_content_col: usize,
    list_marker_consumed_on_line_0: bool,
    bq_outer: bool,
    content_indent: usize,
) -> usize {
    log::trace!("Parsing line block at line {}", start_pos + 1);

    builder.start_node(SyntaxKind::LINE_BLOCK.into());

    let mut pos = start_pos;
    let mut first_line = true;

    while pos < lines.len() {
        let raw_line = lines[pos];

        let kind = if first_line {
            // Detection in `LineBlockParser::detect_prepared` already confirmed
            // line 0 is a marker line; commit without a peek.
            LineKind::Marker
        } else {
            let peek = silent_strip_container_prefix(
                raw_line,
                bq_depth,
                list_content_col,
                bq_outer,
                content_indent,
            );
            if parse_line_block_line_marker(peek).is_some() {
                LineKind::Marker
            } else if peek.starts_with(' ') && !peek.trim_start().starts_with("| ") {
                LineKind::Continuation
            } else {
                break;
            }
        };

        builder.start_node(SyntaxKind::LINE_BLOCK_LINE.into());

        // Emit container-prefix tokens inside LINE_BLOCK_LINE so each
        // line's byte range stays self-contained (matches the top-level
        // line_blocks snapshot convention where LINE_BLOCK_LINE covers a
        // whole source line).
        let stripped = if first_line {
            emit_open_line_prefixes(
                builder,
                raw_line,
                bq_depth,
                list_content_col,
                list_marker_consumed_on_line_0,
                bq_outer,
                content_indent,
            )
        } else {
            emit_content_line_prefixes(
                builder,
                raw_line,
                bq_depth,
                list_content_col,
                bq_outer,
                content_indent,
            )
        };

        match kind {
            LineKind::Marker => {
                let content_start = parse_line_block_line_marker(stripped)
                    .expect("marker presence verified upstream");
                builder.token(
                    SyntaxKind::LINE_BLOCK_MARKER.into(),
                    &stripped[..content_start],
                );
                let content = &stripped[content_start..];
                let (content_without_newline, newline_str) = strip_newline(content);
                if !content_without_newline.is_empty() {
                    inline_emission::emit_inlines(builder, content_without_newline, config, false);
                }
                if !newline_str.is_empty() {
                    builder.token(SyntaxKind::NEWLINE.into(), newline_str);
                }
            }
            LineKind::Continuation => {
                let (line_without_newline, newline_str) = strip_newline(stripped);
                if !line_without_newline.is_empty() {
                    inline_emission::emit_inlines(builder, line_without_newline, config, false);
                }
                if !newline_str.is_empty() {
                    builder.token(SyntaxKind::NEWLINE.into(), newline_str);
                }
            }
        }

        builder.finish_node(); // LineBlockLine
        pos += 1;
        first_line = false;
    }

    builder.finish_node(); // LineBlock

    log::trace!("Parsed line block: lines {}-{}", start_pos + 1, pos);

    pos
}

enum LineKind {
    Marker,
    Continuation,
}

/// Silent peek of the container-prefix strip for continuation/next-marker
/// detection on lines 1..N. Mirrors the order-of-strip in
/// `emit_content_line_prefixes` (`code_blocks.rs`) but writes no tokens.
fn silent_strip_container_prefix<'a>(
    line: &'a str,
    bq_depth: usize,
    list_content_col: usize,
    bq_outer: bool,
    content_indent: usize,
) -> &'a str {
    let mut s = line;
    let strip_bq = |s: &mut &'a str| {
        if bq_depth > 0 {
            *s = strip_n_blockquote_markers(s, bq_depth);
        }
    };
    let strip_list = |s: &mut &'a str| {
        if list_content_col > 0 {
            *s = strip_list_indent(s, list_content_col);
        }
    };
    if bq_outer {
        strip_bq(&mut s);
        strip_list(&mut s);
    } else {
        strip_list(&mut s);
        strip_bq(&mut s);
    }
    if content_indent > 0 {
        let indent_bytes = byte_index_at_column(s, content_indent);
        if s.len() >= indent_bytes {
            s = &s[indent_bytes..];
        }
    }
    s
}

/// Strip and emit the active container prefix on the dispatch line (line 0).
/// Mirrors `prepare_fence_open_line` in `code_blocks.rs` minus the final
/// `strip_leading_spaces` step — line blocks treat any leading spaces
/// before `|` as part of `LINE_BLOCK_MARKER`, so we must not strip them.
fn emit_open_line_prefixes<'a>(
    builder: &mut GreenNodeBuilder<'static>,
    source_line: &'a str,
    bq_depth: usize,
    list_content_col: usize,
    list_marker_consumed_on_line_0: bool,
    bq_outer: bool,
    content_indent: usize,
) -> &'a str {
    let mut s: &'a str = source_line;
    let mut pending_ws_start: Option<usize> = None;
    let suppress_list = list_marker_consumed_on_line_0;

    let flush_ws = |builder: &mut GreenNodeBuilder<'static>,
                    pending: &mut Option<usize>,
                    current_offset: usize| {
        if let Some(start) = *pending
            && current_offset > start
        {
            builder.token(
                SyntaxKind::WHITESPACE.into(),
                &source_line[start..current_offset],
            );
        }
        *pending = None;
    };

    let do_strip_list = |s: &mut &'a str, pending: &mut Option<usize>| {
        if list_content_col == 0 {
            return;
        }
        let stripped = if suppress_list {
            advance_columns(s, list_content_col)
        } else {
            strip_list_indent(s, list_content_col)
        };
        let consumed = s.len() - stripped.len();
        if consumed > 0 {
            let start = source_line.len() - s.len();
            if !suppress_list && pending.is_none() {
                *pending = Some(start);
            }
            *s = stripped;
        }
    };

    let do_strip_bq =
        |builder: &mut GreenNodeBuilder<'static>, s: &mut &'a str, pending: &mut Option<usize>| {
            if bq_depth == 0 {
                return;
            }
            let current_offset = source_line.len() - s.len();
            flush_ws(builder, pending, current_offset);
            *s = strip_n_blockquote_markers(s, bq_depth);
        };

    if bq_outer {
        do_strip_bq(builder, &mut s, &mut pending_ws_start);
        do_strip_list(&mut s, &mut pending_ws_start);
    } else {
        do_strip_list(&mut s, &mut pending_ws_start);
        do_strip_bq(builder, &mut s, &mut pending_ws_start);
    }

    if content_indent > 0 {
        let indent_bytes = byte_index_at_column(s, content_indent);
        if s.len() >= indent_bytes && indent_bytes > 0 {
            let start = source_line.len() - s.len();
            if pending_ws_start.is_none() {
                pending_ws_start = Some(start);
            }
            s = &s[indent_bytes..];
        }
    }

    let final_offset = source_line.len() - s.len();
    flush_ws(builder, &mut pending_ws_start, final_offset);
    s
}

/// Parse a line block marker and return the index where content starts.
/// Returns Some(index) if the line starts with "| " or just "|", None otherwise.
fn parse_line_block_line_marker(line: &str) -> Option<usize> {
    // Line block lines start with | followed by a space or end of line
    // We need to handle leading whitespace (indentation)
    let trimmed_start = line.len() - line.trim_start().len();
    let after_indent = &line[trimmed_start..];

    if after_indent.starts_with("| ") {
        Some(trimmed_start + 2) // Skip "| "
    } else if after_indent == "|" || after_indent == "|\n" {
        Some(trimmed_start + 1) // Just "|", no space
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_line_block_start() {
        assert!(try_parse_line_block_start("| Some text").is_some());
        assert!(try_parse_line_block_start("| ").is_some());
        assert!(try_parse_line_block_start("|").is_some()); // Empty line block
        assert!(try_parse_line_block_start("  | Some text").is_some());

        // Not line blocks
        assert!(try_parse_line_block_start("|No space").is_none());
        assert!(try_parse_line_block_start("Regular text").is_none());
        assert!(try_parse_line_block_start("").is_none());
    }

    #[test]
    fn test_parse_line_block_marker() {
        assert_eq!(parse_line_block_line_marker("| Some text"), Some(2));
        assert_eq!(parse_line_block_line_marker("| "), Some(2));
        assert_eq!(parse_line_block_line_marker("|"), Some(1)); // Empty line block
        assert_eq!(parse_line_block_line_marker("  | Indented"), Some(4));

        // Not valid
        assert_eq!(parse_line_block_line_marker("|No space"), None);
        assert_eq!(parse_line_block_line_marker("Regular"), None);
    }

    #[test]
    fn test_simple_line_block() {
        let input = vec!["| Line one", "| Line two", "| Line three"];

        let mut builder = GreenNodeBuilder::new();
        let new_pos = parse_line_block(
            &input,
            0,
            &mut builder,
            &ParserOptions::default(),
            0,
            0,
            false,
            false,
            0,
        );

        assert_eq!(new_pos, 3);
    }

    #[test]
    fn test_line_block_with_continuation() {
        let input = vec![
            "| This is a long line",
            "  that continues here",
            "| Second line",
        ];

        let mut builder = GreenNodeBuilder::new();
        let new_pos = parse_line_block(
            &input,
            0,
            &mut builder,
            &ParserOptions::default(),
            0,
            0,
            false,
            false,
            0,
        );

        assert_eq!(new_pos, 3);
    }

    #[test]
    fn test_line_block_with_indentation() {
        let input = vec!["| First line", "|    Indented line", "| Back to normal"];

        let mut builder = GreenNodeBuilder::new();
        let new_pos = parse_line_block(
            &input,
            0,
            &mut builder,
            &ParserOptions::default(),
            0,
            0,
            false,
            false,
            0,
        );

        assert_eq!(new_pos, 3);
    }

    #[test]
    fn test_line_block_stops_at_non_line_block() {
        let input = vec!["| Line one", "| Line two", "Regular paragraph"];

        let mut builder = GreenNodeBuilder::new();
        let new_pos = parse_line_block(
            &input,
            0,
            &mut builder,
            &ParserOptions::default(),
            0,
            0,
            false,
            false,
            0,
        );

        assert_eq!(new_pos, 2); // Should stop before "Regular paragraph"
    }
}
