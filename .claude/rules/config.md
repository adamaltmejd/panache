---
paths:
  - "src/config.rs"
  - "src/config/types.rs"
  - "src/config/types/**"
  - "docs/guide/configuration.qmd"
  - "panache.schema.json"
  - "tests/config_schema.rs"
---

Configuration changes should preserve predictable defaults, compatibility, and
clear migration paths.

- Preserve config discovery precedence and failure behavior for explicit
  `--config` paths.
- Keep flavor/extension merging deterministic: start from flavor defaults, then
  apply user overrides.
- Maintain backward compatibility for deprecated keys/sections where currently
  supported; keep warnings explicit and actionable.
- Use canonical kebab-case keys while preserving documented aliases. Existing
  aliases to snake_case are only there for backwards compatibility and should
  not be used in new code.
- Update `docs/guide/configuration.qmd` whenever defaults, keys, or deprecation
  behavior changes.
- Add focused tests in `src/config.rs` for parsing, precedence, merge behavior,
  and deprecation handling when config behavior changes.
- Regenerate `panache.schema.json` with
  `UPDATE_EXPECTED=1 cargo test config_schema` whenever you add, rename, or
  change a config key, enum, or default. `tests/config_schema.rs` snapshots
  the schema and validates every fixture `panache.toml` against it — drift
  fails CI.
- `schemars` derives ignore `#[serde(alias = ...)]` on enum variants. When a
  config enum accepts multiple spellings (canonical + aliases), hand-write
  `impl JsonSchema` so the published schema accepts the same set the parser
  does. See `Flavor` and `PandocCompat` in `crates/panache-parser/src/options.rs`.
