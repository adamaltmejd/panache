//! Proxy types and helpers used to generate the JSON Schema for
//! `panache.toml`. Kept separate so the runtime config types stay
//! lean — these types exist purely to describe the user-facing
//! TOML shape to `schemars`.
//!
//! Each entry has a runtime counterpart that uses `toml::Value`
//! (because the actual deserialization fans out across helpers in
//! `super::resolve_*`). The proxy mirrors the documented input
//! shape, not the materialized struct, so the published schema
//! describes what users write.

use std::collections::HashMap;

use schemars::{JsonSchema, Schema, SchemaGenerator};

use super::FormatterDefinition;

/// Schema entry for a single `[extensions]` key.
///
/// Either a boolean (`gfm-auto-identifiers = true`) or a nested
/// table keyed by flavor name (`[extensions.pandoc] fenced-divs = false`).
#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum ExtensionEntry {
    Bool(bool),
    PerFlavor(HashMap<String, bool>),
}

#[derive(JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum FormatterEntry {
    /// Single preset or named definition: `r = "air"`.
    Single(String),
    /// Sequential chain: `python = ["isort", "black"]`.
    Multiple(Vec<String>),
    /// Named definition table: `[formatters.air] args = [...]`.
    Definition(FormatterDefinition),
}

pub fn extensions_schema(generator: &mut SchemaGenerator) -> Schema {
    <HashMap<String, ExtensionEntry> as JsonSchema>::json_schema(generator)
}

pub fn formatters_schema(generator: &mut SchemaGenerator) -> Schema {
    <HashMap<String, FormatterEntry> as JsonSchema>::json_schema(generator)
}
