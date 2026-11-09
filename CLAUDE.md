# fsexp

A configurable parser for Lisp-like languages: s-expressions plus
multi-character enclosers, operators that take arguments on the left and/or
right, and per-context rules (strings, comments). Its main consumer is the
easl shader language (`../easl`, see `src/parse.rs` there for `EaslSyntax`).
easl depends on this crate through its **GitHub URL**, so easl only sees
changes here once they're pushed (or after a local `[patch]`/path override in
easl's `Cargo.toml`).

## Build & test

```bash
cargo test                 # all tests
cargo test <name>          # one test
cargo clippy --all-targets # the type-complexity warnings are already there
```

Formatting: `rustfmt.toml` sets 2-space indent and 80-column width. The code
uses edition 2024 features, including `if let ... && let ...` chains.

## Layout

- `src/syntax.rs`: the traits that define a grammar. `Syntax` maps
  `ContextId`s to `Context`s, where a `Context` holds the enclosers,
  operators, whitespace and escape char that are active in it. `Syntax` also
  says which context an encloser or operator switches to for its children,
  and lists `reserved_tokens`.
- `src/parse.rs`: `Parse`, the single-pass scanner that reads one top-level
  form. The main loop is `Parse::complete`, which relies on local macros
  (`finish_terminal!`, `skip_bytes!`, `close_ast_and_maybe_return!`, ...).
- `src/parser.rs`: `Parser`, the public driver. It runs `Parse` repeatedly
  and keeps enough top-level forms buffered for operators that take left
  arguments (`Context::lookahead`).
- `src/document.rs`: `Document`, the parsed trees plus parse errors and a
  newline index. It has helpers for cursors and selections and for describing
  errors. It does **not** own the source text, so methods that need the text
  take it as an argument.
- `src/ast.rs`: the generic `Ast<L, I>`, plus the aliases `SyntaxTree`
  (without positions) and `Sexp` (untagged, used heavily in tests).
- `src/str_tagged.rs`: `StringTaggedSyntax`, which builds a grammar from
  string descriptions. Most tests use it: `contextless_from_descriptions` for
  quick grammars, `.with_reserved_tokens(..)` for reserved tokens.
- `src/examples/`: hand-written `Syntax` impls (`sexp`, `sexp_with_strings`,
  `psuedo_clj`).
- `src/diff.rs`: structural diffs over ASTs (insert/delete/replace/snippets).
- `src/formatting.rs`: basic formatters (see `todo.md` for known problems).

## Parser invariants

- **All positions are byte offsets** into the source text. The scanner walks
  `char`s. Every span must lie on char boundaries.
- **Parsing never panics**, whatever the input. Malformed input still produces
  trees, and the problems are reported as `ParseError`s. The fuzz tests in
  `src/robustness_tests.rs` enforce this. A change to `parse.rs` should keep
  them passing, and a new edge case should get a regression test.
- Markers match **longest-first**, whatever order the `Context` lists them in.
  When an opener and an operator are the same length, the opener wins.
- Reserved tokens split text only in contexts that have enclosers or
  operators, so string and plain-comment contents are left alone.
- A closer, whether it matches or not, first closes every operator still
  waiting on right arguments (reporting `OperatorMissingRightArgument`), then
  the innermost open encloser. If no encloser is open, a stray closer is
  reported and skipped.
- Children in a comment context (`ContextId::is_comment`) don't count toward
  an operator's left or right argument count.

## Tests

- `src/lib.rs` → `mod core_tests`: behavioural tests, mostly asserting on
  `read_next_as_sexp()` / `read_all_as_sexps()` output against
  `Sexp::leaf`/`Sexp::inner` trees plus exact `ParseError` lists.
- `src/robustness_tests.rs`: deterministic fuzzing (xorshift) with an
  easl-shaped syntax and `CljSyntax`, plus unicode/CRLF regression tests.
