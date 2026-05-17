# Footnote refs inside footnote definition bodies

Outer[^a] reference in a paragraph (this should not flag).

[^a]: First definition body has a [^b] ref (line 5: flag).

[^b]: Second body has **bold [^c] inside strong** and ~~strike [^d]~~
    plus a [link [^e]](u) — three flags on lines 7--8.

[^c]: Body has `[^f]` in code (no flag — code spans skip the scan).

[^d]: Body has a [@key] citation (no flag — citation, not footnote ref).

    > Nested blockquote in def has [^g] ref (line 13: flag).

    - List item in def has [^h] ref (line 15: flag).

Outer[^c] [@key] [^d] in a sibling paragraph (no flags — outside any def).

[^e]: E.

[^f]: F.

[^g]: G.

[^h]: H.
