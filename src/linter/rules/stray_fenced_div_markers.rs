use rowan::TextRange;

use crate::config::Config;
use crate::linter::diagnostics::{Diagnostic, DiagnosticNoteKind, Location};
use crate::linter::rules::Rule;
use crate::syntax::{SyntaxKind, SyntaxNode};

pub struct StrayFencedDivMarkersRule;

impl Rule for StrayFencedDivMarkersRule {
    fn name(&self) -> &str {
        "stray-fenced-div-markers"
    }

    fn check(
        &self,
        tree: &SyntaxNode,
        input: &str,
        _config: &Config,
        _metadata: Option<&crate::metadata::DocumentMetadata>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for node in tree.descendants() {
            if node.kind() != SyntaxKind::PARAGRAPH {
                continue;
            }
            for elem in node.descendants_with_tokens() {
                let Some(token) = elem.into_token() else {
                    continue;
                };
                if token.kind() != SyntaxKind::TEXT {
                    continue;
                }
                let text = token.text();
                let token_start: u32 = token.text_range().start().into();
                for (run_start, run_end) in find_colon_runs(text) {
                    let abs_start = token_start + run_start as u32;
                    let abs_end = token_start + run_end as u32;
                    let range = TextRange::new(abs_start.into(), abs_end.into());
                    let location = Location::from_range(range, input);
                    let marker = &text[run_start..run_end];
                    let diag = Diagnostic::warning(
                        location,
                        "stray-fenced-div-markers",
                        format!("'{marker}' appears as text, not as a fenced div marker"),
                    )
                    .with_note(
                        DiagnosticNoteKind::Help,
                        "Pandoc only treats ':::' as a fenced div marker when it starts a line \
                         on its own (optionally followed by a class or attributes). Add a \
                         newline before it, or wrap it in backticks if it's intentional text",
                    );
                    diagnostics.push(diag);
                }
            }
        }

        diagnostics
    }
}

/// Non-overlapping byte offsets of every run of three or more `:` characters
/// in `text`.
fn find_colon_runs(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut runs = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            let start = i;
            while i < bytes.len() && bytes[i] == b':' {
                i += 1;
            }
            if i - start >= 3 {
                runs.push((start, i));
            }
        } else {
            i += 1;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn parse_and_lint(input: &str) -> Vec<Diagnostic> {
        let config = Config::default();
        let tree = crate::parser::parse(input, Some(config.clone()));
        StrayFencedDivMarkersRule.check(&tree, input, &config, None)
    }

    #[test]
    fn balanced_div_is_clean() {
        let input = "::: warning\nbody\n:::\n";
        assert!(parse_and_lint(input).is_empty());
    }

    #[test]
    fn flags_lone_triple_colon() {
        let input = "Hello.\n\n:::\n\nGoodbye.\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "stray-fenced-div-markers");
        assert_eq!(diagnostics[0].location.line, 3);
        assert!(diagnostics[0].message.contains(":::"));
        assert!(diagnostics[0].fix.is_none());
    }

    #[test]
    fn flags_longer_runs() {
        let input = "para\n\n::::::\n\nmore\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("::::::"));
    }

    #[test]
    fn ignores_two_colons() {
        let input = "para\n\n::\n\nmore\n";
        assert!(parse_and_lint(input).is_empty());
    }

    #[test]
    fn flags_mid_line_triple_colon() {
        let input = "Use ::: to start a div.\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].location.line, 1);
    }

    #[test]
    fn ignores_inline_code_span() {
        let input = "Type `:::` to open a div.\n";
        assert!(parse_and_lint(input).is_empty());
    }

    #[test]
    fn ignores_indented_code_block() {
        // 4+ spaces => indented code block, not a paragraph.
        let input = "para\n\n    :::\n\nmore\n";
        assert!(parse_and_lint(input).is_empty());
    }

    #[test]
    fn flags_colons_glued_to_inline_span() {
        // Issue #333: ':::' glued to the end of a span on the same line — the
        // user almost certainly meant a closing fence on its own line.
        let input = "::: {lang=en-US}\n[contact Ms. N]{lang=en-US}:::\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].location.line, 2);
        assert!(diagnostics[0].message.contains(":::"));
    }

    #[test]
    fn flags_text_after_colons() {
        // `::: foo bar` is rejected as an opener and falls through to text;
        // it's still a strong "did the user mean a fence?" signal.
        let input = "para\n\n::: not a fence shape with words\n\nmore\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].location.line, 3);
    }

    #[test]
    fn flags_up_to_three_leading_spaces() {
        let input = "para\n\n   :::\n\nmore\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_multiple_strays_in_one_document() {
        let input = "p1\n\n:::\n\np2\n\n::::\n\np3\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].location.line, 3);
        assert_eq!(diagnostics[1].location.line, 7);
    }

    #[test]
    fn flags_multiple_runs_on_one_line() {
        let input = "start ::: middle ::::: end\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains(":::"));
        assert!(diagnostics[1].message.contains(":::::"));
    }

    #[test]
    fn flags_trailing_whitespace_after_colons() {
        let input = "p\n\n:::   \n\nmore\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_crlf_line_endings() {
        let input = "p\r\n\r\n:::\r\n\r\nmore\r\n";
        let diagnostics = parse_and_lint(input);
        assert_eq!(diagnostics.len(), 1);
    }
}
