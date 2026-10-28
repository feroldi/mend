# RFC: Mend, a Language for Patching Binary Files

| Field          | Value                                                       |
|----------------|-------------------------------------------------------------|
| Status         | Draft                                                       |
| Created        | 2026-10-16                                                  |
| Authors        | Mário Feroldi                                               |
| Scope          | Syntax, static semantics, dynamic semantics, standard library, CLI |
| Out of scope   | The JIT instruction set (companion RFC)                     |
| File extension | `.mend`                                                     |

## 1. Summary

Mend is a statically typed, declarative language for patching one binary
file of any format. A Mend program runs against an **input file** named on the
command line. The file's contents, called the **content**, are available
implicitly to `main` and the functions it calls, which read, check and edit
them. When the program succeeds, the edited content is written to an
**output file**.

```
mend patch.mend data/archive.bin -o data/archive.patched.bin
```

Features:

- **Types** equivalent to Rust's primitive types, plus `Opt[T]`, `Pat`,
  `Match[P]`, `Range[T]` and `Endian`.
- **Typed content access** with no read calls: `u32[0x10]` is the `u32`
  stored at offset `0x10`.
- **Views**, which are named, typed locations: `version :: u16[0x04];`.
- **Structs**, which name a record in the file as a group of views:
  `Header.version = 3;`.
- **Enums**, which name integer constants of one type:
  `Kind :: enum u16 { DATA = 0, META = 4 };`, used as `Kind.META`.
- **Edit statements**: `replace … with …`, `insert … at/before/after …`,
  `delete …`, and assignment through views.
- **Locations** given as offsets, ranges (`a..+n`), or byte patterns with
  wildcards, masks, gaps, alternation and named captures.
- **Byte order** set for the whole content (`endian big;`) or for one access
  (`u32.be[at]`).
- **`assert`**, **`if`**, **`match`** and **`for`** as expressions, with
  `batch` for transactional edits.
- **Functions and lambdas**, immutable bindings by default, and `mut` for
  bindings that can be reassigned.
- **Modules**: `use path::to::file [as name];`, `use …::{A, B};` and
  `use …::*;`.
- **A `main :: ()` entry point**. Any function can operate on the content,
  but only while `main` runs; the top level cannot (§5.3).
- **Parameters** supplied on the command line: `ID :: param u16 = 1;`
  with `-D ID=7` (§9.5).

