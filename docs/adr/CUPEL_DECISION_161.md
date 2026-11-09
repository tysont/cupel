# CUPEL_DECISION_161: The JavaScript and TypeScript grammars are vendored, patched, and regenerated from their upstream tags

**Date:** 2026-11-09 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 1

## Context

The first real-repository trial (the owner's `../molten`, a TypeScript monorepo) reported `lang.parse-error` for 12 of 1,077 files. The TypeScript compiler, Vite, and Molten's CI all accept every one of them. Three constructs cause the errors:

1. A bare `&` in JSX text (`<p>SMS & Voice</p>`). The external scanner ends `jsx_text` at `&`, and the grammar accepts a `&` there only as the start of a character reference (`&amp;`). 11 files fail this way.
2. A `&` followed by a letter in a JSX attribute string (`href="…?a=1&family=Inter"`). The string fragment allows `&` only before a character that cannot start a reference, and a reference needs its `;`. Several of the 11 files fail on this as well as on 1.
3. `unique` as an identifier before `.` in plain TypeScript (`i < unique.length`; Molten's `secret-store.ts`). `unique symbol` is two keyword tokens, so after `i <`, a state that also allows type arguments, the lexer takes `unique` as the keyword and the expression branch dies.

Constructs 1 and 2 come from tree-sitter-javascript's JSX rules, which tree-sitter-typescript builds on, so `.js` and `.jsx` fail on them too. Upstream:
- tree-sitter-javascript issue #366 ("Ampersand breaks jsx_text") is open.
- tree-sitter-typescript has had no release since 0.23.2 (2024-11).
- crates.io's latest versions are those Cupel uses, tree-sitter-typescript 0.23.2 and tree-sitter-javascript 0.25.0.

A parse error is not cosmetic. Each one is a warning on the file, and tree-sitter's recovery can drop or misplace facts near the error.

## Options

- (a) Wait for upstream. That leaves false findings on common React code with no date for a fix.
- (b) Repair the source before parsing: replace the offending bytes with ones of the same length, then reparse. It keeps offsets, but it changes the text facts read (identifiers, literals), and it is a workaround with no root cause fixed.
- (c) Vendor the two grammar crates with the rules patched, and regenerate their parsers with the tree-sitter CLI each release used.

Chosen: (c).

## Decision

1. **The patches** live in `vendor/patches/<grammar>-<tag>.patch`. They are applied to the upstream tags v0.23.1 and v0.25.0 of tree-sitter-javascript, and v0.23.2 of tree-sitter-typescript.
   - JSX text: `_jsx_child` also accepts a lone `&`, aliased to `jsx_text`. A full reference still lexes as `html_character_reference`, the longer match.
   - JSX strings: each quoted form also accepts a lone `&`, aliased to `string_fragment`.
   - `unique symbol` is one token (`unique`, whitespace, `symbol`), so `unique` alone is an identifier.
2. **The generated crates** are `vendor/tree-sitter-typescript` and `vendor/tree-sitter-javascript`, wired in through the workspace's `[patch.crates-io]` and excluded from the workspace.
   - Each is the upstream crate's `include` set at its tag, with its `LICENSE` (MIT), and its parsers are regenerated after the patch.
   - The tree-sitter CLI versions are those that generated the published parsers: 0.24.4 for TypeScript, 0.25.8 for JavaScript.
   - Regenerating the unpatched tags with those CLIs reproduces the published `parser.c` files byte for byte, so the patch is the only difference.
3. **`scripts/grammars.sh` rebuilds both** from the tags and patches (it needs git, node, and the network); `git diff vendor` then shows any change. To take a new upstream release:
   - bump the tag and CLI version in the script;
   - rebase the patch;
   - rerun the script;
   - rerun `crates/cupel-lang/tests/ecma_parse.rs` and the ecma fact goldens.

   When upstream fixes a construct, its patch hunk is dropped. When upstream fixes all three, the vendored crates go.
4. **Checks.**
   - Upstream's own corpora pass on the patched grammars: TypeScript and TSX 112 of 112, JavaScript 0.23.1 119 of 119, JavaScript 0.25.0 123 of 123.
   - `crates/cupel-lang/tests/ecma_parse.rs` holds each construct beside the ones that must keep their meaning (`&amp;`, `&#169;`, `&copy;`, `unique symbol`).
   - Cupel's own dogfood leaves `vendor/` out of scope: it is third-party code, not Cupel's.

## Consequences

- Molten's 12 parse errors are 0. No fact golden changed, so code without these constructs parses as before.
- The repository carries about 21 MB of generated C (two TypeScript parsers and one JavaScript parser), marked `linguist-generated` so diffs and language statistics leave it out.
- `[patch.crates-io]` applies to builds from this workspace: `make`, the release workflow, `cargo install --git`, and `cargo binstall --git`. A crates.io build of the library crates would not apply it. That route is already deferred (ADR-146), and publishing it will need the vendored crates published under Cupel's own names, or upstream fixes.
- A grammar upgrade is now a script run and a patch rebase, not a version bump.
