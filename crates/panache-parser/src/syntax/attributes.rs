use crate::parser::utils::attributes::{parse_html_attribute_list, try_parse_trailing_attributes};
use crate::syntax::{AstNode, PanacheLanguage, SyntaxKind, SyntaxNode};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttributeNode(SyntaxNode);

impl AstNode for AttributeNode {
    type Language = PanacheLanguage;

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::ATTRIBUTE | SyntaxKind::DIV_INFO | SyntaxKind::HTML_ATTRS
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        Self::can_cast(node.kind()).then(|| AttributeNode(node))
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl AttributeNode {
    pub fn id(&self) -> Option<String> {
        let text = self.0.text().to_string();
        match self.0.kind() {
            SyntaxKind::HTML_ATTRS => parse_html_attribute_list(&text)
                .and_then(|attrs| attrs.identifier)
                .filter(|id| !id.is_empty()),
            _ => try_parse_trailing_attributes(&text)
                .and_then(|(attrs, _)| attrs.identifier)
                .filter(|id| !id.is_empty()),
        }
    }

    pub fn id_value_range(&self) -> Option<rowan::TextRange> {
        let id = self.id()?;
        let text = self.0.text().to_string();
        let node_start: usize = self.0.text_range().start().into();
        match self.0.kind() {
            SyntaxKind::HTML_ATTRS => {
                // Match `id=` followed by an optional quote and the id value.
                // The salsa indexer uses this range for highlights / renames;
                // a precise inner-value range is preferred over the full attr
                // node range.
                let marker = text.find("id")?;
                let after_id = &text[marker + 2..];
                let eq_off = after_id.bytes().position(|b| b == b'=')?;
                let after_eq = &after_id[eq_off + 1..];
                let (val_offset_in_after_eq, val_len) = match after_eq.bytes().next() {
                    Some(b'"') | Some(b'\'') => (1, id.len()),
                    _ => (0, id.len()),
                };
                let value_start_in_text = marker + 2 + eq_off + 1 + val_offset_in_after_eq;
                let start = rowan::TextSize::from((node_start + value_start_in_text) as u32);
                let end =
                    rowan::TextSize::from((node_start + value_start_in_text + val_len) as u32);
                Some(rowan::TextRange::new(start, end))
            }
            _ => {
                let marker = text.find(&format!("#{}", id))?;
                let start = rowan::TextSize::from((node_start + marker + 1) as u32);
                let end = rowan::TextSize::from((node_start + marker + 1 + id.len()) as u32);
                Some(rowan::TextRange::new(start, end))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_node_extracts_div_info_id_and_range() {
        let config = crate::ParserOptions {
            flavor: crate::options::Flavor::RMarkdown,
            ..Default::default()
        };
        let tree = crate::parse("::: {#mu .exercise}\ntext\n:::\n", Some(config));
        let node = tree
            .descendants()
            .find_map(AttributeNode::cast)
            .expect("attribute node");
        assert_eq!(node.id().as_deref(), Some("mu"));

        let range = node.id_value_range().expect("id range");
        let start: usize = range.start().into();
        let end: usize = range.end().into();
        assert_eq!(&tree.text().to_string()[start..end], "mu");
    }
}
