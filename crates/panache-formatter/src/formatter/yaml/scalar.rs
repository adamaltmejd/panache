//! Scalar rendering: plain / single-quoted / double-quoted / block
//! literal (`|`) / block folded (`>`).
//!
//! Phase 1.1 stub: empty. Rule 3 (quote-style preference: plain →
//! double → single only on backslash-escape need), rule 4 (preserve
//! block scalar style), and the wrap interaction (only plain scalars
//! wrap; quoted/block styles never wrap) land in 1.2+.
