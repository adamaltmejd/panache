//! Block mapping rendering (`YAML_BLOCK_MAP` and its
//! `YAML_BLOCK_MAP_ENTRY`/`YAML_BLOCK_MAP_KEY`/`YAML_BLOCK_MAP_VALUE`
//! children).
//!
//! Phase 1.1 stub: empty. [`super::document::render`] currently emits
//! tokens verbatim and bypasses per-container rendering. The
//! dispatcher will route here once style rules 1–13 are implemented
//! in Phase 1.2+ (indent canonicalization, sequence-item indent,
//! blank-line collapsing, comment-spacing).
