//! Top-level YAML document orchestration.
//!
//! Walks `YAML_STREAM` → `YAML_DOCUMENT` → body containers and
//! dispatches to per-container renderers
//! ([`block_map`](super::block_map),
//! [`block_sequence`](super::block_sequence),
//! [`flow`](super::flow), [`scalar`](super::scalar)).
//!
//! Phase 1.1 stub: walks the CST and emits every token's source byte
//! verbatim. This is byte-lossless but applies none of the 13 style
//! rules — it exists to prove the dispatch surface compiles and is
//! reachable from the cross-validation harness landing in 1.3.

use panache_parser::SyntaxNode;
use rowan::WalkEvent;

use super::options::YamlFormatOptions;

/// Render the given CST root into a string. The root is expected to be
/// the `DOCUMENT` node returned by
/// [`panache_parser::parser::yaml::parse_yaml_tree`], but any CST node
/// works for the byte-lossless stub — we just walk its tokens.
pub(super) fn render(root: &SyntaxNode, _opts: &YamlFormatOptions) -> String {
    let mut out = String::with_capacity(root.text_range().len().into());
    for event in root.preorder_with_tokens() {
        if let WalkEvent::Enter(rowan::NodeOrToken::Token(t)) = event {
            out.push_str(t.text());
        }
    }
    out
}
