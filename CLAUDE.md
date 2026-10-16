# CLAUDE.md: writing RFCs in `docs/wip`

This directory holds draft RFCs for the binpatcher project. The current one is
`rfc-mend-language.md`, which defines **Mend**, a statically typed DSL for
patching binary game-asset files. This file collects what was learned while
writing it, so later RFCs (starting with the Mend instruction-set RFC) stay
consistent with it.

## 0. Writing style (mandatory)

- **Be concise and succinct.** Every sentence must carry information. Cut
  filler, preamble, recaps and hedging.
- **Be direct.** State the rule, then the reason in one sentence if it isn't
  obvious. Don't beat around the bush.
- **No bikeshedding.** Pick an option and move on. Put rejected options in
  *Alternatives considered* as one table row each. Put real open issues in
  *Unresolved questions*. Don't debate in the body.
- **Write documentation-style English:** neutral, declarative, present tense
  ("`write` overwrites bytes at `at`"), as in a reference manual. No
  marketing language, no first person, no conversational asides.
- Prefer a table or a list over prose whenever the content is a set of
  parallel items.

## 1. How the author works

- **Design from the brief when told to.** If the request says "do not read
  this directory" or "design from scratch", read nothing in the project except
  what the prompt names, and don't let existing drafts or code shape the
  design. (The first Mend attempt was interrupted for opening an older RFC in
  `docs/`.) External references the prompt names, such as a GitHub repo,
  should still be studied.
- **Clone reference repos into the scratchpad**, never into the project. For
  a language reference, read its `grammar.ebnf`, `ast.rs`, the parser's
  handling of loops and conditions, and the tests. The tests show the real
  syntax, which can differ from the EBNF (sophia's grammar file says
  `for expr { }`, but the parser and tests use `for i : 0..10 { }`).
- **The prompt is the requirements list.** Each feature the author lists
  ("endianness", "assertions", "lambdas", "explicit mutable bindings", …) must
  map to a clearly findable section. Summarise that mapping in the final reply.
- **Respect scope boundaries.** When the author says part of the design
  belongs in another RFC (for example the JIT instruction set), don't design
  it. Do write a *contract* section that says what this RFC hands to the
  other one and what it expects back.
- **The author's domain:** binary assets for a game the author is making.
  These are data files, not machine code, so there is no security model,
  sandbox, or executable-format knowledge to cover. Say so once, in
  Motivation or Non-goals.
- Output is a Markdown file in this directory, written locally. Don't publish
  it as an artifact or a doc unless asked.

## 2. RFC skeleton

Use this order. Number the sections and refer to them as `§N.M` throughout.

1. **Header table:** Status (Draft), Created (absolute date), Authors
   (Mário Feroldi), Scope, Out of scope, plus project-specific fields such as
   the file extension.
2. **Summary:** what it is, the main features as a list, the lineage, and
   the execution model in one paragraph.
3. **Motivation:** the concrete jobs it serves, and the pain of the current
   approach.
4. **Goals / Non-goals:** numbered goals. Non-goals name what is deferred and
   where to (another RFC, or "Future possibilities").
5. **A taste of X:** one or two realistic examples *before* any formal spec.
6. **Lineage / prior art:** a table of borrowed versus changed constructs,
   with the reason for each change.
7. **Specification sections:** lexical rules, types, declarations, expressions,
   then the domain features, errors, standard library, and dynamic semantics.
8. **Compilation or implementation model**, including the **contract with
   companion RFCs**.
9. **Diagnostics:** rustc-style samples of error and warning messages.
10. **Complete grammar (EBNF)** in the reference language's EBNF style, with
    numbered **disambiguation notes**.
11. **Future possibilities**, **Alternatives considered** (a table: option,
    why rejected), and **Unresolved questions** (labelled `Q1`, `Q2`, …, each
    with the tradeoff stated).
12. **Appendix: worked examples** that use features together.

