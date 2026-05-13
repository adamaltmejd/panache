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
  tokens.** Expose attributes by tokenizing existing source bytes
  (split TEXT into `TEXT + WS + HTML_ATTRS{TEXT} + TEXT`).
  Synthetic tokens break the tree-text-equals-input invariant.
  Use source-byte slices (`&rest[..4]`), never literals (`"<div"`)
  for case-insensitive prefix matches.
- **Same-line `<div>foo</div>` is ONE `HTML_BLOCK_TAG`** — close
  lives inside a TEXT child of the open. Naive `strip_suffix('>')`
  grabs wrong `>`; scan to first **unquoted** `>`. Quoted attribute
  values hide `<` / `>`; tag-bracket scanners thread quote state
  across line boundaries (`count_tag_balance`,
  `find_multiline_open_end`, `pandoc_html_open_tag_closes`).
- **Multi-line open-tag close branches diverge by tag class** —
  void multi-line opens early-exit returning `end_line_idx + 1`
  BEFORE close-marker loop. `same_line_closed` short-circuit must
  guard `multiline_open_end.is_none()`.
- **Incomplete open tags (`<embed\n`, no `>` anywhere) caused
  projector infinite recursion.** Pandoc treats as paragraph text.
  Gate Pandoc BlockTag recognition on `pandoc_html_open_tag_closes`
  in `block_dispatcher::detect_prepared`. CommonMark stays liberal.
- **Self-closing `<tag/>` doesn't bump depth.** Depth-aware close
  matchers check `bytes[j-1] == b'/'` at closing `>`.
- **`input.lines()` strips newlines**; for losslessness-asserting
  parser tests use `split_lines_inclusive`.
- **`HtmlBlockType::BlockTag` is `Box<dyn Any>`-roundtripped via
  block dispatcher.** Adding a field works automatically; E0063
  points at every literal site.

### Pandoc tag categorization

- **Pandoc has THREE tag sets**: strict block (`PANDOC_BLOCK_TAGS`),
  inline-block non-void (`PANDOC_INLINE_BLOCK_TAGS`), inline-block
  void (`PANDOC_VOID_BLOCK_TAGS`). Strict always splits; non-void
  follows `inline_pending` + matched-pair lift; void follows
  `inline_pending` + emits single RawBlock. Source:
  `pandoc/.../TagCategories.hs` + `Readers/HTML.hs::isBlockTag` /
  `isInlineTag`. CommonMark and Pandoc `blockHtmlTags` lists differ
  in both directions (~15 tags); don't merge. Parser gates on
  `is_commonmark`; projector runs Pandoc only.
