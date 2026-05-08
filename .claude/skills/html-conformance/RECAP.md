# HTML conformance — running session recap

This file is the rolling, terse handoff between sessions of the
`html-conformance` skill. Read it at the start of a session for the
suggested next sub-target and known traps; rewrite the **Latest session**
entry at the end with what changed and what to look at next.

Keep entries short. Pass counts + a one-line root cause beat a narrative.
The hard-won judgment calls (why a lever was chosen, why an approach was
reverted, what trap to avoid) are the load-bearing content here.

--------------------------------------------------------------------------------

## Latest session — 2026-05-08 (Phase 1 — `<div>` block lift)

**html-block pass count: 0 → 9** (10 corpus cases seeded; 9 passing,
1 blocked as nested-div Phase 5 target).
**Workspace test count: 0 failing → 0 failing** (all green).

### What landed

Phase 1 ships **two** structural CST changes for `<div>` HTML
blocks under `Dialect::Pandoc`, both byte-lossless:

1. **Wrapper retag**: `HTML_BLOCK` → `HTML_BLOCK_DIV` for matched
   div blocks. Gated on `Dialect::Pandoc && extensions.native_divs
   && tag_name == "div"`.
2. **Open-tag tokenization**: inside the open `HTML_BLOCK_TAG`,
   the bytes `<div ATTRS>` are split into
   `TEXT("<div") + WHITESPACE + HTML_ATTRS{TEXT(attrs)} + TEXT(">")`.
   `HTML_ATTRS` is a new `SyntaxKind`. Source bytes unchanged —
   just finer granularity.

`AttributeNode::can_cast` accepts `HTML_ATTRS`. The existing
salsa indexer's `for attr in
tree.descendants().filter_map(AttributeNode::cast)` walk picks up
`<div id>` automatically, the same way it handles fenced-div
`DIV_INFO` and heading `ATTRIBUTE`. **No parallel salsa walk** —
my earlier sketch had one; it was deleted as redundant.

`AttributeNode::id()` and `id_value_range()` route by
`SyntaxKind`: `HTML_ATTRS` uses `parse_html_attribute_list`
(public sibling helper extracted from
`parse_html_tag_attributes`); other kinds use the existing
`try_parse_trailing_attributes` for `{...}` pandoc syntax.

Block dispatcher decides the wrapper kind in
`parser/block_dispatcher.rs::parse_prepared`; the actual
emission lives in new `parse_html_block_with_wrapper` in
`parser/blocks/html_blocks.rs`. The open-tag tokenization helper
`emit_div_open_tag_tokens` handles quoted attribute values
correctly (a same-line `<div id="x">Content</div>` doesn't get
its open-tag `>` confused with the close tag's `>`).

Projector got an `HTML_BLOCK_DIV` match arm in `pandoc_ast.rs`
that delegates to the existing `try_div_html_block` byte-level
reparser. **The projector did NOT simplify** — it gained a
parallel arm that produces the same `Block::Div` output as
before. Future structural recursion (Phase 5) will replace
`try_div_html_block` with a CST walk.

Formatter accepts `HTML_BLOCK_DIV` wherever it accepts
`HTML_BLOCK` (text emission is identical because the wrapper
walk goes through `descendants_with_tokens` and emits all
tokens verbatim regardless of structure).

### What Phase 1 still does NOT do

- **Recursive content parsing.** Bytes inside the div (between
  open and close tags) are still raw TEXT in
  `HTML_BLOCK_CONTENT`, not block-parsed at parse time. The
  pandoc-native projector reparses them on demand. A real
  structural lift would have `PARAGRAPH`, `LIST`, etc. as direct
  children of `HTML_BLOCK_DIV`.
- **Multi-line open tags.** `<div\n  id="x">` falls back to opaque
  `HTML_BLOCK` because `try_parse_html_block_start` only inspects
  the first line. Edge case.
- **Nested divs (corpus id 199).** The HTML-block scanner is
  depth-unaware; outer div closes at the first inner `</div>`.
  Phase 5 target.

### Files in committable diff

- `crates/panache-parser/src/syntax/kind.rs` (new variant)
- `crates/panache-parser/src/parser/blocks/html_blocks.rs`
- `crates/panache-parser/src/parser/block_dispatcher.rs`
- `crates/panache-parser/src/parser/utils/attributes.rs`
- `crates/panache-parser/src/pandoc_ast.rs`
- `crates/panache-formatter/src/formatter/core.rs`
- `crates/panache-formatter/src/utils.rs`
- `src/salsa.rs`
- `src/linter/rules/undefined_anchor.rs` (2 new tests)
- `crates/panache-parser/tests/pandoc/allowlist.txt`
  (9 new ids under `# html-block`)
- `crates/panache-parser/tests/pandoc/blocked.txt` (199 nested div)
- `crates/panache-parser/tests/fixtures/pandoc-conformance/corpus/`
  — 10 new `<NNNN>-html-block-<slug>/` directories
- `crates/panache-parser/tests/fixtures/cases/html_block_div_with_id_pandoc/`
  + `_commonmark/` paired parser fixtures (+ snapshots)