Style: put information in tables (types, operators, precedence, error kinds,
methods with signatures). Write each rule as a short numbered or bulleted
statement. Every "why" goes in a sentence next to the rule, or in
Alternatives.

## 3. Language-design principles used in Mend

Reuse these in follow-up language RFCs unless the brief says otherwise.

- **Exact semantics over hand-waving.** State evaluation order (left to
  right; for assignment, the right-hand side first), overflow behaviour
  (checked by default, with explicit wrapping operators), the exact rule for
  shifts, cast rules, and what every operation does at a boundary (an
  inclusive range ending at `MAX`, a range with `start > end`, a write past
  the end of the file).
- **List every runtime error kind** in one table, with what raises it. Give
  exit codes.
- **Leave the backend no type-directed decisions.** The front end produces a
  monomorphic, fully typed, desugared core in which every check is explicit
  and every node carries a source span.
- **Resolve syntactic ambiguities in the spec**, not in the implementation:
  - mandatory lambda parameter types, so `(x)` is never a lambda;
  - no lambda directly in `if`/`for`/`match` heads (as with struct literals in
    Rust);
  - `.name[` is type arguments, because methods aren't first-class values;
  - `for pat : expr` versus `for cond` uses a cover grammar, reinterpreted
    when a `:` follows;
  - a label is `'ident` not followed by `'`;
  - a float needs a digit after the `.`, so `0..10` is a range;
  - integer suffixes start with `u` or `i`, which aren't hex digits.
- **Give each contested choice a single answer and say why.** Examples:
  byte order is a runtime `Endian` *value* rather than part of the type
  (`u32be` was rejected); offsets don't follow edits, with `batch` snapshots,
  edit-aware match iterators and back-to-front edits as the alternatives;
  lambdas capture by value.
- **Mutability belongs to bindings, not types.** Implicit ambient state
  (the Mend content) is always writable, and that is stated explicitly.
- **Stay deterministic:** no clock, no randomness, no hash-order iteration,
  so `--dry-run` output can be diffed.
- **Fail safe:** output is written only on success, atomically, through a
  temp file and rename.
- **Prefer declarative forms** (views, edit statements) for the common case,
  and keep functions and statements for the rest. Ambient state that every
  program has should be implicit, not passed around as a handle.

## 4. Mend facts that later RFCs must stay consistent with

- **Execution model:** one input file per run:
  `mend <program.mend> <input> [-o <output>] [-D name=value]…`. The default output is
  `{FILENAME}.out`, which is `<input>` with `.out` appended. The input is
  loaded implicitly as the **content**. There are no file handles and no
  `open`/`save`. The output is written atomically, only on success.
- **`main :: ()` is required.** Run order: top-level statements, then load
  the input, then `main`, then write the output. **Content operations** (typed
  access, views, struct fields, edit statements, `batch`, a dynamic
  `endian`, content functions such as `len()`/`find`) are valid only in
  `main` or in a *content function*. The compiler infers content functions
  over the call graph, and they are callable or nameable only from `main` or
  other content functions. Top-level statements (`assert`, bindings,
  constant `endian`) cannot touch the content. Top-level bindings are
  visible in `main`.
- **Declarative core:** typed content access `T[at]`, `T[r]` and
  `T.be[at]`; views `name :: T[at];`, whose offset is re-evaluated on every
  use; and edit statements `replace T with v`, `insert v at/before/after T`
  and `delete T`, where a target is a range or a pattern with optional
  `first`/`last`/`within`. A pattern target with zero matches raises
  `NotFound`. Content functions (`len()`, `find(P)`, …) take no file or
  buffer argument.
- **Structs:** `Name :: struct { field: T[at], … }` names a record in the
  file. Each field is a view, read as `Name.field` and assigned with
  `Name.field = v`. Field offsets are absolute and may refer to sibling
  fields by bare name. A struct is not a type and has no instances; `Name`
  alone is not an expression.