- **`eitherBlockOrInline` is context-dependent.** Mirror needs BOTH
  parser-side `cannot_interrupt` (don't break running paragraph) AND
  projector-side `inline_pending` (don't split mid-text).
- **Closing forms of all matched-pair tag sets ARE block starts
  under Pandoc** — each emits `BlockTag { closes_at_open_tag: true }`.
  Dispatcher's `cannot_interrupt` keys on inline-block + void only:
  strict-block + verbatim closes get `YesCanInterrupt`; inline-block
  / void closes stay inline in running paragraphs.
- **Verbatim tags fire before inline-block / strict-block arms** —
  `VERBATIM_TAGS` checked first; script-in-eitherBlockOrInline +
  style/textarea-in-blockHtmlTags overlap is harmless.
- **Pandoc `isInlineTag` special cases (issue #10643):** `<style>`
  open+close, `</script>`, PIs, comments, `<script
  type="math/tex…">` (case-insensitive, single-line) cannot
  interrupt paragraph. `<pre>` / non-math-tex `<script>` /
  `<textarea>` DO interrupt. Implemented in
  `HtmlBlockParser::detect_prepared`'s `cannot_interrupt`;
  requires `is_closing: bool` on `HtmlBlockType::BlockTag`.
- **`HtmlBlockType::BlockTag.is_closing` — match guards pivoting on
  tag identity MUST check it.** `pandoc_html_open_tag_closes`
  returns true for both `<div>` and `</div>` (scans for first `>`).
  Gates firing on `tag_name == "div"` alone wrongly retag close
  forms. `HTML_BLOCK_DIV` retag destructures `is_closing: false`;
  `</div>` without matched open keeps opaque HTML_BLOCK → single
  RawBlock per pandoc-native.

### Projector tag splitting

- **`split_html_block_by_tags` walks bytes, not tokens.**
  Context-tracked via `inline_pending`; runs for opaque
  HTML_BLOCKs only (comments, PI, verbatim, void tags, unmatched
  strict / inline-block tags). Matched-pair `<div>` is parser-
  lifted now. `<video>...</video>` matched-pair lift abandons
  when interior opens with void block tag at col 0
  (`inline_block_void_interior_abandons`). Inline-block open with
  no matched close also emits RawBlock — falling through to
  `inline_pending=true` causes stack overflow via tail-text
  reparse recursion.
- **`inline_pending` resets on consecutive newlines (≥ 2).**
  Inter-tag text demotes Para→Plain when butted against next tag;
  tail text does NOT demote. Use `flush_html_block_text` vs
  `flush_html_block_tail_text`.
- **HTML blocks inside blockquotes need
  `collect_html_block_text_skip_bq_markers`** on remaining
  byte-walker paths — parser keeps `BLOCK_QUOTE_MARKER + WS` as
  structural tokens; passing `node.text()` re-recognizes `> ` as
  nested bq. Remaining caller: `emit_html_block` for verbatim in
  bq.
- **Projector `open_tag_raw_block_text` canonicalizes multi-line
  open tags.** With `HTML_ATTRS`, literal source diverges from
  pandoc's canonical single-line form (`normalize_native`
  preserves WS inside `"..."`). Helper walks
  `children_with_tokens`, takes leading `<tagname` TEXT, joins
  HTML_ATTRS trimmed texts with single spaces, appends `>`.
  Single-line opens without HTML_ATTRS keep literal text.

### Refs / footnotes / heading-id resolution

- **`parse_pandoc_blocks` swaps in an inner `RefsCtx`** for
  recursive reparse. Swap belongs IN `parse_pandoc_blocks`, not
  at call sites. `build_refs_ctx` mutates `REFS_CTX` mid-build —
  when swapping save outer FIRST via `mem::take`, THEN call
  `build_refs_ctx`, THEN install.
- **`heading_id_by_offset` is offset-keyed, not slug-keyed.**
  Inner CST's offsets are zero-based; don't copy outer
  `heading_ids` into inner. Build fresh inner ctx and inherit
  cross-boundary refs/footnotes via `build_refs_ctx_inherited`.
- **`fenced_div` walks structural CST via `collect_block`** —
  doesn't use `parse_pandoc_blocks`. Don't generalize the swap
  to fenced divs.
- **`AttributeNode::can_cast` accepts `HTML_ATTRS`**; the salsa
  walk picks up `<div id>` / `<span id>` and non-div strict-block
  tag ids (`<section id="x">`, etc.) automatically. Diverges
  from pandoc-native (which keeps them as RawBlock without
  lifting attrs) but matches user intent for anchor-link
  resolution. No parallel salsa walk.

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
  adjacency** is parser-side
  (`Parser::close_paragraph_as_plain_if_open` +
  `html_block_demotes_paragraph_to_plain`, wired at
  YesCanInterrupt in `core.rs`). CST emits `PLAIN`; projector
  trivially maps. Don't reintroduce projector-side demotion.

### Projector-as-second-stage-parser smell (architectural)

`pandoc_ast.rs` is the public `to_pandoc_ast` API; linter / salsa
/ LSP / formatter walk the CST, not the projector. Phases 1/5
landed structural retags (`HTML_BLOCK_DIV`, `INLINE_HTML_SPAN`);
Phase 6 lifted inner content of `<div>` / non-div strict-block /
inline-block matched-pair shapes (non-bq + bq) into CST children.
Vestigial `<div>` byte walkers (`try_div_html_block`, etc.)
pruned 2026-05-11. Load-bearing remainder: `split_html_block_by_tags`
(opaque HTML_BLOCKs only), `parse_pandoc_blocks` (inter-tag text
reparse via `flush_html_block_text` /
`flush_html_block_tail_text`), `collect_html_block_text_skip_bq_markers`
(one `<pre>` verbatim-in-bq case + multi-line-open-in-bq
fallback), table-cell reparses. `html_div_block` `debug_assert!`s
on unlifted HTML_BLOCK_DIV.

### Structural lift (Fix #3 / Fix #4 family)

- **Recursive parse uses `parse_with_refdefs`, not `parse`.**
  `parse` re-runs `populate_refdef_labels` on JUST the inner
  text, hiding outer refdefs from inner reference links. Thread
  outer config's `refdef_labels` through.
- **`HTML_BLOCK_DIV` retag at dispatcher is two-pronged.** Retag
  fires iff `probe_open_tag_line_has_close_gt(ctx.content, "div")`
  (single-line) OR `pandoc_html_open_tag_closes(lines, line_pos,
  bq_depth)` (multi-line). Incomplete opens (`<div\n` no `>`
  anywhere) keep opaque HTML_BLOCK so projector treats as
  paragraph text. Multi-line + trailing on close-`>` line:
  `emit_multiline_open_tag_with_attrs` captures trailing into
  `pre_content` via `lift_trailing=true` so open `HTML_BLOCK_TAG`
  ends cleanly with `TEXT(">")`.
- **Lifted HTML_BLOCK / HTML_BLOCK_DIV MUST route structural,
  not byte path.** `collect_block` routes `HTML_BLOCK_DIV` →
  `html_div_block`; `emit_html_block` routes lifted HTML_BLOCK →
  `emit_html_block_structural` (not `split_html_block_by_tags`).
  Byte path's `parse_pandoc_blocks` builds fresh inner `RefsCtx`
  → re-disambiguates heading auto-ids, producing stray `-1`
  suffix. Body-lifted signal: no `HTML_BLOCK_CONTENT` child;
  `html_block_open_tag_is_clean` accepts TEXT ending in `>`.
- **`LastParaDemote` enum** on `graft_document_children`:
  `Never` (clean/unbalanced — Para preserved), `SkipTrailingBlanks`
  (div close-butted — demote LAST PARAGRAPH past trailing
  BLANK_LINEs), `OnlyIfLast` (non-div strict-block close —
  demote only when last child is PARAGRAPH with no trailing
  BLANK_LINE).
- **Multi-line open tags emit multiple `HTML_ATTRS` regions** —
  one per attribute line. Iterate + join with `" "` (see
  `cst_div_open_tag_attr`); `.children().find()` only sees first.
- **All non-bq shapes lift** for `<div>` and non-div Pandoc
  strict-block + inline-block matched-pair tags: clean
  multi-line, open-trailing, butted-close, indented-close,
  same-line, empty/blank-only, multi-line open + trailing.
- **Bq lift covers clean + same-line + messy + multi-line-open-
  clean.** Open-line `> ` consumed by outer BLOCK_QUOTE;
  subsequent lines' `> ` re-injected via `BqPrefixState`. Deeper
  bq (`> > <div>`) works transparently. `find_multiline_open_end`
  + `emit_multiline_open_tag_with_attrs/_simple` thread `bq_depth`
  and re-emit `BLOCK_QUOTE_MARKER + WHITESPACE` prefix tokens for
  lines past `start_pos` (line 0's prefix is owned by outer BQ).
- **Bq prefix re-injection: both `NEWLINE` *and* `BLANK_LINE`
  token (kind, not node) advance `line_idx`.** Inner parse puts
  `BLANK_LINE` token (text `"\n"`) inside `BLANK_LINE` node;
  treating only NEWLINE mis-aligns prefixes — losslessness
  violation when blank line precedes content line in body.
- **Three bq lift gates by `depth` after open line.** All require
  `bq_depth > 0` + `depth_aware_tag.is_some()` + tag in
  `is_pandoc_lift_eligible_block_tag`. Inline-block matched-pair
  also gates on NOT `inline_block_void_interior_abandons`.
  Discriminators:
  - `same_line_bq_lift_tag` — `depth <= 0`, single-line. Routes
    through `same_line_closed` branch; uses
    `emit_html_block_body_lifted` with `bq: &mut None`.
    Demote: div=SkipTrailingBlanks, non-div=OnlyIfLast.
  - `bq_clean_lift` — `depth > 0` + close line is `trim_start
    .starts_with("</")` + clean open (`pre_content.is_empty()`).
    Accepts single + multi-line opens. Calls
    `emit_html_block_body_lifted_bq`. Demote: div=Never (Para
    preserved), non-div=OnlyIfLast.
  - `bq_messy_lift_tag` — `depth > 0` + NOT clean. Accepts both
    open shapes; multi-line + trailing uses `lift_trailing` so
    trailing → `pre_content`. Close-marker site bq-strips then
    `try_split_close_line`. Calls
    `emit_html_block_body_lifted_bq_messy`. Demote: div keyed on
    close-butted (Never when `leading` empty, else
    SkipTrailingBlanks); non-div=OnlyIfLast.
- **`try_split_close_line` whitespace-only `leading` is close-tag
  indent, not body content.** For `   </article>`, classify
  whitespace-only via `leading.bytes().all(|b| b == b' ' || b ==
  b'\t')`, pass `body_leading=""` to recursive parse, emit
  leading bytes as `WHITESPACE` inside close `HTML_BLOCK_TAG`.
  Keep demote policy keyed on **original** `leading.is_empty()`.
- **Bq messy-lift duplicate-prefix trap.**
  `emit_html_block_body_lifted_bq_messy` injects close line's bq
  prefix in front of `leading` via BqPrefixState; close
  `HTML_BLOCK_TAG` MUST NOT re-emit `emit_bq_prefix_tokens`
  when `leading` is non-empty (doubles `> ` bytes).
- **Projector `open_tag_raw_block_text` strips bq markers AND
  leading 1-3 space indent.** Bq-wrapped close `> </form>`
  carries `BLOCK_QUOTE_MARKER + WHITESPACE` leading tokens;
  open-line `  <article>` carries standalone `WHITESPACE` before
  tag-name TEXT. Pandoc-native `RawBlock` text is tag bytes only
  — helper skips bq prefix pairs AND leading `WHITESPACE` before
  the accumulator collects its first non-WS token. HTML_ATTRS
  branch (multi-line open canonicalization) unaffected.

### List-item HTML structural lift

- **`ListItemBuffer::emit_as_block` lifts same-line / fully-
  contained HTML blocks via `try_emit_html_block_lift`.** Gate is
  strict: `try_parse_html_block_start` must recognize the first
  line, the inner reparse must produce exactly ONE top-level child
  of kind `HTML_BLOCK` / `HTML_BLOCK_DIV`, the child must consume
  every byte of the buffer text, and `HTML_BLOCK_DIV` requires
  ≥ 2 `HTML_BLOCK_TAG` children (matched open+close). Multi-line
  shapes (`- <section>\n  hello\n  </section>`, `- <video>\n  body\n
  </video>`) also lift as of 2026-05-13 — see "Close-form
  dispatcher gate" trap.
- **Close-form dispatcher gate (multi-line list-item HTML).** The
  dispatcher's HTML-block close-form recognition (`</div>`,
  `</section>`, `</pre>`, …) is gated on the enclosing LIST_ITEM
  buffer NOT having an unclosed matched-pair open of the same
  tag. Mechanism: `BlockContext::list_item_unclosed_html_block_tag:
  Option<String>` is populated in `parse_line` via
  `Parser::list_item_unclosed_html_block_tag` → `ListItemBuffer::
  unclosed_pandoc_matched_pair_tag` → which inspects the first
  buffer text segment with `try_parse_html_block_start`, checks
  it's a `BlockTag { is_closing: false }` matching
  `is_pandoc_matched_pair_tag`, then walks all buffer text
  segments calling `count_tag_balance`. When opens > closes,
  returns the tag name; `HtmlBlockParser::detect_prepared`
  returns `None` for close-forms whose tag matches the field.
  The buffer then accumulates the full matched-pair text, and
  `try_emit_html_block_lift` reparses + grafts. `count_tag_balance`,
  `is_pandoc_lift_eligible_block_tag`, and new
  `is_pandoc_matched_pair_tag` are now `pub(crate)`. The gate
  only fires under Pandoc dialect.
- **List-item indent normalization gap (multi-line `<div>` /
  `<pre>` in list).** Pandoc strips the list-item `content_col`
  leading spaces from continuation lines before reparsing the
  body. The buffer's `try_emit_html_block_lift` reparses the
  raw buffer text WITH leading spaces, so the inner body has
  2-space leading at column 0 of the inner parse. Symptoms:
  `- <div>\n  body\n  </div>` projects as `Div [Plain [body]]`
  (because indented `body` looks like indented-code-or-Plain)
  instead of pandoc's `Div [Para [body]]`. `<pre>` verbatim
  content retains the indent (`<pre>\n  foo\n  </pre>` vs
  pandoc's `<pre>\nfoo\n</pre>`). Fix path: thread `content_col`
  through `emit_as_block`, pre-strip on continuation lines,
  re-inject the stripped bytes as `WHITESPACE` tokens during
  graft to preserve losslessness. Substantial — defer.
- **`format_list_item` silently drops `LIST_MARKER` when the
  list item has NO `PLAIN`/`PARAGRAPH` content_node.** The
  marker-emit pass is wired to the wrapping flow which produces
  no output without a content_node. Per-kind arms in the
  nested-blocks loop emit the marker when
  `no_content_emitted && is_first_real_child`: existing
  `HORIZONTAL_RULE` arm, added `HTML_BLOCK | HTML_BLOCK_DIV` arm
  for the same-line HTML lift. Any new structural lift that
  produces a list-item-as-block CST shape (HEADING-only,
  BLOCK_QUOTE-only, FENCED_DIV-only, etc.) MUST update
  `format_list_item` with the same pattern or the marker
  silently disappears. The `_` fallback at the end of the loop
  just calls `format_node_sync` with content_indent — it does
  NOT emit the marker.

--------------------------------------------------------------------------------

## Phase progress

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | `<div>` block lift (HTML_BLOCK_DIV + HTML_ATTRS structural) | **Wrapper retag landed** (2026-05-08) — issue #263 closed; `<DIV>` losslessness fix landed. **Inner content NOT yet lifted into CST children** — still raw `HTML_BLOCK_CONTENT` TEXT tokens; projector reparses them. |
| 2 | `<span>` inline lift (INLINE_HTML_SPAN) | **Wrapper retag landed** (2026-05-08). Inner inlines mostly trivial (no recursive reparse needed). |
| 3 | Sectioning + verbatim corpus pin; `eitherBlockOrInline` lift | **Conformance landed** — non-void (2026-05-09); void (`<embed>`/`<area>`/`<source>`/`<track>`) (2026-05-10). Implementation leans on projector-side `inline_pending` tracking + byte walker; CST still opaque for split/matched-pair shapes. |
| 4 | Comments, PIs, declarations, CDATA projection | **Conformance landed** (2026-05-08); type-4 CM lowercase still gappy. CST opaque (these constructs project as RawBlock / RawInline). |
| 5 | `markdown_in_html_blocks` interaction edge cases | **Conformance landed** — depth-aware nested div, Plain/Para promotion, refs inheritance, **projector-level splitter** (`split_html_block_by_tags` byte walker + `parse_pandoc_blocks` recursive reparse), outer-matched-pair-abandons-on-void-interior. **The structural CST lift was deferred** — Phase 5's mechanism is the projector reparsing bytes, not the parser emitting structure. |
| 6 (new) | Lift inner HTML block content into structural CST children — `HTML_BLOCK_DIV` / `HTML_BLOCK` get `PARAGRAPH` / `LIST` / etc. as direct children; projector byte walkers become vestigial; `PARAGRAPH→PLAIN` retag at adjacent-HTML-block boundary. | **All non-bq + bq shapes lifted for `<div>` and non-div Pandoc strict-block tags as of 2026-05-12.** Shapes covered: clean multi-line, open-trailing, butted-close, indented-close, same-line, empty / blank-only, multi-line open (clean and trailing). Inline-block matched-pair abandons when body begins with a void block tag (Plain via OnlyIfLast). Bq via three discriminator gates (`bq_clean_lift`, `same_line_bq_lift_tag`, `bq_messy_lift_tag`) — see "Three bq lift gates" trap. Dispatcher's `HTML_BLOCK_DIV` retag gate uses `pandoc_html_open_tag_closes` AND requires `is_closing: false`. Vestigial `<div>` byte walkers pruned 2026-05-11. **As of 2026-05-12** same-line / fully-contained HTML blocks lift inside list items (`ListItemBuffer::emit_as_block` reparse + graft path); formatter's `format_list_item` gets a `HTML_BLOCK / HTML_BLOCK_DIV` arm to emit the marker for these. **As of 2026-05-13** multi-line HTML blocks also lift inside list items for non-div strict-block + inline-block + verbatim matched-pair tags via a close-form dispatcher gate (`BlockContext::list_item_unclosed_html_block_tag` + `ListItemBuffer::unclosed_pandoc_matched_pair_tag`). `<div>` multi-line lifts structurally too but inner body is `Plain` instead of pandoc's `Para` (indent-normalization gap — see "List-item indent normalization" trap). `<pre>` content retains list-item leading indent (same root cause). **Pass count history: 105 → 176** (current). Open shape gaps tracked in latest session's "Suggested next sub-targets". |

--------------------------------------------------------------------------------

## Latest session — 2026-05-13 (multi-line list-item HTML lift via close-form dispatcher gate)

Top-ranked sub-target from previous session: multi-line HTML
block as list-item content (`- <div>\n  body\n  </div>`,
`- <section>\n  hello\n  </section>`, etc. emitting
`Plain[RawInline open, body, RawInline close]` instead of
structural lift).

Implementation took option (a) from the previous session's
ranked fixes: suppress the close-form dispatch when an unclosed
matched-pair open is in the parent LIST_ITEM buffer. The
dispatcher's `HtmlBlockParser::detect_prepared` now returns
`None` for `BlockTag { is_closing: true, tag_name }` when
`ctx.list_item_unclosed_html_block_tag.as_deref() ==
Some(tag_name.to_lowercase().as_str())`. The buffer then
accumulates the full `<open>...</close>` text and
`try_emit_html_block_lift` (added previous session) reparses
and grafts the lifted HTML_BLOCK / HTML_BLOCK_DIV as a direct
LIST_ITEM child — bypassing the default PLAIN/PARAGRAPH wrap.

Probed corpus shapes:
- `- <section>\n  hello\n  </section>` → `RawBlock + Plain +
  RawBlock` ✓ (strict-block matched-pair).
- `- <article>\n  hello\n  </article>` → same ✓.
- `- <video src="x">\n  hello\n  </video>` → same ✓
  (inline-block matched-pair).
- `- <iframe>\n  hello\n  </iframe>` → same ✓.
- `- <span id="x">body</span>` → `Plain [Span ...]` ✓ (was
  already correct before, but now pinned in corpus).

Known divergences kept opaque (indent normalization — see
"List-item indent normalization gap" trap):
- `- <div>\n  body\n  </div>` → `Div [Plain [body]]` instead
  of pandoc's `Div [Para [body]]`. Structural lift succeeded;
  inner body is Plain because reparse sees the 2-space leading.
- `- <pre>\n  foo\n  </pre>` → single RawBlock with content
  `<pre>\n  foo\n  </pre>` (extra 2-space leading) instead of
  pandoc's `<pre>\nfoo\n</pre>`.

Conformance: html 171 → 176, total 364 → 369 (+5). Parser-crate
380 → 382 (added 2 paired fixtures).

### What landed

- `parser/blocks/html_blocks.rs`: `count_tag_balance` and
  `is_pandoc_lift_eligible_block_tag` promoted to `pub(crate)`;
  new `pub(crate) fn is_pandoc_matched_pair_tag` covers
  strict-block, inline-block, and verbatim tags (excluding
  void).
- `parser/utils/list_item_buffer.rs`: added
  `pub(crate) fn unclosed_pandoc_matched_pair_tag(config)` —
  Pandoc-gated walk over buffer text segments returning the
  tag name when opens > closes.
- `parser/block_dispatcher.rs`: new
  `BlockContext::list_item_unclosed_html_block_tag:
  Option<String>` field; `HtmlBlockParser::detect_prepared`
  short-circuits returning `None` when block_type is a close
  form matching the field.
- `parser/core.rs`: new helper
  `Parser::list_item_unclosed_html_block_tag()` populates the
  field in all three `BlockContext { ... }` construction sites
  in `core.rs` (parse_line, line 2549, line 2728).
- Test sites: 7 ad-hoc `BlockContext` constructions in
  `parser/blocks/tests/blockquotes.rs` updated.
- Parser fixtures
  `list_item_html_section_multiline_{pandoc,commonmark}` pin
  paired CST shapes: Pandoc lifts `<section>...</section>` to
  `HTML_BLOCK[HTML_BLOCK_TAG + PLAIN + HTML_BLOCK_TAG]`;
  CommonMark keeps inline-HTML in PLAIN with close as sibling.
- Formatter golden `list_item_html_section_multiline` pins
  idempotent round-trip of `- <section>\n  hello\n  </section>`.
- Corpus 0365 – 0369 pin pandoc-native for
  `<section>` / `<article>` / `<video>` / `<iframe>` multi-line
  in list + `<span>` inline in list.

### Files in committable diff

- `crates/panache-parser/src/parser/blocks/html_blocks.rs`
- `crates/panache-parser/src/parser/utils/list_item_buffer.rs`
- `crates/panache-parser/src/parser/block_dispatcher.rs`
- `crates/panache-parser/src/parser/core.rs`
- `crates/panache-parser/src/parser/blocks/tests/blockquotes.rs`
- `crates/panache-parser/tests/fixtures/{cases,pandoc-conformance/corpus}/`
  + snapshots + `golden_parser_cases.rs`
- `tests/fixtures/cases/list_item_html_section_multiline/` +
  `tests/golden_cases.rs`
- `crates/panache-parser/tests/pandoc/{allowlist.txt,report.txt}`
  + `docs/development/pandoc-report.json`

### Suggested next sub-targets

1. **List-item indent normalization for div / pre body.**
   Multi-line `<div>` in list lifts structurally but inner
   body is `Plain` instead of pandoc's `Para`. Same root
   cause for `<pre>` content keeping the list-item leading
   indent. Fix: thread `content_col` through `emit_as_block`,
   pre-strip indent on continuation lines before reparse,
   re-inject the stripped bytes as `WHITESPACE` tokens during
   `graft_node` (per-line, after each `NEWLINE` token) to
   preserve losslessness. Mid-complexity; touches buffer
   call sites in core.rs and the graft loop in
   list_item_buffer.rs.
2. **Comment + trailing-text split.** `<!-- comment --> body`
   at top level or in list emits a single `RawBlock` with
   the whole content. Pandoc splits into `RawBlock <!-- comment -->`
   + `Para body`. Parser shape is `HTML_BLOCK[TEXT(...)]`;
   projector early-returns when content starts with `<!--`
   (line 1296 of pandoc_ast.rs). Fix: detect `-->` end mid-
   content and split off any trailing non-WS bytes as
   `Para` / `Plain`. Projector-side; would unlock several
   gap cases.
3. **Audit verbatim-tag-in-list shapes more broadly.**
   `<style>`, `<script>`, `<textarea>` multi-line in list
   probably hit the same indent gap as `<pre>`. Probe + add
   corpus cases as the indent fix lands.
4. **`<span>` (and other inline-block) lift inside paragraph
   text, mid-line.** Currently works for fresh-block list-
   item content. Worth probing
   `text <span id="x">body</span> more text` mid-paragraph
   to see if anchor resolution / projection still match
   pandoc.

### New traps

Folded into Persistent traps:
- "Close-form dispatcher gate" — added to "List-item HTML
  structural lift" section.
- "List-item indent normalization gap" — added to same
  section, documents the Plain/Para and pre-indent
  divergences.

--------------------------------------------------------------------------------

## Earlier sessions (compact log)

Newest first. One line per session: date — phase/sub-target — pass
count delta — root cause / lever.

- 2026-05-12 — Phase 6 — list-item-as-sole-content same-line HTML lift (`- <div>foo</div>`, `- <!-- comment -->`, `- <pre>foo</pre>`, `- <p>foo</p>`) — html 167 → 171 — `ListItemBuffer::try_emit_html_block_lift` reparses + grafts when single top-level HTML_BLOCK / HTML_BLOCK_DIV consumes all buffer bytes; formatter `HTML_BLOCK | HTML_BLOCK_DIV` arm emits LIST_MARKER to avoid silent drop.
- 2026-05-12 — Phase 6 — close-line whitespace-only `leading` routes to close `HTML_BLOCK_TAG` indent — html 166 → 167 — strict-block / div lift site classifies whitespace-only `leading` and emits as `WHITESPACE` inside close tag; demote policy unchanged.
- 2026-05-12 — Phase 6 — projector strip leading 1-3 space indent on open/close `HTML_BLOCK_TAG` non-attrs branch — html 165 → 166 — `open_tag_raw_block_text` skips leading `WHITESPACE` when accumulator empty; corpus 0359 pins.
- 2026-05-12 — Phase 6 fix — `HTML_BLOCK_DIV` retag wrongly fired for standalone `</div>` — html 164 → 165 — dispatcher retag gate destructures `is_closing: false` in the `BlockTag` match arm; corpus 0358 pins.
- 2026-05-12 — Phase 6 — multi-line open + trailing-on-close-line structural lift — html 161 → 164 — `emit_multiline_open_tag_with_attrs` gains `lift_trailing` + `pre_content` args; `bq_messy_lift_tag` drops `multiline_open_end.is_none()` clause; dispatcher retag gate switches `_cleanly` → `pandoc_html_open_tag_closes` (the `_cleanly` helper removed).
- 2026-05-12 — Phase 6 — multi-line open in bq structural lift + bq-panic dispatcher gate + formatter goldens for bq messy shapes — html 159 → 161 — `find_multiline_open_end` accepts `bq_depth`; `emit_multiline_open_tag_with_attrs/_simple` take `bq_depth` and re-inject bq prefix tokens past line 0; `bq_lift_tag` drops `multiline_open_end.is_none()`.
- 2026-05-11 — Phase 6 bq lift arc (Fix #5 clean + HTML_ATTRS-in-bq, Fix #7 same-line, Fix #8 messy) + `<div>` byte-walker prune in `pandoc_ast.rs` (~170 net lines) — html stable 159 — three discriminator gates (`bq_clean_lift`, `same_line_bq_lift_tag`, `bq_messy_lift_tag`), `BqPrefixState` re-injection, `inline_block_void_interior_abandons`, `bq_strict_attr_emit_tag_name`, `open_tag_raw_block_text` bq-prefix strip; `html_div_block` `debug_assert!`s on unlifted HTML_BLOCK_DIV.
- 2026-05-11 — Phase 6 / Fix #4 non-div strict-block shape sweep + multi-line open-tag lift — html 142 → 159 — `is_pandoc_lift_eligible_block_tag`, `html_block_has_structural_lift`, `LastParaDemote::{OnlyIfLast,SkipTrailingBlanks,Never}`, `parse_with_refdefs` graft, `emit_multiline_open_tag_with_attrs`, `open_tag_raw_block_text` canonicalizer.
- 2026-05-10 → 2026-05-11 — Phase 6 cannot_interrupt + Fix #1/#2 — html 132 → 142 — PARAGRAPH→PLAIN retag at YesCanInterrupt; `is_closing` field; `is_math_tex_script_open`; pandoc `isInlineTag` (issue #10643).
- 2026-05-10 — Strict-block/verbatim closing-form lift, multi-line void open-tag, incomplete-open recursion fix, Phase 3 void `eitherBlockOrInline` — html 105 → 132 — `closes_at_open_tag`, `pandoc_html_open_tag_closes` gate, `PANDOC_VOID_BLOCK_TAGS`.
- 2026-05-08 → 2026-05-09 — Phases 1-5 seed + projector-side lift (issue #263 closed; non-void eitherBlockOrInline; HTML5 sectioning; `<DIV>` losslessness; Plain/Para; multi-line attrs; refs inheritance) — html 0 → 105 — `HTML_BLOCK_DIV`/`INLINE_HTML_SPAN` retag, `HTML_ATTRS` tokenization, sectioning/verbatim corpus pin, depth-aware nested `<div>`, projector `inline_pending` + parser `cannot_interrupt`, CM/Pandoc blockHtmlTags split, `build_refs_ctx_inherited`.
