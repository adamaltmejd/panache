# HTML conformance — running session recap

Rolling, terse handoff between sessions of the `html-conformance`
skill. Read at the start of a session for phase status, persistent
traps, and the latest session's "Suggested next sub-targets". At the
end of a session, **rewrite** the Latest session entry, add a
one-line entry to the Earlier sessions log, and merge any
still-relevant trap into the Persistent traps section. Keep the file
short — see `SKILL.md`'s "Session recap" section for length budget.

--------------------------------------------------------------------------------

## Persistent traps & invariants (cross-session)

These survive across sessions. Add to this list when a trap is
re-relevant (i.e. you'd warn a future session about it); fold it
back into a session entry only if it's purely historical.

### Disk + tooling

- **Disk lint cache at `~/.cache/panache/`** serves stale
  `undefined-anchor` (and other linter rule) results even after
  `cargo build`. Symptoms: unit tests pass, `panache lint` keeps
  emitting old diagnostics, `eprintln!` from changed code never
  fires. Fix: `rm -rf ~/.cache/panache/` (or
  `cache.enabled = false` in `panache.toml`). Validate via unit
  tests first; treat CLI as downstream.
- **Conformance comparison is whitespace-insensitive**:
  `normalize_native` collapses pandoc's pretty-printed multi-line
  block output to single-line. Visual diffs are misleading.

### Parser shape & losslessness

- **HTML_ATTRS is the structural pattern; never add synthetic
  tokens.** Expose attributes by tokenizing existing source bytes at
  finer granularity (split TEXT into
  `TEXT + WHITESPACE + HTML_ATTRS{TEXT} + TEXT`). Synthetic tokens
  break the tree-text-equals-input invariant.
- **Use source-byte slices, never literal strings, when emitting
  TEXT tokens** for HTML. `"<div"` literal vs `&rest[..4]` was the
  root of the `<DIV>` losslessness regression. Case-insensitive
  prefix matches give a false sense of byte-identity.
- **Same-line `<div>foo</div>` is ONE `HTML_BLOCK_TAG`**, not open
  + content + close. The close `</div>` lives inside a TEXT child
  of the open tag. Any naive `strip_suffix('>')` grabs the wrong
  `>`. Scan to the first **unquoted** `>` (see
  `parse_html_tag_attributes`).
- **Quoted attribute values can hide `<` and `>`.** Tag-bracket
  scanners must thread quote state across line boundaries; don't
  reset per-line. `count_tag_balance`, `find_multiline_open_end`,
  `pandoc_html_open_tag_closes` do this right.
- **Multi-line open-tag close branches diverge by tag class.** The
  `same_line_closed` short-circuit assumes single-line; void-tag
  multi-line opens take a separate early-exit returning
  `end_line_idx + 1` BEFORE the regular close-marker loop. Without
  the explicit branch the parser would scan content lines for a
  closing tag that doesn't exist (void tags have none) and run
  off the document. Likewise `same_line_closed` must guard
  `multiline_open_end.is_none()`.
- **Incomplete open tags caused projector infinite recursion.**
  `<embed\n`, `<div\n`, `<table\n` etc. (no `>` anywhere) were
  recognized as `RawBlock` under Pandoc, but pandoc-native treats
  them as paragraph text. The projector's `flush_html_block_tail_text`
  then reparsed the same bytes and re-emitted the same HTML_BLOCK,
  recursing forever. Fix: gate Pandoc BlockTag recognition on
  `pandoc_html_open_tag_closes(lines, line_pos, bq_depth)` in
  `block_dispatcher.rs::detect_prepared`. Multi-line opens still
  work because the helper scans subsequent lines (across blank
  lines, threading quotes) for an unquoted `>`. CommonMark must
  remain liberal: `<table\n` (no `>`) is a valid CM type-6
  RawBlock.
- **Self-closing `<tag/>` doesn't bump depth.** Depth-aware close
  matchers must check `bytes[j-1] == b'/'` at the closing `>`.
- **`input.lines()` strips newlines**; for losslessness-asserting
  parser tests use
  `crate::parser::utils::helpers::split_lines_inclusive` to build
  `lines: Vec<&str>`.
- **`HtmlBlockType::BlockTag` is `Box<dyn Any>`-roundtripped via
  the block dispatcher.** Adding a field works automatically;
  cargo's E0063 errors point at every literal site that needs
  updating.

### Pandoc tag categorization

- **Pandoc has THREE tag sets, not one**: strict block
  (`PANDOC_BLOCK_TAGS`), inline-block non-void
  (`PANDOC_INLINE_BLOCK_TAGS`), inline-block void
  (`PANDOC_VOID_BLOCK_TAGS`). Each requires distinct handling — the
  strict set always splits, the non-void set follows
  `inline_pending` and lifts as matched-pair, the void set follows
  `inline_pending` and emits a single RawBlock per instance. Source
  of truth: `pandoc/src/Text/Pandoc/Readers/HTML/TagCategories.hs`
  + `Readers/HTML.hs::isBlockTag`/`isInlineTag`.
- **`eitherBlockOrInline` is context-dependent.** Mirroring needs
  BOTH parser-side `cannot_interrupt` (don't break running paragraph)
  AND projector-side `inline_pending` tracking (don't split mid-text).
  Either alone is insufficient.
- **CommonMark and Pandoc `blockHtmlTags` lists differ in BOTH
  directions** by ~15 tags. Don't merge them. The parser's
  `is_commonmark` flag gates which list runs; the projector only
  runs under Pandoc and uses `is_pandoc_block_tag_name` directly.
- **Closing forms of strict-block, verbatim, inline-block, and void
  tags ALL ARE block starts under Pandoc.** Pandoc's `htmlBlock
  isBlockTag` matches both directions for any tag in
  `blockHtmlTags ∪ verbatimTags ∪ eitherBlockOrInline`. Routing in
  the parser: each category emits `BlockTag { closes_at_open_tag:
  true }` so the block ends on the open line. The dispatcher's
  `cannot_interrupt` gate keys ONLY on inline-block + void tag
  names — strict-block (`</p>`, `</nav>`, `</section>`) and verbatim
  (`</pre>`, `</style>`, `</script>`, `</textarea>`) closes get
  `YesCanInterrupt` and DO interrupt running paragraphs (matches
  pandoc). Inline-block / void closes follow `cannot_interrupt`
  semantics and stay inline inside running paragraphs
  (`foo\n</video>` → `Para[foo, SB, RI</video>]`). Earlier recap
  claims that "closing forms must be excluded" were wrong on all
  counts.
- **`<script>` is in `eitherBlockOrInline` AND `blockHtmlTags`.**
  Verbatim handling fires first via `VERBATIM_TAGS`; don't add
  `script` to `PANDOC_INLINE_BLOCK_TAGS`. Likewise `<pre>`,
  `<style>`, `<textarea>` membership in `PANDOC_BLOCK_TAGS` is
  harmless — the verbatim arm fires first.
- **`<style>`, PIs, `</script>`, and `<script type="math/tex…">`
  cannot interrupt a paragraph under Pandoc; `<pre>`/`<script>` open
  without math/tex/`<textarea>` DO** (LANDED 2026-05-10 / 2026-05-11).
  The non-interrupt set mirrors pandoc's `isInlineTag` predicate
  (`pandoc/src/Text/Pandoc/Readers/HTML.hs`):
  - `<style>` open AND close are SPECIAL-CASED to always be inline
    (commit fixing pandoc issue #10643).
  - `</script>` close is similarly special-cased to always be inline.
  - `<script>` open is inline ONLY when the `type` attribute starts
    with `math/tex` (case-insensitive prefix; e.g. `math/tex`,
    `math/tex; mode=display`). Every other `<script>` open is a
    `RawBlock`.
  - PIs (`<? … ?>`) match `T.take 1 name == "?"`.
  - Comments are always inline.
  - Pandoc's `eitherBlockOrInline` set (audio, button, iframe, …,
    plus void area/embed/source/track) returns True from
    `isInlineTag` because those tags are NOT in `blockTags`.
  Earlier RECAP entries claimed `<style>` was "the lone verbatim
  tag NOT in `blockHtmlTags` (verbatimHtmlBlocks only)" — wrong;
  pandoc's `blockHtmlTags` does include `style` and `textarea`. The
  behavior difference comes from `isInlineTag`'s special cases, not
  tag-set membership. Fix: `cannot_interrupt` in
  `HtmlBlockParser::detect_prepared` includes
  `HtmlBlockType::ProcessingInstruction`, `BlockTag`s where
  `tag_name == "style"`, `BlockTag`s where
  `is_closing && tag_name == "script"`, and `BlockTag`s where
  `!is_closing && tag_name == "script" && is_math_tex_script_open(ctx.content)`
  under `Dialect::Pandoc`. The math/tex helper inspects only
  `ctx.content` (single-line opens); multi-line `<script\n type="math/tex">`
  opens are an edge case not yet exercised by the corpus. Required
  adding an `is_closing: bool` field to `HtmlBlockType::BlockTag`
  (carries through every literal site). CommonMark stays liberal —
  paired CM/Pandoc parser fixtures pin any divergence.

### Projector tag splitting

- **`split_html_block_by_tags` walks bytes, not tokens.** It is
  depth-unaware (Phase 5 work for the few cases that need it) and
  context-tracked via `inline_pending`. Don't try to "merge" with
  `find_matching_close` (the smart-quote bracket scanner) — same
  name, different inputs.
- **Matched-pair lift for `<video>...</video>` must abandon when
  interior opens with a void block tag at column 0.** Pandoc-native
  emits per-tag (`<video>` RB, `<source>` RB, Para[fallback, SB,
  RawInline</video>]) — not a balanced lift. Helper
  `interior_starts_with_void_block_tag` peeks past leading
  newlines/whitespace; on hit, the open tag emits as a single
  RawBlock and the closing `</video>` falls into the trailing
  paragraph reparse as RawInline. Indentation before the void tag
  doesn't save the lift (pandoc abandons even with 4-space indent).
- **Inline-block open with no matched close must emit as RawBlock
  at fresh-block.** Falling through to `inline_pending=true` causes
  the trailing tail-text reparse to recurse on the same `<video>...`
  bytes (parser still recognizes the open tag, projector splits it
  again, …) → stack overflow. The same `interior_starts_with_void`
  bail and the no-match bail share the single-tag emit path.
- **`inline_pending` resets on consecutive newlines (≥ 2).** A
  blank line restarts pandoc's block parser; in our byte walker
  that's `\n\n`. Don't substitute "byte == whitespace" — single
  trailing whitespace shouldn't reset.
- **Inter-tag text demotes Para→Plain when butted against the next
  tag**; tail text does NOT demote. Use `flush_html_block_text`
  (inter-tag) vs `flush_html_block_tail_text` (end-of-block).
  Uniform demotion silently breaks `<form>\nfoo\n` and
  `<embed src="x"> trailing` shapes.
- **Plain/Para signal for `<div>` recursive reparse is
  `</div>`-side, not `<div>`-side**: `close_butted = byte_at(close_start - 1) != '\n'`.
  Demotion applies to the LAST block only, regardless of how many
  precede it.
- **`try_div_html_block` requires the WHOLE content to be a single
  `<div>...</div>`** with optional surrounding whitespace. Pass an
  exact `<div>...</div>` slice when calling on a sub-range.

### Refs / footnotes / heading-id resolution

- **`parse_pandoc_blocks` swaps in an inner `RefsCtx`** for the
  recursive `<div>` reparse (and any other call site). The swap
  belongs in `parse_pandoc_blocks` itself, not at call sites.
- **`build_refs_ctx` mutates `REFS_CTX` mid-build** (stages
  cite-num/example-num maps before the heading pre-pass). When
  swapping for an inner reparse, save outer FIRST (`mem::take`),
  THEN call `build_refs_ctx`, THEN install the result.
- **`heading_id_by_offset` is offset-keyed, not slug-keyed.** The
  inner CST's offsets are zero-based and don't intersect the
  outer's offset space. Tempting wrong fix: copy outer
  `heading_ids` into inner. Right fix: build a fresh inner ctx and
  optionally inherit cross-boundary refs/footnotes via
  `build_refs_ctx_inherited`.
- **`fenced_div` does NOT use `parse_pandoc_blocks`** — it walks
  the structural CST via `collect_block`. Fenced divs already
  resolve through the outer ctx; don't generalize the swap to
  fenced divs.
- **`AttributeNode::can_cast` accepts `HTML_ATTRS`**; the existing
  salsa walk picks up `<div id>` / `<span id>` automatically. No
  parallel salsa walk for HTML attrs.

### Out of scope / known divergences

- **`<!ENTITY x "y">` projects `Str "\"y\">"`** where pandoc emits
  `Quoted DoubleQuote [Str "y"]`. Smart-quote / Quoted feature
  gap; not html-conformance.
- **Outer-wins-over-inner ref-conflict**: pandoc's rule is
  document-order-first; we have inner-wins. No corpus exercises
  this; deferred.
- **Cross-boundary cite numbering** for `<div>` recursive reparse
  similarly deferred.
- **Top-level Para→Plain demotion at HTML strict-block / verbatim
  adjacency: LANDED 2026-05-10.** Parser-side fix in
  `Parser::close_paragraph_as_plain_if_open` +
  `html_block_demotes_paragraph_to_plain`, wired at the
  YesCanInterrupt branch in `core.rs`. Gated on `Dialect::Pandoc` +
  `parser_name == "html_block"` + `HtmlBlockType::BlockTag`. CST
  emits `PLAIN` instead of `PARAGRAPH`; projector trivially maps
  each. Don't reintroduce the projector-side demotion (reverted
  earlier the same day).

### Projector-as-second-stage-parser smell (architectural)

The pandoc-AST projector at `crates/panache-parser/src/pandoc_ast.rs`
is a **test-only diagnostic** for CST shape, not a runtime artifact.
Phases 1/5 landed structural retags (`HTML_BLOCK_DIV`,
`INLINE_HTML_SPAN`) but stopped short of lifting inner block content
into structural CST children. Today the projector still re-runs the
markdown parser on HTML block bodies via `parse_pandoc_blocks` /
`split_html_block_by_tags` / `flush_html_block_text` /
`flush_html_block_tail_text` / `try_div_html_block`. That makes the
conformance harness pass while the CST stays opaque — consumers
(linter, salsa, LSP, formatter) walking the CST don't see the
structural decisions pandoc encodes. **The path forward is parser
work** (lift inner blocks into CST children, retag PARAGRAPH→PLAIN
when appropriate, etc.); each lift collapses a chunk of projector
compensation into a trivial CST walk. Defensible reparses (table
cells via `parse_grid_cell_text` / `parse_cell_text_inlines`) match
how pandoc itself sub-parses cell content and can stay.

--------------------------------------------------------------------------------

## Phase progress

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | `<div>` block lift (HTML_BLOCK_DIV + HTML_ATTRS structural) | **Wrapper retag landed** (2026-05-08) — issue #263 closed; `<DIV>` losslessness fix landed. **Inner content NOT yet lifted into CST children** — still raw `HTML_BLOCK_CONTENT` TEXT tokens; projector reparses them. |
| 2 | `<span>` inline lift (INLINE_HTML_SPAN) | **Wrapper retag landed** (2026-05-08). Inner inlines mostly trivial (no recursive reparse needed). |
| 3 | Sectioning + verbatim corpus pin; `eitherBlockOrInline` lift | **Conformance landed** — non-void (2026-05-09); void (`<embed>`/`<area>`/`<source>`/`<track>`) (2026-05-10). Implementation leans on projector-side `inline_pending` tracking + byte walker; CST still opaque for split/matched-pair shapes. |
| 4 | Comments, PIs, declarations, CDATA projection | **Conformance landed** (2026-05-08); type-4 CM lowercase still gappy. CST opaque (these constructs project as RawBlock / RawInline). |
| 5 | `markdown_in_html_blocks` interaction edge cases | **Conformance landed** — depth-aware nested div, Plain/Para promotion, refs inheritance, **projector-level splitter** (`split_html_block_by_tags` byte walker + `parse_pandoc_blocks` recursive reparse), outer-matched-pair-abandons-on-void-interior. **The structural CST lift was deferred** — Phase 5's mechanism is the projector reparsing bytes, not the parser emitting structure. |
| 6 (new) | Lift inner HTML block content into structural CST children — `HTML_BLOCK_DIV` gets `PARAGRAPH` / `LIST` / etc. as direct children; `split_html_block_by_tags` / `flush_html_block_*` / `parse_pandoc_blocks` collapse into trivial CST walks; `PARAGRAPH→PLAIN` retag at adjacent-HTML-block boundary. | **Fix #1 landed (2026-05-10)** — `PARAGRAPH→PLAIN` retag at YesCanInterrupt for HTML BlockTag under Pandoc; +5 (132 → 137 html). **`<style>` + PI sub-target landed (2026-05-10)** — `cannot_interrupt` under Pandoc; +3 (137 → 140). **Fix #2 landed (2026-05-10)** — `html_div_block` reads open-tag attrs via `HTML_BLOCK_TAG → HTML_ATTRS` CST walk; pure projector cleanup, no delta. **`</script>` close cannot_interrupt landed (2026-05-10)** — `is_closing` field added to `HtmlBlockType::BlockTag`; +1 (140 → 141). **`<script type="math/tex…">` open cannot_interrupt landed (2026-05-11)** — `is_math_tex_script_open` helper inspects `ctx.content` attrs; +1 (141 → 142). Fixes #3-#4 from AUDIT.md still pending: lift `<div>` inner blocks into CST children, full `HTML_BLOCK` body structural split. |

Multi-line `<div>` open-tag structural HTML_ATTRS lift landed
(2026-05-09). Multi-line void open-tag now lifts via
`find_multiline_open_end` + simple per-line TEXT/NEWLINE emission
(2026-05-10). Inline-block / void closing forms (`</video>`,
`</embed>`) start single-line `RawBlock`s under Pandoc (2026-05-10).
Strict-block / verbatim closing forms (`</p>`, `</nav>`, `</section>`,
`</pre>`) likewise lift under Pandoc, with `closes_at_open_tag: true`
and CAN interrupt a running paragraph (no `cannot_interrupt` gate)
(2026-05-10).

--------------------------------------------------------------------------------

## Latest session — 2026-05-11 (`<script type="math/tex…">` open cannot_interrupt)

Closed the small follow-up flagged in the previous session. Pandoc's
`isInlineTag` special-cases `<script>` opens when the `type` attribute
starts with `math/tex` (case-insensitive prefix; covers `math/tex`,
`math/tex; mode=display`, etc.). Panache previously split these into
`Plain + RawBlock + Para` mid-paragraph; pandoc keeps the whole thing
as a single `Para` with the open/close tags as `RawInline`.

**Implementation**: rather than plumb tag attrs through
`HtmlBlockType::BlockTag` (would touch every literal site), added a
narrow `is_math_tex_script_open(content)` helper to
`block_dispatcher.rs` that parses the open tag with
`parse_html_tag_attributes` and matches `type` values whose
lowercased text starts with `math/tex`. Extended `cannot_interrupt`
in `HtmlBlockParser::detect_prepared` with
`!is_closing && tag_name == "script" && is_math_tex_script_open(ctx.content)`
under Pandoc dialect. At fresh-block / after-blank positions the
tag still lifts to `RawBlock` per pandoc-native; only the
mid-paragraph case takes the inline path.

Pinning fixtures: corpus case
`0335-html-block-paragraph-then-script-mathtex-open` (renumbered from
0334 to avoid collision with the existing `0334-citation-prefix-paren-escape`
case) + paired parser goldens
`html_block_paragraph_then_script_mathtex_open_{pandoc,commonmark}`.
The shapes diverge: Pandoc keeps `INLINE_HTML` inside a single
`PARAGRAPH`; CommonMark splits `<script>` as a verbatim
`HTML_BLOCK` since CM has no `isInlineTag` override.

Pass count: html 141 → 142 (333 → 335 total — both new conformance
ID 335 and the earlier-session new 333; the in-between 334 is the
existing citation case).

**Multi-line `<script\n type="math/tex">`** is not exercised; the
helper inspects only `ctx.content` (single line). Edge case;
revisit if a corpus case lands.

### Suggested next sub-targets

1. **Fix #3 — lift `<div>` inner blocks into structural CST
   children** (Phase 6 proper, medium). Top recommendation. Collapses
   `parse_pandoc_blocks` recursive reparse + `close_butted` rule +
   cross-boundary `RefsCtx` swap. Approach: fixture-first paired
   goldens for `<div>\nfoo\n</div>` (Para), `<div>foo</div>` (Plain),
   `<div>\n# h\n</div>` (Heading inside Div); change
   `parse_html_block`'s matched-`</div>` path to invoke the block
   dispatcher recursively instead of capturing TEXT in
   `HTML_BLOCK_CONTENT`; update the projector to walk children.
   Formatter idempotency is the risk surface — pin
   `tests/fixtures/cases/` goldens before touching the projector.
   Substantial enough to fill a session; may need sub-division.
2. **Fix #4 — full HTML_BLOCK body structural split** (large; defer
   until #3 lands the pattern). Eliminates `split_html_block_by_tags`,
   both flush helpers, `interior_starts_with_void_block_tag`,
   `find_matching_html_close*`, `inline_pending` flag.
3. **Multi-line `<script\n type=...>`** corpus pin — only if a real
   case appears.

### Files in committable diff

- `crates/panache-parser/src/parser/block_dispatcher.rs` —
  `is_math_tex_script_open` helper + `cannot_interrupt` extension
  + revised comment; new `parse_html_tag_attributes` import.
- `crates/panache-parser/tests/fixtures/pandoc-conformance/corpus/0335-…/`
  + `tests/pandoc/allowlist.txt` — new corpus case + rewritten
  prior-session allowlist comment.
- `crates/panache-parser/tests/fixtures/cases/html_block_paragraph_then_script_mathtex_open_{pandoc,commonmark}/`
  + `tests/golden_parser_cases.rs` + accepted snapshots — paired
  parser fixtures.
- `.claude/skills/html-conformance/RECAP.md` —
  `<style>`/PI/`</script>`/math-tex bullet expanded; this Latest
  session; demoted earlier 2026-05-10 entry to the log.

### New traps

None new — the Persistent traps bullet about `<style>`/PI/
`</script>`/cannot_interrupt has been expanded in place to cover
the math/tex case, including the note that the helper inspects
only `ctx.content` (single-line opens).

--------------------------------------------------------------------------------

## Earlier sessions (compact log)

Newest first. One line per session: date — phase/sub-target — pass
count delta — root cause / lever.

- 2026-05-10 — Tag-set audit; `</script>` close cannot_interrupt; corrected
  `<style>` rationalization — html 140 → 141 — confirmed panache tag-set
  constants match pandoc's `TagCategories.hs`; root cause for non-interrupt
  set is `isInlineTag` special cases (issue #10643), not tag-set membership.
  Added `is_closing: bool` to `HtmlBlockType::BlockTag`; extended
  `cannot_interrupt` with `is_closing && tag_name == "script"`. Flagged
  `<script type="math/tex">` as follow-up (closed 2026-05-11).
- 2026-05-10 — Phase 6 Fix #2 (`html_div_block` structural CST walk)
  — html 140 → 140 — replaced byte-rescan of `<div ATTRS>` with
  `HTML_BLOCK_TAG → HTML_ATTRS` walk; shared
  `extract_div_inner_and_butted` + `assemble_div_block` helpers (pure
  projector cleanup).
- 2026-05-10 — Phase 6 sub-target: `<style>` + PI cannot_interrupt
  under Pandoc — html 137 → 140 — extended `cannot_interrupt` to
  include PI + BlockTag(`style`). (Original session blamed tag-set
  membership; the later 2026-05-10 audit corrected to pandoc's
  `isInlineTag` special-case per issue #10643.)
- 2026-05-10 — Phase 6 Fix #1: PARAGRAPH→PLAIN retag at HTML
  strict/verbatim adjacency — html 132 → 137 — new
  `Parser::close_paragraph_as_plain_if_open` at YesCanInterrupt in
  `core.rs`; gated on Pandoc + html_block + BlockTag.
- 2026-05-10 — Projector audit; AUDIT.md landed — html 132 → 132 —
  inventoried `pandoc_ast.rs` (5,696 lines); ranked parser-side fix
  list (#1 PARAGRAPH→PLAIN, #2 `html_div_block` structural walk, #3
  `<div>` inner-block lift, #4 full HTML_BLOCK split).
- 2026-05-10 — Course correction; aborted projector Para→Plain
  demotion reverted — html 132 → 132 — projector compensation
  defeats the diagnostic; added "What this skill is NOT" to SKILL.md.
- 2026-05-10 — Strict-block + verbatim closing-form lift, inline-block
  matched-pair-abandons-on-void-interior, multi-line void open-tag
  recognition, incomplete open-tag projector recursion fix, Phase 3
  void `eitherBlockOrInline` lift — html 105 → 132 — new
  `try_parse_html_block_start` close-tag branches; `closes_at_open_tag`
  for closing forms; `pandoc_html_open_tag_closes` gate;
  `PANDOC_VOID_BLOCK_TAGS`; `interior_starts_with_void_block_tag`;
  split `_text`/`_tail_text` helpers.
- 2026-05-09 — Phase 3 lifts (eitherBlockOrInline non-void; HTML5
  sectioning corpus; `<DIV>` losslessness; Phase 5 div Plain/Para +
  multi-line attrs + refs inheritance) — html 62 → 105 — context-aware
  projector `inline_pending` + parser `cannot_interrupt`; CM/Pandoc
  blockHtmlTags split; `build_refs_ctx_inherited` + `mem::take` swap.
- 2026-05-08 — Phases 1-5 seed work (issue #263 closed) — html 0 →
  62 — `HTML_BLOCK_DIV`/`INLINE_HTML_SPAN` retag, `HTML_ATTRS`
  tokenization, type-4/5 gating, sectioning/verbatim corpus pin,
  depth-aware nested `<div>`.