- **Modules:** one `.mend` file per module. `use a::b::file [as alias];`
  makes items available as `file::NAME` or `alias::NAME`. Item imports are
  `use a::b::file::{A, B as C};` and `use a::b::file::*;`. Paths resolve to
  `a/b/file.mend`, searched from the root module's directory, then `-I`
  dirs. A non-root module holds only declarations, and all of them are
  exported: there are no private items, and no `self::`/`super::` relative
  paths (both decided by the author). `use` comes first in a module; that is what lets the parser tell
  a qualified `ident::` apart from a declaration `ident ::`.
- **Syntax** (from github.com/feroldi/sophia): `name :: expr` declarations;
  `N : T : v;` typed constants; `x := e` and `mut x := e` bindings; `x: T = e`;
  functions and lambdas are both `(a: T) -> R { … }`, with `()` as the return
  type when `-> R` is omitted; `if c { } else if d { } else { }`; `for {}`,
  `for c {}`, `for x : iter {}`, `for some(x) := e {}`; `for … else`;
  `'label:`; `break [label] [expr]`; a block's value is its last expression.
- **Parameters:** `name :: param T [= default];`, root module only, set with
  `-D`. They are runtime values bound before top-level statements, not
  constants, so the compiled form never depends on arguments.
- **Strings:** only `f"…"` interpolates. Plain strings treat `{ }` literally.
- **Errors are not recoverable.** No `try`/`catch`/`Result`; expected
  failures use `?T` forms (`find`, `T.get(at)`, `checked_*`, `parse`, …).
- **Types:** Rust primitives, with `usize`/`isize` as aliases of `u64`/`i64`. `[T]` is
  an owned sequence with value semantics (copy-on-write), not a borrowed
  slice. `?T`, `!`, `str`, and the built-in types `Pat`, `Match[P]`,
  `Range[T]` and `Endian`. Built-in type arguments go in `[ ]`.
- **Precedence:** Rust's order, plus `??` just above comparisons. `++` means
  concatenation. `+% -% *%` are wrapping operators.
- **Byte-order precedence:** an explicit `T.le`/`T.be` (or a `to_bytes(e)`
  argument), then the current byte order set by the `endian e;` statement
  (which can be a runtime value), then `le`.
- **Locations:** offsets (`usize`), ranges (`a..b`, `a..=b`, `a..+n`, and
  open forms only where a receiver supplies the bound), and pattern literals
  `p#/…/#` compiled at compile time: leftmost match, ordered alternatives,
  shortest-first gaps, and no pattern that can match zero bytes.
- **Edit semantics:** the marker-adjustment table in §14.4 and the conflict
  rules for `batch { … }` in §14.3. The batch region is lowered as explicit
  `batch_begin`/`batch_end` instructions, with `batch_end` on every exit
  except error paths.
- **Instruction-set RFC contract:** §18.3 of the Mend RFC. Re-read it
  before writing the companion RFC. That RFC owns the register/stack model,
  encoding, calling convention, closure representation, and whether "JIT"
  means native code or threaded code.

## 5. Self-review checklist before finishing an RFC

The Mend draft needed fixes for each of these, so check them every time:

- [ ] Every example type-checks under the RFC's own rules. Watch for
      `if` branches of different array lengths (`[0u8]` vs `[]`), which need
      an annotation that coerces both to `[T]`.
- [ ] Examples use declared constants where the type system requires them
      (`Match[P]` needs `P` to be a named top-level pattern, not an inline
      literal).
- [ ] Tables don't contradict each other (an error kind listed for an
      operation that actually raises a different kind).
- [ ] Optional or named parameters are marked as optional in signature
      tables.
- [ ] Every operator in the precedence table is in the typing table and in
      the EBNF, and the other way round.
- [ ] Byte sizes in examples add up (for example, a tuple meant to fill an
      8-byte slot).
- [ ] Every keyword in examples is listed as a keyword, or explicitly
      described as a contextual or prelude name.
- [ ] The final reply maps each requested feature to its section and points
      out the open questions most worth the author's decision.