The syntax follows [sophia](https://github.com/feroldi/sophia). Mend is a JIT
language: `mend` compiles the program, loads the input, runs, and writes the
output in one invocation. The companion RFC defines the instruction set; §18
defines the interface to it.

## 2. Motivation

Binary files of every kind (save files, archives, media containers,
firmware images, executables, database and network captures) need repeated,
mechanical edits: bumping version fields, recomputing checksums, changing
values across table records, replacing byte sequences, inserting or
stripping chunks, and migrating files between format versions. Ad-hoc
scripts repeat file handling, byte-order unpacking and offset bookkeeping,
and they fail late. Hex-editor edits cannot be reproduced.

Mend makes these edits short declarations over one file, checks types and
patterns at compile time, and writes nothing unless the whole program
succeeds. Mend treats every input as a byte sequence and knows no file
format; formats are described in Mend code (structs, enums, patterns,
modules). A Mend program is trusted like a shell script, so Mend has no
security model beyond the user's file-system access.

## 3. Goals and non-goals

**Goals**

1. Declarative description of locations, checks and edits over one file.
2. Static types with no implicit numeric conversions.
3. Explicit mutability for bindings.
4. Defined behaviour for every edge case: overflow, bounds, overlapping
   edits, and edits made during a scan.
5. A front end that hands the backend a monomorphic, fully typed,
   desugared program.

**Non-goals**

- The instruction set and its execution model (companion RFC).
- Working on more than one file per run.
- Built-in knowledge of specific formats: executable formats, instruction
  encodings, compression and archive containers. Mend patches their bytes;
  libraries written in Mend describe their layout.
- User generics, user-defined aggregate types, and
  recoverable errors (§15, §21).

## 4. Overview

```mend
endian little;

MAGIC :: b"FWIM";
DELAY_MS : u32 : 250;

Image :: struct {
    magic:      [u8; 4][0x00],
    version:    u16[0x04],
    checksum:   u32[0x0C],
    boot_delay: u32[if version >= 5 { 0x40 } else { 0x38 }],
}

assert DELAY_MS <= 9_999;                // runs before the input is read

main :: () {
    assert Image.magic == MAGIC, "not a firmware image";
    assert Image.version >= 3, f"unsupported image version {Image.version}";

    Image.boot_delay = DELAY_MS;
    Image.checksum = crc32(u8[0x10..]);
}
```

```mend
ENTRY :: p#/'RECD' {id: ?? ??} {flags: ??} 00/#;

is_reserved :: (id: u16) -> bool { id >= 0x00A0 && id < 0x00B0 }

main :: () {
    for m : ENTRY {
        if is_reserved(u16[m.id.start]) { u8[m.flags.start] = 0; }
    }

    replace p#/FF 00 FF/# with x"00 FF 00";
}
```

## 5. Execution model

### 5.1 Command line

```
mend <program.mend> <input> [-o <output>] [-D <name>=<value>]… [--dry-run] [--max-steps <n>]
mend --emit asm <program.mend> [-I <dir>]…
```

| Argument          | Meaning                                                                 |
|-------------------|-------------------------------------------------------------------------|
| `<program.mend>`  | Program to run. Required.                                               |
| `<input>`         | File to patch. Required.                                                |
| `-o <output>`     | Output path. Default: `{FILENAME}.out`, where `{FILENAME}` is `<input>` as given (`a/b.bin` → `a/b.bin.out`). |
| `-D <name>=<value>` | Set the parameter `name` (§9.5). May be repeated, once per name. |
| `--dry-run`       | Run, but print a summary and a hex diff of the changes instead of writing the output. |
| `--max-steps <n>` | Abort with `StepLimit` after `n` loop iterations plus calls. No limit by default. |
| `-I <dir>`        | Add a module search directory (§9.4). May be repeated.                  |
| `--emit asm`      | Compile, print the bytecode listing (bytecode RFC §21) to stdout, and exit. Runs no program code. The text form is unstable and may change with any `mend` version. |

### 5.2 Run sequence

1. Compile the program. A compile error stops the run before the input is
   read.
2. Bind the parameters from `-D` arguments and defaults (§9.5).
3. Evaluate the top-level statements in source order.
4. Read `<input>` fully into memory. This is the content.
5. Call `main`.
6. If `main` returns, write the content to `<output>` through a temporary
   file and a rename, which replaces any existing file atomically.
7. If any runtime error occurs, write nothing.

Notes:
- An invalid parameter or a failing top-level statement stops the run before
  the input is read.
- `return;` in `main` ends the run successfully.
- The input file is never modified, unless `-o` names it.
- The output is written even when the content is unchanged.

### 5.3 Program structure

A program is the root module plus every module it imports (§9.4). The root
module is a sequence of **declarations** and **top-level statements**, and
it must declare exactly one `main :: () { … }`.

**Declarations and statements**
- Declarations (`::`) are visible in the whole program in any order:
  constants, parameters, views, structs, enums and functions.
- Top-level statements run top to bottom, before `main`. Bindings made with
  `:=` are visible from the next statement on, and inside `main`.
- Other declarations may refer only to declarations, never to top-level
  bindings.

**`main`**
- `main` takes no parameters and returns `()`. It cannot be called or used
  as a value.

**Content operations** are typed access, uses of views and struct fields,
edit statements, `batch`, `endian` with a non-constant operand, and the
content functions (§16).

- Declaring a view or a struct is not a content operation. Using one is.
- A content operation is valid in the body of any function or lambda,
  `main` included.
- A content operation is invalid at the top level: in top-level statements,
  constants, parameter defaults and enum values.
- A function or lambda is a **content function** if its body contains a
  content operation, or calls or names a content function. The compiler
  infers this over the call graph.
- The top level cannot call or name a content function.

As a result, every content operation runs while `main` runs, after the input
is loaded. The top level can never touch the content.

Statement snippets in §9–§17 are assumed to be inside `main`, unless they
are declarations.

```mend
endian little;                      // ok: constant byte order
LIMIT :: 100u32;
assert LIMIT > 0;                   // ok: no content operation

timeout :: u32[0x40];               // ok: declaring a view
cap_timeout :: () { timeout = timeout.min(LIMIT); }   // content function

x := timeout;                       // error: content operation at the top level
cap_timeout();                      // error: content function called at the top level

main :: () {
    cap_timeout();                  // ok
}
```

| Exit code | Meaning                                  |
|-----------|------------------------------------------|
| 0         | success; output written                  |
| 1         | runtime error; nothing written           |
| 2         | compile error                            |
| 3         | I/O error reading the input or writing the output |
| 4         | usage error: invalid command-line arguments, including `-D` values |

## 6. Syntax lineage

| Construct          | sophia                                  | Mend                                             |
|--------------------|-----------------------------------------|--------------------------------------------------|
| Declaration        | `f :: () -> i32 { … }`                  | same; also constants, views and structs         |
| Binding            | `x := 1;`                               | same; `mut x := 1;`, `x: u8 = 1;`                |
| Function / lambda  | `(a: i32) -> i32 { … }`                 | same syntax for both                             |
| Conditional        | `if c { } else if d { } else { }`       | same; plus `if some(x) := e { }`                 |
| Loops              | `for { }`, `for c { }`, `for i : a..b { }` | same; any iterable after `:`; `else`; labels  |
| Break with value   | `break e`                               | same                                             |
| Block value        | last expression without `;`             | same                                             |

Additions: `return`, `match`, `use`, edit statements, typed content access, structs,
enums, `batch`. Bindings are statements, not expressions.

## 7. Lexical structure

- **Source:** UTF-8. Comments are `// …` and `/* … */`; block comments nest.
- **Identifiers:** `[A-Za-z_][A-Za-z0-9_]*`. A lone `_` is the wildcard
  pattern.
- **Reserved keywords:** `as assert batch break continue delete else endian
  enum false for if insert match mut none param replace return struct true
  use`, and the primitive type names.
- **Contextual keywords**, which are keywords only in specific positions:
  `with at before after first last within` (inside edit statements),
  `little big` (after `endian`), `le be` (after a type name and `.`), and
  `some` (in patterns).
- **Prelude names** (types, `le`, `be`, `some`, functions) can be shadowed.
  Shadowing one produces a warning.
- **Labels:** `'name`, when not followed by a closing `'`.

| Literal          | Examples                                   | Type                       |
|------------------|--------------------------------------------|----------------------------|
| Integer          | `42`, `0x7F`, `0b1010`, `0o17`, `1_000`     | inferred; defaults to `i32` |
| Suffixed integer | `42u8`, `0xFFu8`, `-1i64`, `16usize`        | the suffix type            |
| Float            | `1.5`, `2.0e-3`, `1.0f32`                   | inferred; defaults to `f64` |
| Bool / char      | `true`, `'a'`, `'\u{1F600}'`                | `bool` / `char`            |
| Byte char        | `b'a'`                                      | `u8`                       |
| String           | `"id = {id}"` (no interpolation)            | `str`                      |
| Format string    | `f"id = {id:#x}"`                           | `str`                      |
| Byte string      | `b"RIFF"`                                   | `[u8; N]`                  |
| Hex bytes        | `x"DE AD BE EF"`                            | `[u8; N]`                  |
| Pattern          | `p#/'HDR' ?? {ver: ??}/#`                     | `Pat` (§13.2)              |

Lexical rules:

- A float needs a digit after the `.`, so `0..10` is a range.
- An integer suffix starts with `u` or `i`, which are not hex digits.
- A `-` written directly before an integer literal folds into it, so
  `-128i8` is valid.
- Escapes: `\n \r \t \\ \0 \' \"`, `\xHH`, `\u{…}` (except in byte
  strings), and `\{ \}` (format strings only).
- Only format strings `f"…"` interpolate, with `{expr}` and `{expr:spec}`. The
  spec starts at the first `:` that is not inside parentheses or brackets. In
  plain and byte strings `{` and `}` are ordinary characters, because strings
  are often stored in binary files verbatim.
- A format string is not a literal pattern in `match`.
- A pattern literal is a separate token kind, not a string: it starts with
  `p#/` and ends at the first `/#` outside a text element (§13.2). It has no
  interpolation and no escapes outside text elements, may span lines, and may contain `//`
  comments that run to the end of the line.

## 8. Types

| Type                         | Description                                                    |
|------------------------------|----------------------------------------------------------------|
| `bool`                       | 1 byte when encoded: `0` or `1`                                |
| `char`                       | Unicode scalar value; encoded as `u32`                         |
| `u8 … u128`, `i8 … i128`     | integers; signed types are two's complement                    |
| `usize`, `isize`             | aliases of `u64` and `i64`; `usize` names offsets and lengths  |
| `f32`, `f64`                 | IEEE 754                                                       |
| `()`, `!`                    | unit; never (the type of `return`, `break`, `fail`)            |
| `str`                        | immutable UTF-8 string                                         |
| `[T; N]`                     | array; `N` is a compile-time `usize`                           |
| `[T]`                        | owned sequence with value semantics (copy-on-write)            |
| `(T1, …)`                    | tuple                                                          |
| `(T1, …) -> R`               | function type                                                  |
| `Opt[T]`                     | optional value, as Rust's `Option<T>`: `some(x)` or `none`     |
| `Range[T]`                   | half-open integer interval; `Range` means `Range[usize]`       |
| `Pat`                        | compiled byte pattern                                          |
| `Match[P]`, `Match`          | match of the constant pattern `P` (typed captures), or of an unknown pattern |
| `Endian`                     | values `le` and `be`                                           |

Type properties:

- **Equatable** (`==`, `!=`): all types except function types.
- **Ordered** (`<`, `>`, `<=`, `>=`): integers, floats, `char`, `bool`,
  `str`, and sequences of ordered types (compared lexicographically).
- **Encodable**: scalars, plus arrays, sequences and tuples of encodable
  types, plus `str` (encoded as UTF-8).
- **Decodable**: encodable types of fixed size, so `bool`, `char`,
  integers, floats and `[T; N]`.

Only built-in types take type arguments, written in `[ ]`. Structs (§12.3)
and enums (§9.6) are not types.

## 9. Declarations and bindings

### 9.1 Declarations

```mend
HEADER_LEN :: 16usize;               // constant
VERSION : u16 : 7;                   // constant with an explicit type
CHUNK :: p#/'CHNK' {size: [4]}/#;      // pattern constant
count :: u32[0x08];                  // view (§12.2)
Hdr :: struct { ver: u16[0x04] }     // struct (§12.3)
Kind :: enum u8 { A = 1, B = 2 }     // enum (§9.6)
align :: (n: usize) -> usize { (n + 3) & !3 }   // function
```

- Constants are evaluated at compile time. They cannot access the content.
  A failed evaluation is a compile error.
- A declaration whose right-hand side is a typed content access is a
  **view**. A view is not evaluated when declared (§12.2).
- A declaration ends with `;`, except a function, a struct or an enum, which
  ends at its closing brace. A `;` after a struct or an enum is optional.

### 9.2 Bindings

```mend
x := 10;
y: u8 = 10;
mut n := 0usize;
(lo, hi) := split(v);
```

- A binding is immutable unless it is declared `mut`. Only a `mut` binding can
  be assigned (`=`, `+=`, …) or changed in place (`s.push(x)`).
- Reading a binding by name yields a copy of its value. A later change to
  either binding, by assignment or in place, does not affect the other. This
  holds for every type, `str`, `[T]` and function values included; Mend has
  no references.
- Shadowing is allowed.
- A `mut` parameter (`(mut n: u32)`) is a mutable local copy of the argument.
- The content itself is always writable, through views, typed access and
  edit statements. Its mutability is not tied to any binding.

### 9.3 Inference and coercion

- Function signatures are written out in full and are never inferred.
- Inference is local to one function body (or to the top level) and works in
  both directions. Unsuffixed literals take the type their uses require, and
  otherwise default to `i32` or `f64`.
- A literal that does not fit its type is a compile error.
- There is no implicit numeric conversion. Use `as`, with Rust's rules, or
  `try_into[U]() -> Opt[U]`. `usize` and `u64` are the same type, as are `isize`
  and `i64`, so a `u64` field is an offset without a cast.
- Implicit coercions, applied only where an expected type is known:
  `[T; N] → [T]`, `! → any type`, `Match[P] → Match`. The expected type
  flows into the branches of `if` and `match`.

### 9.4 Modules and imports

A module is one `.mend` file. The **root module** is the program named on the
command line. `use` declarations import other modules.

| Form                               | Binds                       | Reference     |
|------------------------------------|-----------------------------|---------------|
| `use a::b::file;`                  | the module, as `file`       | `file::NAME`  |
| `use a::b::file as gfx;`           | the module, as `gfx`        | `gfx::NAME`   |
| `use a::b::file::{A, B as C};`     | the names `A` and `C`       | `A`, `C`      |
| `use a::b::file::*;`               | every name the module exports | `NAME`      |

```mend
use formats::riff;
use formats::firmware as fw;
use util::bits::{align_up, FLAG_DEAD as DEAD};
use util::checksums::*;

main :: () {
    assert riff::Riff.form == b"WAVE";
    fw::Header.version = 3;
    fw::Header.checksum = crc32(u8[fw::BODY..]);
}
```

**Resolution**
- The path `a::b::file` names the file `a/b/file.mend`.
- It is searched for relative to the root module's directory, then in each
  `-I <dir>` in order. The first match wins. If nothing matches, the compile
  error lists the paths tried.
- Path segments are identifiers, so file and directory names must be valid
  identifiers.
- The last segment of a `use` path always names a file. Item imports always
  use `{…}` or `*`.
- There are no relative paths: `self::` and `super::` do not exist. A path
  means the same thing in every module.
- A file reached by several paths is one module, identified by its canonical
  path.

**Module contents**
- A non-root module contains only `use` declarations and declarations:
  constants, views, structs, enums and functions.
- Top-level statements, `endian` and `main` are compile errors in a non-root
  module.
- Every declaration is exported. There are no private items. `use`
  declarations are not exported: an import is visible only in the module that
  writes it.
- Import cycles are allowed, because modules contain only order-independent
  declarations.

**Names**
- `use` declarations come first in a module and end with `;`.
- A qualified name is `module::NAME`, one level deep: `module::Struct.field`,
  `module::f(x)`, `module::Kind.META` and `Match[module::PAT]` are all valid.
- `use` imports modules and items, never enum variants:
  `use m::Kind.META;` and `use m::{Kind.META};` are compile errors (E0608).
  Import the enum and write `Kind.META`.
- A module alias or explicit import that conflicts with a local declaration
  or another explicit import is a compile error.
- Glob imports have lower priority than local declarations and explicit
  imports. A name provided by two globs is an error only when it is used.
- Content-function inference (§5.3) runs over the whole program, across
  modules.

### 9.5 Parameters

A parameter is an immutable value supplied on the command line.

```mend
ID   :: param u16 = 1;
NAME :: param str;
```

```
mend fix.mend firmware.bin -D ID=7 -D NAME=board-a
```

- A parameter is declared as `name :: param T [= default];`, only in the
  root module. A library receives values through function parameters, and
  `-D` names stay unqualified.
- `T` is `bool`, `char`, `str`, an integer type or a float type. The default
  is a constant expression of type `T`.
- `-D name=value` parses `value` with the rules of `parse[T]` (§16). `bool`
  accepts `true` and `false`. `str` takes the text verbatim.
- An unknown name, a repeated name, a value that does not parse, or a
  missing value for a parameter without a default is a usage error
  (exit code 4).
- Parameters are bound before the top-level statements run (§5.2).
  Functions, views, struct fields, top-level statements and `main` can refer
  to them.
- A parameter is not a constant. Constants, array lengths and pattern
  literals cannot refer to it, so the compiled form does not depend on the
  arguments and the cache (§18.1) stays valid.

### 9.6 Enums

An enum names a set of integer constants that share one type.

```mend
Kind :: enum u16 {
    DATA  = 0,
    INDEX = 4,
    META  = 0x10,
};

kind :: u16[0x08];

main :: () {
    k := kind;
    assert Kind.contains(k), f"unknown kind {k:#x}";
    size := match k {
        Kind.DATA  => 16usize,
        Kind.INDEX => 64,
        _          => 128,
    };
    if k == Kind.DATA { kind = Kind.META; }
    print(f"{Kind.name_of(k).unwrap()}: {size} bytes");
}
```

Declaration:
- An enum is declared at the top level as
  `Name :: enum T { VARIANT = value, … }`.
- `T` is required and must be an integer type. It is the type of every
  variant, and so fixes the encoded size.
- Every variant has an explicit value. `enum u16 { A, B, C }` is a compile
  error (E0607): the value is what the file stores, so it is written where it
  is declared and reordering variants cannot change it.
- A value is a constant expression of type `T` (§9.1); unsuffixed literals
  take type `T`. It may refer to constants and to variants, including other
  variants of the same enum, always as `Name.VARIANT`. A cycle is a compile
  error.
- An enum has at least one variant. Variant names are unique within the enum.
  Two variants with the same value are a compile error; write an alias as a
  constant (`DEFAULT : u16 : Kind.DATA;`).
- `contains` and `name_of` are not valid variant names.

Use:
- `Name.VARIANT` is a constant of type `T`, valid wherever a constant is:
  constant expressions, top-level statements, array lengths (when `T` is
  `usize`) and `match` patterns (§10.4). Using it is not a content operation.
- Operators and encoding treat a variant as a plain `T`: `u16[at] = Kind.META`
  writes two bytes in the current byte order, and `insert Kind.META at 0`
  inserts two bytes.
- From another module, a variant is `module::Name.VARIANT`.
- Variants cannot be imported on their own (§9.4). A variant is always
  written with its enum's name.

| Member                 | Result     | Meaning                                    |
|------------------------|------------|--------------------------------------------|
| `Name.contains(v: T)`  | `bool`     | true if `v` is the value of a variant      |
| `Name.name_of(v: T)`   | `Opt[str]` | the name of the variant whose value is `v` |

Not a type:
- As with structs, an enum is not a type and has no instances. Values read
  from the content are plain `T` and compare with variants directly.
  Validate them with `Name.contains`.
- `Name` alone is not an expression (E0603).
- A `match` on a `T` that lists every variant is not exhaustive; it still
  needs `_`.

## 10. Expressions

### 10.1 Blocks

`{ s1; s2; e }` evaluates to `e`, or to `()` if there is no trailing
expression. An expression that ends in a block (`if`, `match`, `for`,
`batch`, or a bare block) can be used as a statement without `;` if its type
is `()` or `!`.

### 10.2 Operators

Highest precedence first:

| Level | Operators                                       | Assoc. |
|-------|-------------------------------------------------|--------|
| 1     | call, method, index, field                      | left   |
| 2     | unary `-` `!`                                   | prefix |
| 3     | `as`                                            | left   |
| 4     | `*` `/` `%` `*%`                                | left   |
| 5     | `+` `-` `+%` `-%` `++`                          | left   |
| 6     | `<<` `>>`                                       | left   |
| 7     | `&`                                             | left   |
| 8     | `^`                                             | left   |
| 9     | `\|`                                            | left   |
| 10    | `??`                                            | right  |
| 11    | `==` `!=` `<` `>` `<=` `>=`                     | none   |
| 12    | `&&`                                            | left   |
| 13    | `\|\|`                                          | left   |
| 14    | `..` `..=` `..+`                                | none   |
| 15    | `=` and compound assignments                    | right  |
| 16    | `return`, `break`, `assert`, edit statements    | prefix |

| Operator        | Operands → result                                 | Runtime error                     |
|-----------------|---------------------------------------------------|-----------------------------------|
| `+ - * / %`     | same integer or float type `T` → `T`              | integers only: overflow; division by zero; `MIN / -1`. Floats follow IEEE 754 and never raise |
| `+% -% *%`      | same integer type → same type, wrapping           | —                                 |
| `& \| ^`        | same integer type, or `bool`                      | —                                 |
| `!`             | `bool` (logical) or integer (bitwise)             | —                                 |
| unary `-`       | signed integer or float                           | overflow on `MIN`                 |
| `<< >>`         | integer `T` on the left, any integer on the right → `T` | shift amount < 0 or ≥ bit width |
| `&& \|\|`       | `bool`; short-circuit                             | —                                 |
| comparisons     | same equatable or ordered type → `bool`           | —                                 |
| `??`            | `Opt[T]` and `T` → `T`; the right side is evaluated only if needed | —                    |
| `++`            | `[T]` and `[T]` → `[T]`; `str` and `str` → `str`  | —                                 |
| `a..b`, `a..=b`, `a..+n` | same integer type `T` → `Range[T]`       | overflow                          |

`>>` is arithmetic on signed types and logical on unsigned types. Rotations,
saturating arithmetic and checked arithmetic are methods (§16).

### 10.3 Calls

- `f(a, b)` calls a function value.
- `x.m(a)` calls the built-in method `m` of `x`'s type. If there is none, it
  calls a function `m(x, a)` in scope. A name that matches both is a compile
  error.
- Built-in operations take type arguments in `[ ]` (`s.parse[u8]()`) and
  optional **named arguments** (`find(P, from: 0x100)`). User functions take
  positional arguments only.

### 10.4 Control flow

```mend
kind := if tag == b"SND " { 1 } else if tag == b"TEX " { 2 } else { 0 };

if some(m) := find(CHUNK) { print(f"chunk at {m.start:#x}") }

name := match u8[0] {
    0x00         => "empty",
    0x01 | 0x02  => "small",
    0x10..=0x1F  => "medium",
    t if t >= 0xF0 => f"reserved {t}",
    _            => "other",
};
```

**`if`**
- The condition must be a `bool`. Both branches must have the same type, and
  an `if` without `else` has type `()`.
- `if pat := e` runs the block when `e` matches `pat`.

**`match`**
- Allowed patterns: literals, enum variants (`Kind.META`, §9.6), literal
  ranges (whose bounds may be variants), `_`, an identifier binding,
  `some(p)`, `none`, and tuples. Alternatives are separated by `|`, and an
  arm may have an `if` guard.
- Arms are tried top to bottom. A `match` must be exhaustive.

**`for`**

| Form                       | Meaning                                     |
|----------------------------|---------------------------------------------|
| `for { … }`                | infinite loop                               |
| `for c { … }`              | loop while `c` is true                      |
| `for x : e { … }`          | iterate over `e`                            |
| `for some(x) := e { … }`   | loop while `e` matches                      |
| `'l: for …`                | labelled loop; `break 'l`, `continue 'l`    |

| Iterable                                | Element      |
|-----------------------------------------|--------------|
| `Range[T]`, `r.rev()`, `r.step_by(k)`   | `T`          |
| `[T; N]`, `[T]`                         | `T`          |
| `s.enumerate()`                         | `(usize, T)` |
| `str`                                   | `char`       |
| `P: Pat`, `matches(P, within: r)`       | `Match[P]` (§14.4) |

Loop values:
- `for { … }` has the type of its `break` values, or `!` if it never breaks.
- Any other loop may have an `else` block, which runs when the loop ends
  without `break`. The loop's type is then the common type of the `break`
  values and the `else` block.
- Without `else`, the loop has type `()`.

**Other rules**
- `return e` leaves the enclosing function or lambda. In `main`, `return;`
  ends the run successfully. `return` is not allowed in top-level
  statements.
- A lambda cannot be the outermost expression of an `if`, `for` or `match`
  head. Wrap it in parentheses.

## 11. Functions and lambdas

```mend
align_up :: (n: usize, to: usize) -> usize {
    assert to.is_power_of_two();
    (n + to - 1) & !(to - 1)
}

xor_with :: (key: u8) -> (u8) -> u8 { (b: u8) -> u8 { b ^ key } }
```

**Signatures**
- Functions are declared `name :: (params) -> R { body }`. If `-> R` is left
  out, the return type is `()`.
- Parameter types are always written out. This keeps `(x)` from ever being
  a lambda.

**Recursion and function values**
- Top-level functions may be recursive and mutually recursive. Lambdas
  cannot refer to themselves.
- Functions and lambdas share function types. Function values cannot be
  compared.

**Captures and the content**
- Lambdas capture outer bindings **by value**, at the moment the lambda is
  created. Captures are immutable inside the lambda.
- All functions and lambdas can read and edit the content. Only the top
  level cannot (§5.3).

## 12. Content access

### 12.1 Typed access

`T[at]` reads a value of type `T` stored at offset `at`. Used as a place, it
writes one.

| Form            | Type of the expression | Meaning                                            |
|-----------------|------------------------|----------------------------------------------------|
| `T[at]`         | `T`                    | decode `T` at offset `at: usize`                   |
| `T[r]`          | `[T]`                  | decode `r.len() / size_of(T)` elements; the range length must be a multiple of the element size |
| `T.le[…]`, `T.be[…]` | as above          | the same, with an explicit byte order              |
| `[T; N][at]`    | `[T; N]`               | decode an array                                    |
| `T.get(at)`, `T.le.get(at)`, `T.be.get(at)` | `Opt[T]` | `none` if `at..+size_of(T)` goes past `len()`; otherwise `some(T[at])` |

`T` is any decodable type. As a place, `T[x] = v` overwrites the bytes and
never changes the content length.

```mend
magic_ok := u8[0..4] == b"RIFF";
u16.be[0x20] = 0x0102;
u8[0x30] |= 0x01;
```

Range rules:
- In content access and edit statements, the open forms `a..`, `..b` and
  `..` take their missing bound from `len()`.
- `a..+n` means `a..a+n`.
- A range with `start > end`, or one that goes past `len()`, raises
  `OutOfBounds` when it is used.

### 12.2 Views

A view declaration `name :: <typed access>;` names a location.

- Each use of `name` evaluates the offset expression again and reads the
  content. Assigning to `name` writes the content.
- A view's offset may refer to constants, other views and functions, so a
  location can depend on data:

```mend
entry_count  :: u32[0x08];
table_offset :: u32[0x0C];
last_entry   :: u32[table_offset as usize + (entry_count as usize - 1) * 4];
```

- Views may be declared inside blocks. There they may also refer to bindings
  in scope.
- Views are not values. Use `x := view;` to capture the current value.

### 12.3 Structs

A `struct` names a record in the content. Each field is a view with a name, a
type and a location. A struct is a demarcation of a region of the file, not a
data type.

```mend
Header :: struct {
    magic:   [u8; 4][0x00],
    version: u16[0x04],
    flags:   u16.be[0x06],
    count:   u32[0x08],
    table:   u8[0x10..+count as usize * 8],
}

assert Header.magic == b"FWIM";
Header.version = 3;
Header.flags |= 0x0001;
```

Declaration:
- A struct is declared at the top level as
  `Name :: struct { field: <content access>, … }`.
- Each field uses any form from §12.1: a scalar, an array, a range, or an
  explicit byte order.
- Field names are unique within a struct. Fields may overlap.

Use:
- `Name.field` behaves exactly as a view (§12.2). Each read decodes the
  content, and each assignment, including compound assignment, encodes into
  it.
- Inside `batch`, fields read the snapshot, like any view.

Offsets:
- Field offsets are absolute. A record at a base offset writes the base in
  each field: `version: u16[HDR + 4]`.
- Inside the struct body, a field's offset may refer to sibling fields by
  bare name, as well as to anything a top-level view may refer to.
- A cycle among field offsets is a compile error.

No instances:
- A struct is not a type and has no instances. There is no constructor.
- `Name` alone is not an expression. It cannot be bound, passed, returned,
  compared, or used as a type. Only `Name.field` is valid.

### 12.4 Byte order

The byte order of a multi-byte encode or decode is chosen by the first rule
that applies:

1. an explicit `T.le` / `T.be`, or a `to_bytes(e)` argument;
2. the **current byte order** of the content;
3. `le`.

The statement `endian e;` sets the current byte order. Here `e` is
`little`, `big`, or any `Endian` expression, so the byte order can be
detected at runtime. A top-level `endian` accepts only `little` or `big`. It
sets the byte order that `main` starts with.

```mend
endian match u16.le[0] {
    0xFEFF => le,
    0xFFFE => be,
    _      => fail("bad byte-order mark"),
};
```

Encoding rules:
- Aggregates encode element by element, in order, with no padding.
- Decoding a `bool` other than `0`/`1`, or an invalid `char`, raises
  `InvalidValue`.
- `to_bytes`/`from_bytes` on multi-byte values need an explicit `Endian`.

The front end marks each access as static-endian or dynamic-endian (§18).

## 13. Locations

### 13.1 Offsets, ranges and matches

A location is one of:

- an offset (`usize`);
- a range (`Range`);
- a pattern (`Pat`), which denotes its matches.

Offsets, ranges and matches are plain values and do not move when the
content changes. §14.3 and §14.4 define the constructs that account for
edits.

### 13.2 Patterns

A pattern literal `p#/…/#` is compiled at compile time. A malformed pattern is
a compile error. Whitespace separates elements and is otherwise ignored.

```mend
SECT_HDR :: p#/
    'SECT'            // magic
    {ver: 0?}         // version 0x00–0x0F
    [2]               // reserved
    {flags: 80/80}    // bit 7 set
/#;
```

| Element      | Example           | Matches                                   |
|--------------|-------------------|-------------------------------------------|
| byte         | `4F`              | `0x4F`                                    |
| wildcard     | `??`              | any byte                                  |
| nibble       | `4?`, `?F`        | one fixed nibble                          |
| masked byte  | `40/F0`           | `b & 0xF0 == 0x40`                        |
| text         | `'RIFF'`          | the UTF-8 bytes of the text; `\'` and `\\` are its only escapes, and `/#` inside it does not end the literal |
| gap          | `[4]`, `[2..8]`, `[2..=8]` | a fixed or bounded number of any bytes |
| group        | `( … )`           | grouping                                  |
| alternation  | `(00 01 \| FF)`   | either alternative                        |
| capture      | `{name: …}`       | records the sub-match range as `name`     |

Matching semantics:
- The leftmost match wins. Alternatives are tried in order. Gaps try the
  shortest length first.
- A pattern that can match zero bytes is a compile error.
- Successive matches never overlap. The next search starts at the previous
  match's `end`.
- `Pat.bytes(s: [u8])` builds an exact-match pattern at runtime, with no
  captures.

### 13.3 Match values

| Member        | Type                | Meaning                                        |
|---------------|---------------------|------------------------------------------------|
| `m.start`, `m.end` | `usize`        | bounds of the match                            |
| `m.range`     | `Range`             | `m.start..m.end`                               |
| `m.len()`     | `usize`             | length of the match                            |
| `m.name`      | `Range` or `Opt[Range]` | a capture: `Opt[Range]` if it is inside an alternative that may not be taken |

Capture names are checked at compile time. Naming `Match[P]` in a signature
requires `P` to be a top-level pattern constant, in the same module or
qualified (`Match[fmt::CHUNK]`).

### 13.4 Search functions

| Function                           | Result        | Notes                                        |
|------------------------------------|---------------|----------------------------------------------|
| `find(P, from:, within:)`          | `Opt[Match[P]]`   | first match                                  |
| `rfind(P, before:, within:)`       | `Opt[Match[P]]`   | last element of `find_all` over the window ending at `before` |
| `match_at(P, at)`                  | `Opt[Match[P]]`   | match anchored at `at`                       |
| `is_at(P, at)`                     | `bool`        | true if `match_at(P, at)` is `some`          |
| `contains(P, within:)`             | `bool`        | true if `find` would succeed                 |
| `count(P, within:)`                | `usize`       | number of matches                            |
| `find_all(P, within:)`             | `[Match[P]]`  | snapshot of all matches                      |
| `matches(P, within:)`              | iterator      | lazy; adjusts for edits (§14.4)              |
| `expect(P, from:, within:)`        | `Match[P]`    | like `find`, but raises `NotFound` if there is no match |

## 14. Edits

### 14.1 Edit statements

```
replace <target> with <value>;
insert  <value>  at <offset>;
insert  <value>  before <target>;
insert  <value>  after  <target>;
delete  <target>;

<target> = [first | last] <pattern> [within <range>]
         | <range>
```

| Statement                     | Range target `r`            | Pattern target `P`                         |
|-------------------------------|-----------------------------|--------------------------------------------|
| `replace T with v`            | replace bytes in `r` by `encode(v)` | replace each match                 |
| `insert v at o`               | —                           | — (`o: usize`; `o == len()` appends)       |
| `insert v before T`           | insert at `r.start`         | insert before each match                   |
| `insert v after T`            | insert at `r.end`           | insert after each match                    |
| `delete T`                    | remove bytes in `r`         | remove each match                          |

Value rules:
- `v` is encoded using the current byte order.
- With a pattern target, `v` may also be a function `(Match[P]) -> V`, which
  is called once per match. It sees the content as it was before that edit.

Target rules:
- A pattern target without `first`/`last` affects **every** match.
  `first`/`last` affect one match: the first or last element of
  `find_all(P)`. The last match never overlaps the forward sequence.
- A pattern target that matches nothing raises `NotFound`. To allow zero
  matches, guard the statement: `if contains(P) { … }`.
- Scanning for multiple matches follows §14.4, so content that the edit
  inserted is never scanned again.

Other rules:
- Edit statements have type `()`. They are allowed anywhere an expression
  is, including inside functions and lambdas.
- Overwrites go through typed access or views (§12). They never change the
  length.

```mend
replace p#/FF 00 FF/# with x"00 FF 00";
replace first CHUNK with (m: Match[CHUNK]) -> [u8] {
    b"CHNK" ++ (u32[m.size.start] * 2).to_bytes(be)
};
insert [0u8; 16] after p#/'HDR' [12]/#;
delete 0x40..+16;
delete p#/'DBG ' [12]/# within 0x100..;
```

### 14.2 Sequences with a runtime length

`[x; n]` with a non-constant `n` has type `[T]`. For example,
`replace r with [0u8; r.len()];` fills a range.

### 14.3 `batch`

`batch { … }` makes every offset inside the block refer to the content as it
was when the block began.

1. Reads inside the block see a **snapshot** taken on entry, including
   `len()`, searches, views and typed access. Edits made in the block are not
   visible to reads in the block.
2. Edits are checked against the snapshot and queued as (snapshot range, new
   bytes, sequence number). Insertions have empty ranges.
3. When the block exits normally, including by `break` or `return`, the queue
   is checked:
   - overlapping non-empty ranges raise `BatchConflict`;
   - an insertion strictly inside another edit's range raises
     `BatchConflict`;
   - insertions at the same point apply in program order;
   - an insertion at a replaced range's start goes before the replacement,
     and one at its end goes after it.

   Then every queued edit is applied.
4. Nesting `batch` is an error: a compile error when it is visible in the
   source, and a runtime error through a function call.
5. Edits made by functions called inside the block are also queued.
6. `endian` statements are not allowed inside `batch`.

```mend
riff_size :: u32[4];

batch {
    riff_size += chunk.len() as u32;   // reads the snapshot value
    insert chunk at 12;                // 12 is a snapshot offset
}
```

### 14.4 Scanning during edits

Iterating a pattern (`for m : P`, or `matches(P)`), and multi-match edit
statements, keep two **markers** that adjust for edits:

- `pos`, where the next search starts;
- `lim`, where the window ends.

When an edit replaces the range `[a, b)` with `n` bytes, each marker `x` is
adjusted:

| Case                            | `pos`              | `lim`              |
|---------------------------------|--------------------|--------------------|
| `x < a`                         | `x`                | `x`                |
| `x == a == b` (insertion at `x`)| `x + n`            | `x`                |
| `x >= b` (otherwise)            | `x + n - (b - a)`  | `x + n - (b - a)`  |
| `a <= x < b`                    | `a + n`            | `a`                |

As a result:
- Bytes inserted at or just after the current match are never scanned again.
- Bytes inserted at the window end are outside the window.
- Edits after `pos` and inside the window are visible to later searches.

Inside `batch`, scans read the snapshot and markers do not move.

## 15. Assertions and errors

`assert c;` and `assert c, msg;` have type `()`.

- `c` must be a `bool`, and `msg` a `str`.
- `msg` is evaluated only if the assertion fails.
- When `c` is a comparison, the error report shows both operand values.

| Construct               | Behaviour                                |
|-------------------------|------------------------------------------|
| `fail(msg) -> !`        | raises `Assertion` with `msg`            |
| `o.unwrap()`            | raises `NotFound` if `o` is `none`       |
| `o.expect(msg)`         | raises `NotFound` with `msg` if `o` is `none` |
| `expect(P)`             | raises `NotFound`, quoting the pattern   |

Errors are not recoverable: there is no `try`, `catch` or error type.
Operations that can fail in normal use have an `Opt[T]` form instead: `find`,
`match_at`, `T.get`, `checked_*`, `try_into`, `parse` and `str.from_utf8`.
A patch either applies completely or not at all.

Every runtime error aborts the run, and no output is written. The report
includes the error kind, the message, the source span, the call stack and,
when relevant, the content offset.

| Kind             | Raised by                                              |
|------------------|--------------------------------------------------------|
| `Assertion`      | `assert`, `fail`                                       |
| `NotFound`       | `unwrap`, `expect`, a pattern target with no matches   |
| `OutOfBounds`    | content access, an edit, an invalid range, indexing    |
| `Overflow`       | checked integer arithmetic, shifts, range arithmetic   |
| `DivisionByZero` | integer `/`, `%`                                       |
| `InvalidValue`   | decoding a bad `bool`, `char` or UTF-8 string; `align_up`/`align_down` with a non-power-of-two; `clamp` with `lo > hi`; `Pat.bytes([])` |
| `LengthMismatch` | `T[r] = v` where `v` has a different length; `T[r]` where `r.len()` is not a multiple of `size_of(T)`; `T.from_bytes(b)` where `b.len() != size_of(T)` |
| `BatchConflict`  | §14.3; a `batch` or an `endian` statement reached through a call inside `batch` |
| `StepLimit`      | `--max-steps`                                          |
| `StackOverflow`  | a call nested more than 100 000 deep                   |

## 16. Standard library

All names are in the prelude.

**Content**

| Function                     | Result      |
|------------------------------|-------------|
| `len()`                      | `usize`     |
| `endianness()`               | `Endian`    |
| `modified()`                 | `bool`      |
| `input_path()`, `output_path()` | `str`    |
| search functions             | §13.4       |

`modified()` is true if the content differs from the input in length or in
any byte. Writing identical bytes leaves it false, so a program can detect an
already-patched file.

**Integers** (`T` is any integer type)
- Constants: `T.MIN`, `T.MAX`, `T.BITS`.
- Arithmetic: `wrapping_*`, `saturating_*`, and `checked_* -> Opt[T]` (`add`,
  `sub`, `mul`, `div`, `shl`, `shr`); `pow`; `abs`; `min`; `max`; `clamp`.
- Bits: `rotate_left/right(n: u32)`, `swap_bytes`, `reverse_bits`,
  `count_ones`, `count_zeros`, `leading_zeros`, `trailing_zeros`.
- Alignment: `is_power_of_two`, `align_up(a)`, `align_down(a)`.
- Bit fields: `bit(i: u32) -> bool`, `with_bit(i, v)`, `bits(r: Range[u32]) -> T`,
  `with_bits(r, v)`.
- Bytes: `to_bytes(e) -> [u8; size_of(T)]`, `T.from_bytes(b: [u8], e) -> T`.
- Conversion: `try_into[U]() -> Opt[U]`.

**Sequences**
- Queries: `len`, `is_empty`, indexing, slicing, `contains`,
  `position -> Opt[usize]`.
- Building: `++`, `reverse`, `repeat(n)`, `enumerate`.
- Higher-order: `map`, `filter`, `fold`, `any`, `all`, and checked `sum`.
- On `mut` bindings only: `push`, `pop -> Opt[T]`, `insert(i, x)`, `remove(i)`.

**Strings**
- `len` (in bytes), `chars`, `bytes`, `++`, `contains`, `starts_with`,
  `ends_with`.
- `parse[T]() -> Opt[T]` accepts the `0x`, `0b` and `0o` prefixes.
- `str.from_utf8(b) -> Opt[str]`.

**Options**
- `Opt[T]` follows Rust's `Option<T>`: constructors `some(x)` and `none`;
  methods `is_some`, `is_none`, `unwrap`, `expect(msg)`, `unwrap_or(d)`,
  `map(f)`, `and_then(f)`, `or(o)`, `filter(f)`; and `o ?? d`, which is
  `o.unwrap_or(d)` with `d` evaluated only if `o` is `none`.
- `none` has type `Opt[T]` for the `T` the context requires; with no
  context it is an error.

**Checksums**
- `crc32`, `crc32c`, `adler32`, `fnv1a32`, `fnv1a64`, each taking `[u8]`.

**Output**
- `print(s)` writes to stdout, and `eprint(s)` to stderr.
- Format specs: `[[fill]align][#][0][width][x|X|b|o|e|?]`. `?` prints byte
  sequences as hex, and a `Match` prints as `0x40..0x48`.

## 17. Dynamic semantics

- **Evaluation order:** left to right, for operands, arguments (positional,
  then named, as written) and elements.
- **Assignment:** the right-hand side is evaluated first, then the place, then
  the store.
- **Short-circuit:** `&&`, `||` and `??` evaluate their right side only when
  needed.
- **Views:** a view's offset expression is evaluated on every access, in the
  current state of the content.
- **Bounds:** every content access is bounds-checked against the current
  `len()`, or the snapshot length inside `batch`.
- **Determinism:** runs have no clock, randomness or hash-order iteration.
  The same program and input always produce the same output, and the same
  `--dry-run` report.

## 18. Compilation model

### 18.1 Pipeline

```
source → load modules → lex → parse → resolve → type-check → desugar → Typed Core → const-eval → lower → instructions → run
```

The program is compiled before the input is read, on every run. Version 1
does not cache the compiled form (bytecode RFC §16.2).

### 18.2 Desugaring

| Surface                                    | Typed Core                                         |
|--------------------------------------------|----------------------------------------------------|
| view use, struct field use, typed access   | offset computation + `load[T, endian]` / `store[T, endian]` intrinsics |
| edit statements                            | `edit_replace`, `edit_insert`, `edit_delete` intrinsics; loops with markers for pattern targets |
| `for x : iterable`                         | `for {}` with explicit iterator state              |
| `for … else`, labels                       | structured loops with explicit exit edges          |
| `match`, `if pat := e`                     | decision trees                                     |
| `a..+n`, open ranges                       | explicit ranges with checked arithmetic and `len()` bounds |
| compound assignment, UFCS, format strings  | plain loads, stores, calls and formatting intrinsics |
| lambdas                                    | closure conversion; functions with no captures have no environment |
| modules, qualified names                   | one flattened program with module-unique symbols   |
| parameters, top-level statements, `main`   | an `init` function that binds the parameters and runs before the input is read, then `main` as the entry point |
| literals, coercions                        | concrete types, explicit coercion nodes            |
| enum variants, `Name.contains`, `Name.name_of` | constants of type `T`; comparison chains; a decision tree over string constants |

Constants, array lengths and patterns are evaluated at compile time on the
Typed Core, with content intrinsics disabled and a fixed step limit.

### 18.3 Interface to the instruction-set RFC

The front end guarantees:

1. One concrete type per value. No type variables or overloads remain.
2. Every runtime check is explicit: arithmetic, bounds, shifts, unwraps and
   decoding validity. The backend may remove a check only if it proves the
   check cannot fail.
3. Intrinsics have stable IDs plus type arguments. Byte order is a static
   operand (`le`/`be`) or a dynamic one.
4. The content is a single implicit buffer. No intrinsic takes a file or
   buffer operand.
5. Patterns are delivered as matcher descriptions: a DFA when the pattern
   has no alternation or variable gaps, and an ordered backtracking program
   otherwise. Each comes with capture slots and a proof that it cannot match
   zero bytes.
6. Every node carries a source span.
7. `batch` lowers to `batch_begin` / `batch_end`, with `batch_end` on every
   non-error exit.
8. Content intrinsics appear only in content functions (§5.3), which
   include `main` when it touches the content. `init`
   code never needs the content buffer.

The runtime provides:

- the content buffer, with snapshots, edit queues and marker adjustment;
- the pattern engine;
- parsing `-D` arguments into parameter values (§9.5);
- reading the input and committing the output (§5.2);
- formatting;
- error reports;
- the step counter.

The companion RFC defines the instruction set, its encoding, the
calling convention, the closure layout, and the code-generation strategy.

## 19. Diagnostics

Diagnostics use the rustc layout: an error code, a primary span, and
`help:` notes.

```
error[E0308]: mismatched types
 --> fix.mend:8:11
  |
8 | timeout = extra + 1u16;
  |           ^^^^^^^^^^^^ expected `u32`, found `u16`
  = help: `timeout` is a `u32` view (line 3)
```

```
error[E0602]: constants cannot access the content
 --> fix.mend:2:10
  |
2 | LIMIT :: u32[0] * 2;
  |          ^^^^^^ content access
  = help: use a binding (`limit := u32[0] * 2;`) or a view
```

```
error[E0603]: struct `Header` is not a value
 --> fix.mend:9:6
  |
9 | h := Header;
  |      ^^^^^^ a struct names a region of the content
  = help: access a field instead, e.g. `Header.version`
```

```
error[E0604]: content operation at the top level
 --> fix.mend:7:6
  |
7 | x := timeout;
  |      ^^^^^^^ `timeout` is a view; reading it accesses the content
  = help: move this statement into `main` or a function
```

```
error[E0605]: module not found: `formats::riff`
 --> fix.mend:1:5
  |
1 | use formats::riff;
  |     ^^^^^^^^^^^^^ no file `formats/riff.mend`
  = note: searched: ./formats/riff.mend, /opt/mend/lib/formats/riff.mend
```

```
error[E0606]: constant refers to a parameter
 --> fix.mend:3:8
  |
3 | MAX :: ID * 2;
  |        ^^ `ID` is a parameter; its value is known only at run time
  = help: use a binding (`max := ID * 2;`)
```

```
error[E0607]: enum variant without a value
 --> fix.mend:3:5
  |
3 |     INDEX,
  |     ^^^^^ every variant of `Kind` needs an explicit value
  = help: write the value stored in the file, e.g. `INDEX = 4`
```

```
error[E0608]: enum variants cannot be imported
 --> fix.mend:1:14
  |
1 | use formats::Kind.META;
  |              ^^^^^^^^^ `META` is a variant of `Kind`
  = help: import the enum (`use formats::{Kind};`) and write `Kind.META`
```

Lints:

- a `mut` binding that is never mutated;
- a shadowed prelude name;
- a read inside `batch` of bytes the same batch has already written;
- a pattern target that can never match within its `within` range.

## 20. Grammar

The notation follows sophia's `grammar.ebnf`. Lexical rules are in §7.

```ebnf
program         = { use-decl } , { top-item } ;
top-item        = declaration | statement ;

use-decl        = "use" , path , [ "as" , identifier ] , ";"
                | "use" , path , "::" , "{" , use-item , { "," , use-item } , [ "," ] , "}" , ";"
                | "use" , path , "::" , "*" , ";" ;
path            = identifier , { "::" , identifier } ;
use-item        = identifier , [ "as" , identifier ] ;     (* never `Enum.VARIANT`; §9.4 *)
name-ref        = identifier , [ "::" , identifier ] ;    (* qualified name *)

declaration     = identifier , "::" , function-expr
                | identifier , "::" , expr , ";"
                | identifier , ":" , type , ":" , expr , ";"
                | identifier , "::" , "param" , type , [ "=" , expr ] , ";"
                | identifier , "::" , struct-decl , [ ";" ]
                | identifier , "::" , enum-decl , [ ";" ] ;
struct-decl     = "struct" , "{" , [ field , { "," , field } , [ "," ] ] , "}" ;
field           = identifier , ":" , content-access ;
enum-decl       = "enum" , primitive-type , "{" , variant , { "," , variant } , [ "," ] , "}" ;
variant         = identifier , "=" , expr ;                 (* the value is mandatory *)

type            = primitive-type | "str" | "()" | "!"
                | "[" , type , "]" | "[" , type , ";" , expr , "]"
                | "(" , type , "," , [ type-list ] , ")"
                | "(" , [ type-list ] , ")" , "->" , type
                | "Pat" | "Endian"
                | "Opt" , "[" , type , "]"
                | "Range" , [ "[" , type , "]" ]
                | "Match" , [ "[" , name-ref , "]" ] ;
type-list       = type , { "," , type } , [ "," ] ;
primitive-type  = "bool" | "char" | "f32" | "f64"
                | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
                | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" ;

block           = "{" , { statement } , [ expr ] , "}" ;
statement       = ";"
                | binding , ";"
                | identifier , "::" , expr , ";"          (* local view *)
                | endian-stmt , ";"
                | block-like-expr , [ ";" ]
                | expr , ";" ;
binding         = [ "mut" ] , binding-pattern , ( ":=" , expr | ":" , type , "=" , expr ) ;
binding-pattern = identifier | "_" | "(" , binding-pattern , "," ,
                  [ binding-pattern , { "," , binding-pattern } , [ "," ] ] , ")" ;
endian-stmt     = "endian" , ( "little" | "big" | expr ) ;

expr            = assign-expr
                | "return" , [ expr ]
                | "break" , [ label ] , [ expr ]
                | "continue" , [ label ]
                | "assert" , expr , [ "," , expr ]
                | edit-expr ;

edit-expr       = "replace" , target , "with" , expr
                | "insert" , expr , "at" , expr
                | "insert" , expr , ( "before" | "after" ) , target
                | "delete" , target ;
target          = [ "first" | "last" ] , range-expr , [ "within" , range-expr ] ;

assign-expr     = range-expr , [ assign-op , assign-expr ] ;
assign-op       = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
range-expr      = or-expr , [ ( ".." | "..=" | "..+" ) , or-expr ]
                | or-expr , ".." | ".." , [ or-expr ] ;
or-expr         = and-expr , { "||" , and-expr } ;
and-expr        = cmp-expr , { "&&" , cmp-expr } ;
cmp-expr        = coalesce-expr , [ ( "==" | "!=" | "<" | ">" | "<=" | ">=" ) , coalesce-expr ] ;
coalesce-expr   = bitor-expr , [ "??" , coalesce-expr ] ;
bitor-expr      = bitxor-expr , { "|" , bitxor-expr } ;
bitxor-expr     = bitand-expr , { "^" , bitand-expr } ;
bitand-expr     = shift-expr , { "&" , shift-expr } ;
shift-expr      = add-expr , { ( "<<" | ">>" ) , add-expr } ;
add-expr        = mul-expr , { ( "+" | "-" | "+%" | "-%" | "++" ) , mul-expr } ;
mul-expr        = cast-expr , { ( "*" | "/" | "%" | "*%" ) , cast-expr } ;
cast-expr       = prefix-expr , { "as" , type } ;
prefix-expr     = ( "-" | "!" ) , prefix-expr | postfix-expr ;

postfix-expr    = primary-expr , { postfix-op } ;
postfix-op      = "(" , [ arguments ] , ")"
                | "." , identifier , [ "[" , type-list , "]" ] , "(" , [ arguments ] , ")"
                | "." , ( identifier | integer-literal )
                | "[" , expr , "]" ;
arguments       = argument , { "," , argument } , [ "," ] ;
argument        = [ identifier , ":" ] , expr ;

primary-expr    = literal | name-ref | "none"
                | content-access
                | primitive-type , "." , identifier          (* u32.MAX, u32.from_bytes *)
                | "Pat" , "." , identifier
                | "(" , expr , ")"
                | "(" , expr , "," , [ expr , { "," , expr } , [ "," ] ] , ")"
                | "[" , [ expr , { "," , expr } , [ "," ] ] , "]"
                | "[" , expr , ";" , expr , "]"
                | function-expr
                | block-like-expr ;

content-access  = primitive-type , [ "." , ( "le" | "be" ) ] , "[" , expr , "]"
                | "[" , type , ";" , expr , "]" , [ "." , ( "le" | "be" ) ] , "[" , expr , "]" ;

block-like-expr = block | if-expr | match-expr | for-expr | batch-expr ;

function-expr   = "(" , [ parameter , { "," , parameter } , [ "," ] ] , ")" , [ "->" , type ] , block ;
parameter       = [ "mut" ] , identifier , ":" , type ;

if-expr         = "if" , condition , block , [ "else" , ( if-expr | block ) ] ;
condition       = expr-no-lambda | match-pattern , ":=" , expr-no-lambda ;
match-expr      = "match" , expr-no-lambda , "{" , { match-arm } , "}" ;
match-arm       = match-pattern , [ "if" , expr ] , "=>" , ( block , [ "," ] | expr , [ "," ] ) ;
match-pattern   = pattern-alt , { "|" , pattern-alt } ;
pattern-alt     = "_" | identifier | "none"
                | literal-pattern , [ ( ".." | "..=" ) , literal-pattern ]
                | "some" , "(" , match-pattern , ")"
                | "(" , match-pattern , "," , [ match-pattern , { "," , match-pattern } ] , ")" ;
literal-pattern = [ "-" ] , integer-literal | char-literal | byte-char-literal
                | string-literal | "true" | "false"
                | name-ref , "." , identifier ;           (* enum variant; note 7 *)
for-expr        = [ label , ":" ] , "for" , [ binding-pattern , ":" , expr-no-lambda | condition ] ,
                  block , [ "else" , block ] ;
batch-expr      = "batch" , block ;
label           = "'" , identifier ;

pattern-literal = "p#/" , alt , "/#" ;                   (* one token; §7 *)
alt             = seq , { "|" , seq } ;
seq             = item , { item } ;
item            = hex-byte | "??" | nibble | masked-byte | text
                | "[" , integer , [ ( ".." | "..=" ) , integer ] , "]"
                | "(" , alt , ")" | "{" , identifier , ":" , alt , "}" ;
```

`main` is an ordinary `function-expr` declaration. The rules of §5.3 are
static checks, not grammar.

Disambiguation:

1. **`for` heads.** A head of the form `binding-pattern :` iterates, matching
   sophia's `for i : 0..10`. Any other head is a condition.
2. **Lambdas.** A `(` starts a lambda only when it is followed by `)` and then
   `->` or `{`, or by `[mut] identifier :`. `expr-no-lambda` is `expr`
   without an unparenthesised outermost `function-expr`.
3. **Content access.** A primitive type name followed by `[` is a content
   access. This is unambiguous because type names are keywords.
   `[T; N][at]` is an array access when `T` is a type, and a repeat literal
   followed by an index otherwise.
4. **Edit statements.** `with`, `at`, `before`, `after` and `within` end the
   preceding expression. They have no meaning as operators.
5. **Statements.** A statement that starts with a `block-like-expr` ends at
   its closing brace.
6. **`::` after an identifier.** At the start of an item or statement,
   `ident ::` is a declaration, unless `ident` is a module alias bound by a
   `use` in the same module, in which case it starts a qualified name.
   `use` declarations come first, so the parser knows every alias before it
   reads any other item. Declaring a name equal to a module alias is an
   error.
7. **Patterns.** In a `match` pattern, `name.ident` is an enum variant and a
   bare identifier is a binding. `name.ident` naming anything other than a
   variant is a compile error.

## 21. Future possibilities

- **Parameterized structs** for tables of records:
  `Entry :: struct (i: usize) { id: u16[TABLE + i * 8] }`, used as
  `Entry(3).id = 5;`.
- **Field constraints** checked as implicit assertions:
  `magic: [u8; 4][0] == b"FWIM"`.
- **Relative layout**, where a struct gets a base offset and fields follow one
  another without explicit offsets.
- **Marks:** a `Mark` type that follows edits by the rules of §14.4.
- **Enum-typed access:** `Kind[at]` decodes a `T` and raises `InvalidValue`
  when it is not a variant's value.
- **Undo output:** emit the inverse edit list alongside the output.
- **Greedy gaps:** `[a..b]!` tries the longest length first. `!` is unused
  in patterns, so adding it is backward compatible.

## 22. Alternatives considered

| Alternative                                   | Reason for rejection                                         |
|-----------------------------------------------|--------------------------------------------------------------|
| Explicit `open`/`save` and file handles       | One file per run is the use case. Handles add a capability system and I/O failure modes for no gain. |
| Structs as value types with instances        | A record in the file is a location, not a value. Instances would need copy and aliasing rules, plus a way to write them back. Fields as views give read and write access directly. |
| Content operations in top-level statements    | Mixes configuration with operations, and makes the input load order observable. With `main`, top-level checks can fail before the input is read. |
| Restricting content operations to the lexical body of `main` | Forces helpers such as `block(i)` to be inlined. Allowing them in every function and rejecting them only at the top level keeps helpers while still confining execution to `main`. |
| `f.read[u32](at)` method calls                | Noisy in a language whose subject is always the content.     |
| `T @ at` access operator                      | Has precedence traps (`u32 @ a + 4`). Brackets delimit the offset. |
| Endianness in the type (`u32be`)              | Byte order belongs to encoding, not arithmetic, and must be selectable at runtime. |
| Offsets that follow edits                     | Needs hidden indirection on every integer. `batch` and markers cover the need. |
| Pattern edits that silently match nothing     | Hides stale patches. Zero matches must be an explicit `if contains(P)`. |
| Reference captures in lambdas                 | Brings in aliasing and lifetime rules for no practical gain. |
| Patterns as prefixed strings (`p"…"`)       | Patterns are not strings. A distinct delimiter keeps string escapes and interpolation out, and lets `"` appear in text elements without escaping. |
| `.` for module access (`riff.Riff.size`)      | Ambiguous with struct field access. `::` keeps modules and fields apart. |
| String paths (`use "lib/riff.mend";`)         | Paths as identifiers are portable, and they map directly onto qualified names. |
| Private items (`pub`, `_` prefix)             | Patch libraries are small. Exporting everything keeps modules simple. |
| Relative paths (`self::`, `super::`)          | One resolution rule, from the root directory and `-I`, means a path means the same thing in every module. |
| Unbounded quantifiers in patterns             | Unpredictable cost on binary data. Bounded gaps suffice.     |
| `-D` overriding any typed constant            | Makes every constant part of the CLI, and makes the compiled form depend on the arguments. |
| Interpolation in every string literal         | Strings are written into binary data verbatim; braces in data would need escaping. |
| `usize` as a distinct type from `u64`         | Both are 64-bit, so the distinction only adds casts that cannot fail. |
| `u64` only, no `usize`                        | `usize` documents that a value is an offset or a length.     |
| `?T` sigil for optional types                 | Reads as part of `??` and pattern wildcards (`??`, `?F`). `Opt[T]` uses the same `[ ]` type-argument form as `Range[T]` and `Match[P]`. |
| `Result[T, E]` with `?` propagation           | Pushes signatures toward fallibility. `Opt[T]` forms cover expected failures, and a partial patch is never wanted. |
| `try`/`catch` by error kind                   | Needs unwinding semantics, including out of `batch`, for no practical gain. |
| Enums as distinct types (`Kind[at]`, casts to and from `T`) | Values in the content are raw integers. A distinct type adds a conversion and a validity rule to every access; `Name.contains` covers validation. |
| `::` for enum variants (`Kind::META`)          | Collides with declarations (`ident ::`) at the start of a statement, and makes module paths two levels deep. `.` matches struct fields. |
| Importing variants (`use m::Kind.META;`)      | A variant written without its enum's name reads as a plain constant and can collide with other names. |
| Implicit variant values (`enum u16 { A, B }`) | The value is part of the file format. Implicit numbering hides it and changes it when variants are reordered. |
| Duplicate variant values                      | Makes `name_of` and `match` arms ambiguous. An alias is a plain constant. |
| Captures as offsets (`u16[m.id]` for `u16[m.id.start]`) | `T[r]` already decodes `[T]` over a range. The shorthand would make the result type depend on the capture's runtime length. |

## 23. Unresolved questions

Lexical questions found while implementing the scanner. "Scanner" states
what the implementation currently does.

| Question | Tradeoff | Scanner | See |
|----------|----------|---------|-----|
| Q7. Where a format spec starts | §7 starts the spec at the first `:` outside parentheses and brackets, so `f"{riff::NAME}"` splits at `::`. Excluding `::` and `:=` and tracking braces keeps qualified names and blocks usable, at the cost of a longer rule. | `::` and `:=` do not start a spec; braces are tracked | §7 |
| Q8. Floats without a `.` | Accepting `1e10` and `3f64` follows Rust. Requiring a `.` makes every float visibly a float. | accepted | §7 |
| Q9. `\xHH` above `0x7F` in `str` and `char` | Allowing it as a code point (U+0080–U+00FF) is convenient. Rejecting it, as Rust does, avoids reading `\xFF` as a byte that would make the string invalid UTF-8. | rejected | §7 |
| Q10. Non-ASCII characters in `b"…"` and `b'…'` | Encoding them as UTF-8 allows `b"é"`. Rejecting them, as Rust does, keeps byte literals one byte per character; `x"C3 A9"` covers the rest. | rejected | §7 |
| Q11. Hex-bytes grouping | Requiring whitespace between bytes catches odd-length typos earlier. Allowing whitespace only between two-digit bytes also accepts `x"DEADBEEF"`. | two adjacent digits per byte; whitespace optional between bytes | §7 |
| Q12. Bare `}` in format-string text | An error keeps `{`/`}` symmetric with the `\{ \}` escapes. Treating it as literal text is more permissive but asymmetric. | error | §7 |
| Q13. `t.0.1` | Without a rule, `0.1` scans as a float. Scanning a number directly after `.` as an integer matches `"." , integer-literal` in §20 without parser token splitting. | integer after `.` | §20 |
| Q14. Codes for lexical errors | §19 assigns no codes to lexical errors. A dedicated range (for example E01xx) is needed before scanner errors appear in diagnostics. | no codes | §19 |

Questions resolved on 2026-10-16:

| Question                                   | Decision                                           | See        |
|--------------------------------------------|----------------------------------------------------|------------|
| Q1. User parameters                         | `param` declarations, set with `-D`; runtime values | §9.5       |
| Q2. Interpolation only in `f"…"`            | Yes                                                | §7         |
| Q3. `usize` versus `u64`                    | `usize`/`isize` are aliases of `u64`/`i64`         | §8, §9.3   |
| Q4. Recoverable errors                      | None; `Opt[T]` forms only                              | §15        |
| Q5. Greedy gap marker `[2..8]!`             | Deferred                                           | §21        |
| Q6. Captures as offsets                     | No                                                 | §22        |

## Appendix A. Examples

### A.1 Insert a RIFF chunk

```mend
endian little;

Riff :: struct {
    tag:  [u8; 4][0],
    size: u32[4],
    form: [u8; 4][8],
}

// Built before the input is read; visible in `main`.
payload := b"built-by-mend";
pad: [u8] = if payload.len() % 2 == 1 { [0u8] } else { [] };
chunk := b"MEND" ++ (payload.len() as u32).to_bytes(le) ++ payload ++ pad;

main :: () {
    assert Riff.tag == b"RIFF" && Riff.form == b"WAVE", "not a WAVE file";

    batch {
        Riff.size += chunk.len() as u32;
        insert chunk at 12;
    }
}
```

### A.2 Delete flagged table records

```mend
endian big;

RECORD :: 32usize;
DEAD : u8 : 0x80;

count :: u32[0];

main :: () {
    n := count as usize;
    mut kept := 0u32;

    batch {
        for i : 0..n {
            off := 4 + i * RECORD;
            if u8[off + 2] & DEAD != 0 { delete off..+RECORD; } else { kept += 1; }
        }
        count = kept;
    }

    print(f"kept {kept} of {n} records");
}
```

### A.3 Decode an XOR-obfuscated region with a lambda

```mend
KEY : u8 : 0x5A;

xor_with :: (key: u8) -> (u8) -> u8 { (b: u8) -> u8 { b ^ key } }

decode := xor_with(KEY);            // not a content function; valid here

main :: () {
    u8[0x10..] = u8[0x10..].map(decode);
}
```

### A.4 Walk chunks and find one with a loop expression

```mend
endian little;

CHUNK :: p#/{tag: [4]} {size: [4]}/#;

find_chunk :: (tag: [u8; 4]) -> Opt[Match[CHUNK]] {
    mut at := 12usize;
    for at < len() {
        m := match_at(CHUNK, at).expect("truncated chunk header");
        if u8[m.tag] == tag { break some(m) }
        at = (m.end + u32[m.size.start] as usize).align_up(2);
    } else {
        none
    }
}

main :: () {
    d := find_chunk(b"data").expect("no data chunk");
    print(f"data chunk at {d.start:#x}");
}
```

### A.5 Bit fields

```mend
endian little;

block_count :: u32[0];
block :: (i: usize) -> u32 { u32[4 + i * 4] }

main :: () {
    n := block_count as usize;
    mut used := 0usize;

    for i : 0..n {
        b := block(i);
        used += b.bit(31) as usize;
        if b.bits(0..4) == 0xF { u32[4 + i * 4] = b.with_bits(0..4, 0x1); }
    }

    print(f"{used}/{n} blocks in use");
}
```