- Updated existing snapshots: `parser_cst_html_block.snap`,
  `parser_cst_html_block_commonmark_type6_type7_pandoc.snap` (pure
  HTML_BLOCK → HTML_BLOCK_DIV retag, byte-identical CST).
- `tests/fixtures/cases/html_block_div_idempotent/` formatter
  golden (round-trip pinning).
- `docs/reference/linter-rules.qmd` (removed `<div id>` limitation
  note; kept `<a id>` / `<a name>`).
- `crates/panache-parser/tests/pandoc/report.txt` +
  `docs/development/pandoc-report.json` (regenerated).
- `.claude/skills/html-conformance/SKILL.md` + `RECAP.md` (new).

### Issue #263 status

**Closed.** `<div id="anchor-c">Content.</div>\n\nSee
[link](#anchor-c).\n` no longer raises `undefined-anchor`. Verified
via:
- 2 new unit tests in
  `src/linter/rules/undefined_anchor.rs`.
- Manual CLI repro: `panache lint /tmp/263.md` → "No issues found".
- Corpus case `0201-html-block-div-issue-263` passes against
  pandoc-native.

### Suggested next sub-targets, ranked

1. **Phase 2 — Inline `<span>` lift.** Mirror Phase 1 minimally:
   add `INLINE_HTML_SPAN` SyntaxKind, retag the existing
   `INLINE_HTML` wrapper when a balanced `<span>...</span>` is
   recognized under Pandoc. Coordinate with `pandoc-ir-migrate`
   Phase 1 — IR's opaque scan stays; the parser-side retag is
   complementary. Probe `*foo <span>bar</span> baz*` to confirm
   emphasis doesn't pair into the span.
2. **Phase 3 — Negative-space pin.** Add ~5-8 corpus cases for
   `<section>`, `<article>`, `<aside>`, `<nav>` (stay as
   `RawBlock`) and verbatim tags `<pre>`/`<style>`/`<script>`/
   `<textarea>` (no markdown inside). Most should pass without
   any code change; goal is corpus coverage so future regressions
   are caught.
3. **Phase 5 (nested div, blocked.txt id 199)** — needs depth-aware
   pre-scan in `parser/blocks/html_blocks.rs`. Higher complexity
   than Phase 2/3; defer until Phase 2 lands.

### Don't redo / known traps (new this session)

- **Disk lint cache at `~/.cache/panache/` serves stale
  `undefined-anchor` results.** This bit me hard during salsa
  development: `cargo build` succeeds, unit tests pass, but
  `panache lint` keeps emitting the OLD diagnostic. The CLI reads
  cached lint output keyed on a tool-fingerprint that did NOT
  invalidate when I changed the lint rule. Fix: `rm -rf
  ~/.cache/panache/` between debugging runs, OR set
  `cache.enabled = false` in `panache.toml`. Always validate the
  rule via unit tests first; CLI is downstream. (Also documented
  in top-level `AGENTS.md`.)
- **`<div id="x">Content</div>` on one line is ONE
  `HTML_BLOCK_TAG`, not two.** The parser's `is_closing_marker`
  match fires on the same line as the open. The open-tag
  tokenization helper `emit_div_open_tag_tokens` therefore must
  scan to the first **unquoted** `>` — both the helper and
  `parse_html_tag_attributes` get this right; `strip_suffix('>')`
  would grab the close tag's `>` and break things.
- **HTML_ATTRS is the structural pattern; do NOT add synthetic
  tokens.** The right way to expose attributes structurally is
  finer-grained tokenization of the EXISTING source bytes (split
  one TEXT into `TEXT + WHITESPACE + HTML_ATTRS{TEXT} + TEXT`).
  This preserves losslessness because no new bytes are emitted.
  Adding synthetic ATTRIBUTE tokens — like the rejected initial
  draft did — would duplicate bytes and break the
  tree-text-equals-input invariant.
- **An earlier draft of Phase 1 had a parallel salsa walk for
  `HTML_BLOCK_DIV`.** It was redundant once `HTML_ATTRS` got
  added to `AttributeNode::can_cast`. The parallel walk was
  deleted. If you find yourself adding a new walk for a kind
  that "looks like an attribute region", check whether you can
  add it to `AttributeNode::can_cast` instead — that's the
  established pattern (see `DIV_INFO`, `ATTRIBUTE`,
  `SPAN_ATTRIBUTES` are all SPAN_ATTRIBUTES).
- **The legacy `try_div_html_block` byte-level reparser in
  `pandoc_ast.rs` STAYS.** It's still how the projector renders
  the div's inner content, since the CST keeps the inner bytes
  as raw TEXT. Don't delete until Phase 5 produces structural
  inner blocks at parse time.
- **Existing parser snapshots that contain `<div>` under Pandoc
  WILL change** when this lands. Three fixtures hit this in
  Phase 1; all diffs are pure tokenization-granularity changes
  (same bytes, more nodes). Don't blanket-accept — review each
  to confirm bytes are unchanged.
