# RFC: Mend Bytecode, the Instruction Set of the Mend Interpreter

| Field          | Value                                                                 |
|----------------|-----------------------------------------------------------------------|
| Status         | Draft                                                                 |
| Created        | 2026-10-16                                                            |
| Authors        | Mário Feroldi                                                         |
| Scope          | Register model, value representation, instruction set and encoding, calling convention, closures, content and pattern runtime interface, verifier, compiled-program format, lowering from Typed Core |
| Out of scope   | Mend syntax and semantics (`rfc-mend-language.md`, cited as *Mend §N*); native code generation (§23) |
| Companion      | Mend §18.3                                                            |
| File extension | `.mbc` (serialized compiled program, §16)                             |

## 1. Summary

Mend Bytecode (MBC) is the instruction set that `mend` lowers a Mend program
to and then interprets. It is a typed register machine:

- **Two register banks per frame:** 64-bit scalar registers `s0…` and
  reference-counted handle registers `h0…` (§7).
- **No runtime type tags.** Every opcode fixes its operand types; values
  wider than one register are flattened into contiguous runs (§7.3).
- **Fixed 64-bit instruction words** with 16-bit operands, one or two words
  per instruction (§8).
- **Checked operations by default.** Each Mend runtime check is fused into
  the instruction that needs it; unchecked forms exist for proven-safe
  cases (§9).
- **Content opcodes** for typed loads and stores, edits, `batch`, and
  pattern search with edit-adjusting scanners (§9.9–§9.11, §13).
- **Pattern matchers** as data: a fixed form and a backtracking program
  form (§14).
- **A structural verifier** that makes any accepted program memory-safe to
  interpret (§17).

Lineage: Lua 5's register VM, WebAssembly's typed opcodes and validation, the
JVM's verifier and constant pool, and Pike/backtracking regex programs (§5).

**Execution model.** `mend` compiles the program at the start of each run and
interprets it immediately; no native code is generated (§6.2).

## 2. Motivation

The Mend front end produces a monomorphic, fully typed Typed Core
(Mend §18.3). Executing it needs a target that:

- keeps the per-operation cost low in loops over large files, without the
  per-node dispatch and dynamic value layout of a tree-walking interpreter;
- implements the content model (snapshots, edit queues, markers) once, in the
  runtime, behind a small set of opcodes;
- can be serialized and checked cheaply before it runs;
- maps every runtime error back to a source span and call stack.

Native compilation costs more startup time than a typical patch run takes,
so MBC is interpreted.

## 3. Goals and non-goals

**Goals**

1. Execute every Typed Core program with the exact dynamic semantics of
   Mend, including error kinds, spans and step counts.
2. No type-directed decisions at run time: the encoding of each instruction
   fixes its operand types and widths.
3. Memory safety for every program the structural verifier accepts.
4. Compile and verify in time linear in program size.
5. Determinism: the same program and input produce the same observable
   behaviour (§6.3).
6. One dispatch loop: intrinsics never call back into bytecode.

**Non-goals**

- Native code generation (§23).
- Changing Mend semantics. Gaps found while writing this RFC are listed in
  §19.3 for the language RFC to decide.
- A compiled-program cache (§23).
- Debugger and stepping support (§23).
- A stable public bytecode for third-party producers. MBC is versioned with
  the compiler.

## 4. A taste of Mend bytecode

```mend
KEY : u8 : 0x5A;
count :: u32[0];

main :: () {
    for i : 0..count as usize {
        u8[4 + i] ^= KEY;
    }
}
```

```
fn @main () frame s:6 h:0
    ldi.u64    s0, 0              ; i := range start
    ldi.u64    s2, 0
    ld.le.u32  s1, s2             ; count (no `endian` in any function: static le)
    cvt.u64    s1, s1, u32        ; as usize
L0: lt.u64     s2, s0, s1
    brf        s2, L1
    tick                          ; one source loop iteration
    ldi.u8     s3, 0x5A           ; right-hand side first: KEY
    addi.u64   s4, s0, 4          ; place: 4 + i (checked)
    ld.u8      s5, s4
    xor.u8     s5, s5, s3
    st.u8      s4, s5
    addi.u64   s0, s0, 1
    jmp        L0
L1: ret
end
```

The type is in the mnemonic (`.u32`), the byte order is static (`.le`), each
checked operation raises on its own (`addi` raises `Overflow`, `ld` raises
`OutOfBounds`), and the loop ticks the step counter once per iteration.

## 5. Prior art

| Source                    | Borrowed                                              | Changed                                              | Reason                                     |
|---------------------------|-------------------------------------------------------|------------------------------------------------------|--------------------------------------------|
| Lua 5 VM                  | register machine; call arguments in contiguous registers; results written over them | 64-bit words, 16-bit operands; two register banks | Type-specialised opcodes need opcode space; handles must be found for reference counting. |
| WebAssembly               | typed opcodes (`i32.add`); validation before execution; no value tags | registers instead of an operand stack; unstructured jumps | Fewer dispatches per Mend operation; lowering produces a CFG anyway. |
| JVM                       | constant pool; verifier; line-number tables; balanced monitors | structural verification only; type checking only in debug builds | The producer is the trusted compiler; the verifier guards memory safety against corrupted `.mbc` files. |
| Swift / CPython           | reference counting; copy-on-write on a uniqueness check | no cycle collector                                  | Mend values cannot form cycles (§7.4).      |
| Pike VM / backtracking regex | ordered `split`, `save` slots, leftmost-first matching | bounded gaps only; failure memoisation               | Mend §13.2 requires ordered alternation and shortest-first gaps; memoisation bounds cost. |
| Emacs markers             | positions adjusted by edits                           | two marker kinds (`pos`, `lim`) with Mend's table    | Mend §14.4 defines the adjustment.           |

## 6. Execution model

### 6.1 Phases

1. **Compile** (Mend §18.1): source → Typed Core → const-eval → lowering →
   MBC program.
2. **Verify** (§17), in debug builds.
3. **Bind parameters** into globals from `-D` and defaults (§16.1).
4. **Run `init`**: the top-level statements (Mend §18.2).
5. **Load the input** into the content buffer (§13.1).
6. **Run `main`.**
7. **Commit** the output (Mend §5.2), or report the error.

### 6.2 Interpretation

`mend` compiles at the start of every run and interprets the result. The interpreter pre-decodes instruction words into an internal form
and dispatches with one loop. The pre-decoded form is not specified; only the
behaviour of the encoded instructions is.

### 6.3 Observable behaviour

An implementation, including any optimisation (§18.4), must preserve:

- the output bytes, and whether output is written;
- the bytes written to stdout and stderr, in order;
- the exit code;
- the error report: kind, message, span, call stack, content offset;
- the step at which `StepLimit` fires.

Register contents, reference counts, allocation and instruction counts are
not observable.

## 7. Values and registers

### 7.1 Register banks

Each call frame has two banks, sized by the function header (§10):

| Bank   | Names            | Holds                                     | Initial value |
|--------|------------------|-------------------------------------------|---------------|
| scalar | `s0` … `s65534`  | one 64-bit word                           | `0`           |
| handle | `h0` … `h65534`  | a reference to a heap object, or null     | null          |

The program also has two global banks, `gs0…` and `gh0…` (§7.5).

### 7.2 Scalar representation

Every instruction writes scalars in **normal form** and may assume normal-form
inputs.

| Type                          | Normal form in one word                          |
|-------------------------------|--------------------------------------------------|
| `u8` `u16` `u32` `u64`        | zero-extended                                    |
| `i8` `i16` `i32` `i64`        | sign-extended                                    |
| `bool`                        | `0` or `1`                                       |
| `char`                        | the scalar value, zero-extended                  |
| `f32`                         | IEEE bits in the low 32 bits; high bits zero     |
| `f64`                         | IEEE bits                                        |
| `Endian`                      | `0` = `le`, `1` = `be`                            |
| `u128` `i128`                 | two words: low word in `sN`, high word in `sN+1` |

`usize` and `isize` are `u64` and `i64` (Mend §8).

### 7.3 Layouts

Every Mend type has a **layout**: a number of scalar words and a number of
handles. A value occupies a contiguous **run** in each bank, so an operand
naming a compound value is a pair of base registers (`sB`, `hB`).

| Type                                   | Scalar words            | Handles       | Order                                   |
|----------------------------------------|-------------------------|---------------|-----------------------------------------|
| `bool` `char` 8–64-bit ints, floats, `Endian` | 1                | 0             | —                                       |
| `u128` `i128`                          | 2                       | 0             | low, high                               |
| `()` `!`                               | 0                       | 0             | —                                       |
| `str`, `[T]`, `[T; N]`, `Pat`, function types | 0                | 1             | —                                       |
| `(T1, …, Tn)`                          | Σ s(Ti)                 | Σ h(Ti)       | fields in order, each bank separately   |
| `Opt[T]`                               | 1 + s(T)                | h(T)          | tag (`0` = `none`), then `T`            |
| `Range[T]`                             | 2·s(T)                  | 0             | start, end                              |
| `Match`                                | 2                       | 0             | start, end                              |
| `Match[P]`                             | 2 + Σ capture words     | 0             | start, end, then each capture of `P` in source order: `start, end`, or `tag, start, end` if optional (Mend §13.3) |

Rules:
- A `none` has zero scalar payload words and null payload handles, so every
  register state is deterministic.
- An empty run (zero words) may name any register; its index is ignored.
- `[T; N]` and `[T]` share one representation. The coercion `[T; N] → [T]`
  is a no-op.

### 7.4 Heap objects and handles

| Object    | Contents                                               | Mutability                    |
|-----------|--------------------------------------------------------|-------------------------------|
| `Str`     | UTF-8 bytes                                            | immutable                     |
| `Seq`     | element layout + elements                              | copy-on-write                 |
| `Closure` | function id + captured scalar words + captured handles | immutable                     |
| `Pat`     | a matcher (§14)                                        | immutable                     |
| `Scanner` | matcher + window + markers (§13.5)                     | internal; never a Mend value  |

**Element storage.** A `Seq` whose element layout is one scalar stores
elements packed at their encoded width (`bool`, `u8`, `i8` → 1 byte; `char`,
`u32`, `f32` → 4; `u128` → 16; …). `[u8]` is a plain byte vector. Other
element layouts are stored as records of `s` words and `h` handles.

**Reference counting.**
- Handles are reference counted. Mend values cannot form cycles: strings,
  closures and patterns are immutable, and sequences are copied on write, so
  no object can come to contain a handle to itself. Reference counting
  therefore reclaims everything, deterministically.
- Mutating sequence instructions (§9.8) mutate in place when the count is 1
  and copy first otherwise.
- `hmov` copies a handle (count + 1). `htake` moves it (source becomes null).
  `hdrop` releases it. Lowering uses `htake` on the last use of a binding so
  that a later mutation finds the object unique.
- `call`, `callc`, `intr` and `ret` move their handle operands (§10). All
  other instructions read handle operands without consuming them; storing a
  handle into an object increments its count.

**Null.** A null handle is valid only in an unused register or in a `none`
payload. Using a null handle as an operand is an internal error (§15.2).

### 7.5 Globals

Top-level bindings and parameters live in the global banks `gs` and `gh`,
sized in the program header. `init` writes them, `main` reads them, and the
runtime writes parameters before `init` runs (§16.1).

## 8. Encoding

### 8.1 Words and formats

An instruction is one or two little-endian 64-bit words. A word is four
16-bit fields:

```
 63        48 47        32 31        16 15         0
+------------+------------+------------+------------+
|     c      |     b      |     a      |     op     |
+------------+------------+------------+------------+
```

| Format | Words | Fields                                                           |
|--------|-------|------------------------------------------------------------------|
| `ABC`  | 1     | `op a b c`                                                       |
| `AJ`   | 1     | `op a j`, where `j = (c << 16 \| b)` as `i32`                    |
| `ABCX` | 2     | `op a b c`, then an extension word `d e f g` (`d` in bits 0–15)  |

- `pc` counts words. A jump target is `pc_of_jump + j`.
- `0xFFFF` in an optional operand means "none" (written `-`).
- Registers, table indices and function ids are 16-bit, which sets the limits
  in §20.

### 8.2 Opcodes

```
 15                    6 5    4 3      0
+-----------------------+------+--------+
|        family         | mod  |  type  |
+-----------------------+------+--------+
```

- **family** (10 bits) selects the operation (§9).
- **type** (4 bits) is the operand type code (§8.3). Families without a type
  use `0`.
- **mod** (2 bits) is a family-specific modifier: the endian mode for content
  and encoding families, the pattern source for search families, else `0`.

### 8.3 Type codes and modifiers

| Code | Type   | Code | Type   | Code | Type   |
|------|--------|------|--------|------|--------|
| `0`  | `u8`   | `5`  | `i8`   | `A`  | `f32`  |
| `1`  | `u16`  | `6`  | `i16`  | `B`  | `f64`  |
| `2`  | `u32`  | `7`  | `i32`  | `C`  | `bool` |
| `3`  | `u64`  | `8`  | `i64`  | `D`  | `char` |
| `4`  | `u128` | `9`  | `i128` | `E` `F` | reserved |

`Endian` values use code `0` (`u8`) where a type code is needed.

| Endian mode `mod` | Mnemonic | Byte order                                         |
|-------------------|----------|----------------------------------------------------|
| `0`               | `.le`    | little-endian                                      |
| `1`               | `.be`    | big-endian                                         |
| `2`               | `.cur`   | the runtime's current byte order (§13.2)           |
| `3`               | `.reg`   | the `Endian` in a register: field `a` for `setend`, the extension word's `e` field for `enc` |

For 1-byte types the mode is `0` and is omitted in assembly (`ld.u8`).

| Pattern source `mod` | Pattern operand              | Match layout written       |
|----------------------|------------------------------|----------------------------|
| `0`                  | pattern table index `pN`     | `Match[P]` (with captures) |
| `1`                  | handle register `hN` (`Pat`) | `Match` (start, end)       |

### 8.4 Tables

Instructions refer to program tables by 16-bit index (§16.1).

| Prefix | Table             | Entry                                                          |
|--------|-------------------|----------------------------------------------------------------|
| `k`    | scalar constants  | type code + one or two words                                   |
| `kh`   | handle constants  | `Str`, `Seq` (type + elements), or closure (function + constant captures) |
| `t`    | type descriptors  | a Mend type; gives its layout, element layout, encoding and comparison |
| `p`    | patterns          | a matcher (§14) and its capture layout                         |
| `f`    | format descriptors| pieces: literal (`kh`) or argument (type `t` + spec, Mend §16)  |
| `d`    | detail descriptors| values to show in an error report: `(label, t, register)` items, an optional pattern `p` |
| `w`    | switch tables     | default target + sorted `(value, target)` pairs                |
| `@`    | functions         | §10                                                            |
| `$`    | intrinsics        | §12                                                            |

Type descriptor kinds: scalar (type code), unit, `str`, `[T]`, `[T; N]`,
tuple, `Opt[T]`, `Range[T]`, `Match`, `Match[p]`, `Endian`, `Pat`, function
`(T…) -> R`.

### 8.5 Assembly notation

The text form is used in this RFC, in `--emit asm` (Mend §5.1) and in tests. The
assembler and disassembler round-trip. The grammar is in §22.

- Mnemonic: `family[.mode][.type]`, e.g. `ld.be.u16`, `add.i32`, `find`.
- Operands: `s3`, `h1`, `gs0`, `gh2`, table references (`k4`, `kh0`, `t2`,
  `p1`, `f0`, `d0`, `w0`), `@name`, `$name[T]`, labels `L3`, immediates, and
  `-` for none.
- The search families infer `mod` from the pattern operand (`p…` or `h…`).

## 9. Instruction set

Notation: `sD`/`hD` destination, `sA`/`hA`/`sB`/… sources. `sR` is a range run
`sR, sR+1`. "Run" operands take the width of their type's layout (§7.3). All
integer instructions also accept `u128`/`i128`, whose operands name pairs.

### 9.1 Moves and constants

| Fam   | Mnemonic        | Fmt  | Operands             | Effect                                         |
|-------|-----------------|------|----------------------|------------------------------------------------|
| `000` | `nop`           | ABC  | —                    | nothing                                        |
| `001` | `mov`           | ABC  | `sD, sA`             | `sD ← sA`                                      |
| `002` | `movn`          | ABC  | `sD, sA, n`          | copy `n` words; runs may overlap (memmove)     |
| `003` | `ldi.T`         | AJ   | `sD, imm32`          | sign-extend `imm32`, convert to `T` (integer, `bool`, `char`); the verifier checks it fits |
| `004` | `ldk.T`         | ABC  | `sD, kN`             | load a scalar constant (one or two words)      |
| `005` | `hldk`          | ABC  | `hD, khN`            | load a handle constant                         |
| `006` | `hmov`          | ABC  | `hD, hA`             | copy handle                                    |
| `007` | `htake`         | ABC  | `hD, hA`             | move handle; `hA` ← null                       |
| `008` | `hdrop`         | ABC  | `hA`                 | release; `hA` ← null                           |
| `009` | `ldfn`          | ABC  | `hD, @f`             | closure value for a top-level function (no captures; one shared object per function) |
| `00A` | `ldpat`         | ABC  | `hD, pN`             | `Pat` value for a pattern constant             |
| `00B` | `gget`          | ABC  | `sD, gsN`            | read a global scalar                           |
| `00C` | `gset`          | ABC  | `gsN, sA`            | write a global scalar                          |
| `00D` | `ghget`         | ABC  | `hD, ghN`            | read a global handle (copy)                    |
| `00E` | `ghset`         | ABC  | `ghN, hA`            | write a global handle (copy)                   |
| `00F` | `clr`           | ABC  | `sD, n`              | zero `n` words                                 |

Floats and 64-bit values that do not fit `imm32` use `ldk`.

### 9.2 Integer arithmetic

`T` is an integer type unless noted. Checked forms raise as listed; they
follow Rust semantics otherwise (truncating division, `%` takes the sign of
the dividend).

| Fam   | Mnemonic   | Fmt | Operands          | Effect                                   | Raises |
|-------|------------|-----|-------------------|------------------------------------------|--------|
| `020` | `add.T`    | ABC | `sD, sA, sB`      | `a + b`                                  | `Overflow` |
| `021` | `sub.T`    | ABC | `sD, sA, sB`      | `a - b`                                  | `Overflow` |
| `022` | `mul.T`    | ABC | `sD, sA, sB`      | `a * b`                                  | `Overflow` |
| `023` | `div.T`    | ABC | `sD, sA, sB`      | `a / b`                                  | `DivisionByZero`; `Overflow` on `MIN / -1` |
| `024` | `rem.T`    | ABC | `sD, sA, sB`      | `a % b`                                  | `DivisionByZero`; `Overflow` on `MIN % -1` |
| `025` | `addw.T`   | ABC | `sD, sA, sB`      | wrapping `+%`                            | — |
| `026` | `subw.T`   | ABC | `sD, sA, sB`      | wrapping `-%`                            | — |
| `027` | `mulw.T`   | ABC | `sD, sA, sB`      | wrapping `*%`                            | — |
| `028` | `adds.T`   | ABC | `sD, sA, sB`      | saturating add                           | — |
| `029` | `subs.T`   | ABC | `sD, sA, sB`      | saturating sub                           | — |
| `02A` | `muls.T`   | ABC | `sD, sA, sB`      | saturating mul                           | — |
| `02B` | `addc.T`   | ABC | `sD, sA, sB`      | `checked_add` → `Opt[T]` at `sD`         | — |
| `02C` | `subc.T`   | ABC | `sD, sA, sB`      | `checked_sub` → `Opt[T]`                 | — |
| `02D` | `mulc.T`   | ABC | `sD, sA, sB`      | `checked_mul` → `Opt[T]`                 | — |
| `02E` | `divc.T`   | ABC | `sD, sA, sB`      | `checked_div` → `Opt[T]`                 | — |
| `02F` | `shlc.T`   | ABC | `sD, sA, sB:u32`  | `checked_shl` → `Opt[T]`                 | — |
| `030` | `shrc.T`   | ABC | `sD, sA, sB:u32`  | `checked_shr` → `Opt[T]`                 | — |
| `031` | `neg.T`    | ABC | `sD, sA`          | `-a` (signed)                            | `Overflow` on `MIN` |
| `032` | `abs.T`    | ABC | `sD, sA`          | `abs` (signed)                           | `Overflow` on `MIN` |
| `033` | `pow.T`    | ABC | `sD, sA, sB:u32`  | `a.pow(b)`                               | `Overflow` |
| `034` | `and.T`    | ABC | `sD, sA, sB`      | `&` (also `bool`)                        | — |
| `035` | `or.T`     | ABC | `sD, sA, sB`      | `\|` (also `bool`)                       | — |
| `036` | `xor.T`    | ABC | `sD, sA, sB`      | `^` (also `bool`)                        | — |
| `037` | `not.T`    | ABC | `sD, sA`          | bitwise `!`; logical on `bool`           | — |
| `038` | `shl.T`    | ABC | `sD, sA, sB:u32`  | `a << b`                                 | `Overflow` if `b ≥ BITS` |
| `039` | `shr.T`    | ABC | `sD, sA, sB:u32`  | `a >> b`; arithmetic if signed           | `Overflow` if `b ≥ BITS` |
| `03A` | `rotl.T`   | ABC | `sD, sA, sB:u32`  | `rotate_left`                            | — |
| `03B` | `rotr.T`   | ABC | `sD, sA, sB:u32`  | `rotate_right`                           | — |
| `03C` | `bswap.T`  | ABC | `sD, sA`          | `swap_bytes`                             | — |
| `03D` | `rbit.T`   | ABC | `sD, sA`          | `reverse_bits`                           | — |
| `03E` | `popcnt.T` | ABC | `sD:u32, sA`      | `count_ones`                             | — |
| `03F` | `clz.T`    | ABC | `sD:u32, sA`      | `leading_zeros`                          | — |
| `040` | `ctz.T`    | ABC | `sD:u32, sA`      | `trailing_zeros`                         | — |
| `041` | `min.T`    | ABC | `sD, sA, sB`      | minimum (also floats, §9.3)              | — |
| `042` | `max.T`    | ABC | `sD, sA, sB`      | maximum (also floats)                    | — |
| `043` | `addi.T`   | ABC | `sD, sA, imm16`   | `a + c` with `c` as `i16`, checked       | `Overflow` |

Shift amounts are always `u32`. Lowering converts other amount types with
`cvtx.u32` (§9.5), which raises `Overflow` for a negative or too-large
amount, the same kind the shift itself raises (Mend §10.2, §15).
`count_zeros` lowers to `BITS - popcnt`.

### 9.3 Float arithmetic

`add sub mul div rem neg abs min max` with type `f32` or `f64` follow IEEE 754
and Rust: they never raise. `rem` is Rust's `%` on floats; `min`/`max` return
the non-NaN operand. The errors in Mend §10.2 apply to integers only.

### 9.4 Comparison

| Fam   | Mnemonic  | Fmt  | Operands             | Effect                                                 |
|-------|-----------|------|----------------------|--------------------------------------------------------|
| `060` | `eq.T`    | ABC  | `sD:bool, sA, sB`    | `a == b` (IEEE for floats)                            |
| `061` | `ne.T`    | ABC  | `sD:bool, sA, sB`    | `a != b`                                               |
| `062` | `lt.T`    | ABC  | `sD:bool, sA, sB`    | `a < b`; signedness from `T`                           |
| `063` | `le.T`    | ABC  | `sD:bool, sA, sB`    | `a <= b`                                               |
| `064` | `gt.T`    | ABC  | `sD:bool, sA, sB`    | `a > b`                                                |
| `065` | `ge.T`    | ABC  | `sD:bool, sA, sB`    | `a >= b`                                               |
| `066` | `heq`     | ABCX | `sD:bool, hA, hB, t` | deep equality of two values of the handle type `t` (`str`, sequences) |
| `067` | `hcmp`    | ABCX | `sD:i32, hA, hB, t`  | lexicographic order: `-1`, `0`, `1`                    |

`T` here includes `bool`, `char` and `Endian` (as `u8`). Tuples, `Opt`,
`Range` and `Match` compare component-wise in lowered code. `heq` on elements
of type `Opt` compares tags first and ignores the payload of `none`.

### 9.5 Conversion

| Fam   | Mnemonic  | Fmt | Operands       | Effect                                                    | Raises |
|-------|-----------|-----|----------------|-----------------------------------------------------------|--------|
| `070` | `cvt.D`   | ABC | `sD, sA, S`    | `a as D` from source type code `S` (Rust `as` rules: truncate/extend, saturating float→int with NaN → 0, `u8 → char`, `char → int`, `bool → int`) | — |
| `071` | `cvtc.D`  | ABC | `sD, sA, S`    | `try_into[D]` → `Opt[D]` at `sD`; integer→`char` checks validity | — |
| `072` | `cvtx.D`  | ABC | `sD, sA, S`    | lossless conversion or error                              | `Overflow` |

### 9.6 Control flow

| Fam   | Mnemonic       | Fmt | Operands       | Effect                                                 |
|-------|----------------|-----|----------------|--------------------------------------------------------|
| `080` | `jmp`          | AJ  | `L`            | jump                                                   |
| `081` | `brt`          | AJ  | `sA, L`        | jump if `sA` (bool) is `1`                             |
| `082` | `brf`          | AJ  | `sA, L`        | jump if `sA` is `0`                                    |
| `083` | `switch.T`     | ABC | `sA, wN`       | jump to the target for `sA`, else the default; `T` ≤ 64 bits |
| `084` | `tick`         | ABC | —              | count one step (§9.12)                                 |
| `085` | `unreachable`  | ABC | —              | internal error (§15.2); ends a path of type `!`        |

### 9.7 Calls, closures, intrinsics

| Fam   | Mnemonic  | Fmt  | Operands                   | Effect                                          |
|-------|-----------|------|----------------------------|-------------------------------------------------|
| `090` | `call`    | ABC  | `sB, hB, @f`               | call `f`; arguments at `sB…`/`hB…`, results written back there (§10); counts one step |
| `091` | `callc`   | ABCX | `sB, hB, hF, t`            | call the closure in `hF`, whose type is the function type `t`; counts one step |
| `092` | `ret`     | ABC  | `sA, hA`                   | return the result runs at `sA…`/`hA…`           |
| `093` | `mkclos`  | ABCX | `hD, @f, sC, hC`           | closure of `f` capturing the runs at `sC…`/`hC…` (counts from `f`'s header) |
| `094` | `intr`    | ABCX | `$id[t], sB, hB`           | intrinsic call (§12); same argument and result convention as `call`; no frame, no step |

### 9.8 Sequences and strings

Element type `t` is given in the extension word. Elements are passed as runs
`(sV, hV)`. Mutating instructions copy on write (§7.4).

| Fam   | Mnemonic  | Fmt  | Operands                     | Effect                                     | Raises |
|-------|-----------|------|------------------------------|--------------------------------------------|--------|
| `0A0` | `snew`    | ABC  | `hD, sCap, t`                | empty `[t]` with capacity `sCap`           | — |
| `0A1` | `sfill`   | ABCX | `hD, sV, hV, sN, t`          | `[v; n]`                                    | — |
| `0A2` | `slen`    | ABC  | `sD:u64, hA`                 | element count; byte length for `str`       | — |
| `0A3` | `sget`    | ABCX | `sD, hD, hA, sI, t`          | element `i`                                | `OutOfBounds` |
| `0A4` | `sset`    | ABCX | `hA, sI, sV, hV, t`          | `a[i] = v`                                  | `OutOfBounds` |
| `0A5` | `spush`   | ABCX | `hA, sV, hV, t`              | `push`                                     | — |
| `0A6` | `spop`    | ABCX | `sD, hD, hA, t`              | `pop` → `Opt[t]`                            | — |
| `0A7` | `sins`    | ABCX | `hA, sI, sV, hV, t`          | `insert(i, v)`; `i == len` appends          | `OutOfBounds` |
| `0A8` | `srem`    | ABCX | `sD, hD, hA, sI, t`          | `remove(i)` → element                       | `OutOfBounds` |
| `0A9` | `scat`    | ABC  | `hD, hA, hB`                 | `++` on sequences or strings               | — |
| `0AA` | `sslice`  | ABC  | `hD, hA, sR`                 | sub-sequence; on `str`, the range must fall on char boundaries | `OutOfBounds` |

### 9.9 Content access

`E` is the endian mode (`le`, `be`, `cur`). `T` is a scalar type. Every access
is bounds-checked against the current length, which inside `batch` is the
snapshot length (§13.4). A range with `start > end`, or an `end` past the
length, raises `OutOfBounds`. Offset arithmetic overflow counts as out of
bounds.

| Fam   | Mnemonic     | Fmt | Operands          | Effect                                            | Raises |
|-------|--------------|-----|-------------------|---------------------------------------------------|--------|
| `0C0` | `ld.E.T`     | ABC | `sD, sAt`         | `T[at]`                                            | `OutOfBounds`; `InvalidValue` for `bool`/`char` |
| `0C1` | `ldo.E.T`    | ABC | `sD, sAt`         | `T.get(at)` → `Opt[T]`                             | `InvalidValue` |
| `0C2` | `st.E.T`     | ABC | `sAt, sV`         | `T[at] = v`; in `batch`, queued (§13.4)            | `OutOfBounds` |
| `0C3` | `ldv.E.T`    | ABC | `hD, sR`          | `T[r]` → `[T]`                                     | `OutOfBounds`; `LengthMismatch` if `r.len()` is not a multiple of `size_of(T)`; `InvalidValue` |
| `0C4` | `stv.E.T`    | ABC | `sR, hV`          | `T[r] = v`                                         | `OutOfBounds`; `LengthMismatch` |
| `0C5` | `ldd.E`      | ABC | `hD, sAt, t`      | decode a nested array `t` (e.g. `[[u8; 4]; 2]`)    | as `ldv` |
| `0C6` | `std.E`      | ABC | `sAt, hV, t`      | encode a nested array `t` in place                 | `OutOfBounds` |
| `0C7` | `len`        | ABC | `sD:u64`          | `len()`                                            | — |
| `0C8` | `getend`     | ABC | `sD:Endian`       | `endianness()`                                     | — |
| `0C9` | `setend.E`   | ABC | — or `sA` (`.reg`) | `endian e;` sets the current byte order (§13.2); `E` is `le`, `be` or `reg` | `BatchConflict` inside `batch` |
| `0CA` | `modified`   | ABC | `sD:bool`         | `modified()` (§13.1)                               | — |

`[T; N][at]` lowers to `ldv` over `at..+N·size_of(T)`.

### 9.10 Edits and batch

`sR` is a `Range` run; `hB` is a `[u8]` handle.

| Fam   | Mnemonic  | Fmt  | Operands              | Effect                                               | Raises |
|-------|-----------|------|-----------------------|------------------------------------------------------|--------|
| `0D0` | `edrep`   | ABC  | `sR, hB`              | replace bytes in `r` with `b`                        | `OutOfBounds` |
| `0D1` | `edins`   | ABC  | `sAt, hB`             | insert `b` at `at`; `at == len` appends              | `OutOfBounds` |
| `0D2` | `eddel`   | ABC  | `sR`                  | delete bytes in `r`                                  | `OutOfBounds` |
| `0D3` | `bbeg`    | ABC  | —                     | begin a batch (§13.4)                                | `BatchConflict` if a batch is active |
| `0D4` | `bend`    | ABC  | —                     | check and apply the queue (§13.4)                    | `BatchConflict` |
| `0D5` | `enc.E`   | ABCX | `hD, sV, hV, t`       | encode a value of encodable type `t` to `[u8]` (Mend §12.4); `.reg` reads the `Endian` from `e` | — |

Outside `batch` each edit applies immediately and adjusts markers (§13.3).
Inside, it is checked against the snapshot and queued.

### 9.11 Search and scan

`P` is a pattern operand (§8.3). `sW` is a window run `start, end`; lowering
computes it from `from:`, `before:` and `within:`. All search instructions
read the current content (the snapshot inside `batch`).

| Fam   | Mnemonic   | Fmt  | Operands        | Effect                                                        | Raises |
|-------|------------|------|-----------------|---------------------------------------------------------------|--------|
| `0E0` | `find`     | ABC  | `sD, P, sW`     | first match in `w` → `Opt[Match…]`                            | `OutOfBounds` for a bad window |
| `0E1` | `rfind`    | ABC  | `sD, P, sW`     | last match of the forward sequence in `w` (§14.3)             | `OutOfBounds` |
| `0E2` | `matat`    | ABC  | `sD, P, sAt`    | match anchored at `at`, window `at..len` → `Opt[Match…]`      | `OutOfBounds` if `at > len` |
| `0E3` | `count`    | ABC  | `sD:u64, P, sW` | number of matches                                             | `OutOfBounds` |
| `0E4` | `findall`  | ABC  | `hD, P, sW`     | `[Match…]` snapshot                                           | `OutOfBounds` |
| `0E5` | `scan`     | ABC  | `hD, P, sW`     | new scanner with markers `pos = w.start`, `lim = w.end` (§13.5) | `OutOfBounds` |
| `0E6` | `snext`    | ABC  | `sD, hS, P`     | next match in `pos..lim` → `Opt[Match…]`; sets `pos` to its end | — |

`snext`'s `P` must be the scanner's pattern (or `-` for a `Pat` scanner); it
fixes the layout written to `sD`.

### 9.12 Errors, formatting, steps

| Fam   | Mnemonic  | Fmt  | Operands               | Effect                                                   |
|-------|-----------|------|------------------------|----------------------------------------------------------|
| `0F0` | `raise`   | ABC  | `kind, hMsg, dN`       | raise the error `kind` (§15.1) with optional message and detail |
| `0F1` | `fmt`     | ABCX | `hD, sB, hB, fN`       | format the argument runs at `sB…`/`hB…` with `fN` → `str` |

**Steps.** `tick`, `call` and `callc` each add one to the step counter. With
`--max-steps n`, the step that would make the counter exceed `n` raises
`StepLimit` instead. Without a limit, counting has no effect. Lowering emits
`tick` once per executed body of a source-level `for`; loops created by
desugaring (edit scans, sequence methods) do not tick, but the calls they make
do.

## 10. Calling convention

**Function header** (§16.1): name, signature type `t`, frame size `s:N h:M`,
capture counts. Frame layout:

| Scalar bank                       | Handle bank                       |
|-----------------------------------|-----------------------------------|
| `s0…` parameters (flattened, in order) | `h0…` parameters             |
| then captures (closures only)     | then captures                     |
| then locals and temporaries       | then locals and temporaries       |

**`call sB, hB, @f`**
1. Count one step. If the call depth would exceed the limit (§20), raise
   `StackOverflow`.
2. Create a frame with all scalars `0` and all handles null.
3. Copy the parameter scalar run from `sB…`; move the parameter handle run
   from `hB…` (the caller's registers become null).
4. Run `f`.

**`ret sA, hA`**
1. Copy the result scalar run to the caller's `sB…`; move the result handle
   run to the caller's `hB…`, releasing what those registers held.
2. Release the frame's remaining handles and pop it.

**`callc sB, hB, hF, t`** is `call` on the closure's function, plus: copy the
closure's captured scalars and handles into the frame after the parameters.
`hF` must lie outside the argument handle run. If the closure's function does
not have signature `t`, it is an internal error.

`init`, `main` and constant functions have signature `() -> ()` or
`() -> T`. Returning from `main` ends the run successfully.

## 11. Closures

- A lambda lowers to a function whose header declares its capture counts,
  plus a `mkclos` at the lambda's position, which copies the captured runs.
  This implements capture by value at creation time (Mend §11).
- Captures are read-only registers in the callee; the verifier rejects writes
  to them.
- A function value for a top-level function, or for a lambda with no
  captures, is `ldfn`: one shared closure object per function, no
  allocation after the first use.
- Constant closures (a constant whose value is a function) are `kh` entries.
- Calls to a statically known function use `call`, never `callc`.

## 12. Intrinsics

Intrinsic ids are stable: an id is never reused or renumbered, and new
intrinsics take new ids. Arguments and results follow §10: each argument's
layout is appended, in order, to the scalar and handle runs at `sB`/`hB`, and
results are written back there. Handle arguments are moved, so a unique
sequence can be transformed in place.

| Id     | Name              | `[t]` | Arguments            | Result          | Raises                              | Content |
|--------|-------------------|-------|----------------------|-----------------|-------------------------------------|---------|
| `0001` | `input_path`      | —     | —                    | `str`           | —                                   | yes |
| `0002` | `output_path`     | —     | —                    | `str`           | —                                   | yes |
| `0010` | `print`           | —     | `str`                | —               | —                                   | no  |
| `0011` | `eprint`          | —     | `str`                | —               | —                                   | no  |
| `0020` | `str_contains`    | —     | `str, str`           | `bool`          | —                                   | no  |
| `0021` | `str_starts_with` | —     | `str, str`           | `bool`          | —                                   | no  |
| `0022` | `str_ends_with`   | —     | `str, str`           | `bool`          | —                                   | no  |
| `0023` | `str_bytes`       | —     | `str`                | `[u8]`          | —                                   | no  |
| `0024` | `str_chars`       | —     | `str`                | `[char]`        | —                                   | no  |
| `0025` | `str_char_at`     | —     | `str, u64`           | `(char, u64)`   | `OutOfBounds` unless a char starts there | no  |
| `0026` | `str_from_utf8`   | —     | `[u8]`               | `Opt[str]`      | —                                   | no  |
| `0027` | `str_parse`       | `T`   | `str`                | `Opt[T]`        | —                                   | no  |
| `0030` | `seq_reverse`     | `T`   | `[T]`                | `[T]`           | —                                   | no  |
| `0031` | `seq_repeat`      | `T`   | `[T], u64`           | `[T]`           | `Overflow`                          | no  |
| `0032` | `seq_contains`    | `T`   | `[T], T`             | `bool`          | —                                   | no  |
| `0033` | `seq_position`    | `T`   | `[T], T`             | `Opt[u64]`      | —                                   | no  |
| `0040` | `align_up`        | `T`   | `T, T`               | `T`             | `Overflow`; `InvalidValue` if `a` is not a power of two | no  |
| `0041` | `align_down`      | `T`   | `T, T`               | `T`             | `InvalidValue` if `a` is not a power of two | no  |
| `0042` | `clamp`           | `T`   | `T, T, T`            | `T`             | `InvalidValue` if `lo > hi`        | no  |
| `0050` | `to_bytes`        | `T`   | `T, Endian`          | `[u8]`          | —                                   | no  |
| `0051` | `from_bytes`      | `T`   | `[u8], Endian`       | `T`             | `LengthMismatch`                    | no  |
| `0060` | `pat_bytes`       | —     | `[u8]`               | `Pat`           | `InvalidValue` if empty            | no  |
| `0070` | `crc32`           | —     | `[u8]`               | `u32`           | —                                   | no  |
| `0071` | `crc32c`          | —     | `[u8]`               | `u32`           | —                                   | no  |
| `0072` | `adler32`         | —     | `[u8]`               | `u32`           | —                                   | no  |
| `0073` | `fnv1a32`         | —     | `[u8]`               | `u32`           | —                                   | no  |
| `0074` | `fnv1a64`         | —     | `[u8]`               | `u64`           | —                                   | no  |

Everything else in Mend §16 lowers to instructions: `len`, `endianness` and
`modified` are opcodes; integer methods are §9.2 opcodes or short sequences
(`is_power_of_two` is `popcnt == 1`; `bit`, `with_bit`, `bits`, `with_bits`
are shifts and masks); `Opt` methods, `map`, `filter`, `fold`, `any`, `all`,
`sum` and `enumerate` are loops and branches (§18.2).

## 13. Content runtime

### 13.1 State

| State        | Meaning                                                              |
|--------------|----------------------------------------------------------------------|
| buffer       | the content bytes                                                    |
| input        | the input bytes, immutable; the buffer shares them until the first write |
| `E`          | current byte order                                                   |
| `dirty`      | set by every applied edit or store; cleared by `modified`            |
| `differs`    | the last result of `modified`; starts false                          |
| batch        | inactive, or active with an edit queue                               |
| markers      | the `(pos, lim)` pairs of live scanners                              |
| loaded       | false during `init`; content opcodes check it (internal error)       |

`modified` (Mend §16): if `dirty`, set `differs` to whether the buffer differs
from the input in length or in any byte, and clear `dirty`; then return
`differs`. Writing identical bytes leaves the result false. The comparison
costs `O(len)` at most once per write sequence, and `--dry-run` reuses the
input for its diff.

### 13.2 Byte order

- `E` starts as the value of the last top-level `endian` statement, stored in
  the program header, or `le`.
- If no function contains an `endian` statement, `E` never changes, and
  lowering resolves every implicit byte order to `.le` or `.be` statically.
  Otherwise implicit accesses use `.cur`.
- `setend` inside an active batch raises `BatchConflict` (Mend §15). This covers
  `endian` reached through a call, which Mend §14.3 cannot reject statically.

### 13.3 Edits outside `batch`

An edit replaces `[a, b)` with `n` bytes (an insertion has `a == b`, a
deletion has `n == 0`):

1. Check `a <= b <= len`, else `OutOfBounds`.
2. Splice the buffer. Cost is `O(len)` per edit; many edits belong in a
   `batch` (§13.4), which costs `O(len + total)`.
3. Adjust every live marker by the table in Mend §14.4.
4. Set `dirty`.

A store `st` is a replacement with `n == b - a`; it cannot move markers.

### 13.4 Batch

`bbeg` activates a batch with an empty queue. While it is active:

- Reads see the buffer, which edits do not change, so the buffer **is** the
  snapshot. No copy is taken.
- Every edit and store is bounds-checked against the buffer and appended to
  the queue as `(a, b, bytes, seq, pc)`; `seq` is the program order and `pc`
  locates the instruction for reports.
- Markers do not move.

`bend`:

1. Sort the queue by `a`; at equal `a`, insertions (`a == b`) before
   non-empty ranges; insertions at the same point by `seq`.
2. Raise `BatchConflict`, naming both entries, if two non-empty ranges
   overlap or an insertion point `x` lies strictly inside a non-empty range
   (`a < x < b`). This is Mend §14.3 rule 3.
3. Rebuild the buffer in one forward pass: copy unchanged bytes, emit each
   entry's bytes in sorted order. An empty deletion has no effect.
4. Adjust markers as if the entries were applied one at a time in the
   reverse of the sorted order. Applying back-to-front keeps each entry's
   snapshot offsets valid, so the result equals step 3.
5. Set `dirty` if the queue was non-empty, and deactivate the batch.

The verifier guarantees that `bend` runs in the same frame as `bbeg` on every
non-error path (§17). A `bbeg` while a batch is active can only happen
through a call and raises `BatchConflict` (Mend §15).

### 13.5 Markers and scanners

- `scan` creates a `Scanner` holding a matcher, `pos` and `lim`, and
  registers its markers with the runtime.
- `snext` searches `pos..lim` (§14.3). On a match `[ms, me)` it sets
  `pos = me` and returns the match. Patterns cannot match zero bytes, so
  `pos` always advances.
- Every applied edit adjusts every registered marker (§13.3, §13.4).
- Releasing the last handle to a scanner unregisters its markers. Reference
  counting makes this deterministic; there is no `scan_close`.

## 14. Pattern matchers

The front end delivers each pattern as one of two forms, with capture slots
and a proof that it cannot match zero bytes (Mend §18.3 item 5).

### 14.1 Fixed form

Used when the pattern has no alternation and no variable gap (the "DFA" case
of Mend §18.3).

- A length `L ≥ 1` and `L` byte tests `(value, mask)`: a byte `x` passes when
  `x & mask == value`. A literal byte has mask `FF`, `??` and fixed gaps
  have mask `00`, a nibble has `F0` or `0F`.
- Each capture is a static `(offset, length)`.

### 14.2 Program form

A backtracking program over the bytes at the current position `i`.

| Op            | Effect                                                                       |
|---------------|------------------------------------------------------------------------------|
| `test v m`    | fail unless `i < end` and `byte[i] & m == v`; then `i += 1`                  |
| `skip n`      | fail unless `i + n <= end`; then `i += n`                                    |
| `gap lo hi`   | try `i += k` for `k = lo, lo+1, …, hi` in order (shortest first), within `end` |
| `split L1 L2` | try `L1`; on failure, try `L2` (ordered alternation)                         |
| `jmp L`       | continue at `L`                                                              |
| `save k`      | set capture slot `k` to `i`; undone on backtrack                             |
| `match`       | success: the match is `[start, i)`                                           |

Text elements lower to `test` sequences. A capture `{c: …}` is
`save 2c … save 2c+1`. A capture whose slots are unset at `match` is `none`
(Mend §13.3).

**Cost bound.** The program has no back edges and no backreferences, so a
failure at `(op, i)` does not depend on the path taken there. The engine
memoises failed `(op, i)` pairs per start position, which bounds the work per
start to `ops × window`.

### 14.3 Search semantics

For window `[ws, we)`:
- Try start positions `ws, ws+1, …` in order. The first start with a match
  wins; within one start, the first match found by the ordered search wins.
  A match must end at or before `we`.
- Successive matches (`count`, `findall`, `rfind`, `snext`) start the next
  search at the previous match's end.
- `rfind` returns the last match of that forward sequence (Mend §13.4), so
  `last P` agrees with `findall`.
- The engine may use any acceleration (scanning for a fixed byte, SIMD) that
  returns the same match.

## 15. Errors and reports

### 15.1 Error kinds

| Code | Kind             | Raised by (this RFC)                                         |
|------|------------------|--------------------------------------------------------------|
| `1`  | `Assertion`      | `raise`                                                      |
| `2`  | `NotFound`       | `raise`                                                      |
| `3`  | `OutOfBounds`    | content access, edits, windows, `sget`/`sset`/…, `sslice`, `str_char_at` |
| `4`  | `Overflow`       | checked arithmetic, shifts, `cvtx`, `pow`, `abs`, `neg`, `seq_repeat`, `align_up` |
| `5`  | `DivisionByZero` | `div`, `rem`                                                 |
| `6`  | `InvalidValue`   | decoding `bool`/`char`; `align_up`, `align_down`, `clamp`, `pat_bytes` |
| `7`  | `LengthMismatch` | `stv`, `ldv`, `from_bytes`                                   |
| `8`  | `BatchConflict`  | `bend`, `bbeg`, `setend`                                     |
| `9`  | `StepLimit`      | `tick`, `call`, `callc`                                      |
| `10` | `StackOverflow`  | `call`, `callc`                                              |

Every error aborts the run with exit code 1; nothing is written (Mend §5.2).

### 15.2 Internal errors

A condition that verified code produced by a correct compiler cannot reach is
an **internal error**: a null or wrong-kind handle operand, a `callc`
signature mismatch, `snext` with the wrong pattern, `unreachable`, or a
content opcode before the input is loaded. It is reported as a bug in `mend`,
with the function and `pc`, and exits with code 1.

### 15.3 Reports

The report contains the kind; the message (the `raise` message, or a default
per kind); the span of the faulting instruction; the call stack, one span per
frame; the content offset for content errors; and the values named by the
detail descriptor. Spans come from each function's span table (§16.1), which
maps `pc` ranges to source spans. For `assert` on a comparison, lowering
attaches a detail with both operands (Mend §15).

## 16. Compiled program format

### 16.1 Layout

A `.mbc` file is a sequence of sections. Integers are little-endian; counts
are `u32`; strings are `u32` length + UTF-8.

| Section        | Contents                                                                        |
|----------------|---------------------------------------------------------------------------------|
| Header         | magic `MBC\0`, format version `u32`, compiler version, initial byte order, `init` and `main` function ids |
| Modules        | per module: canonical path, BLAKE3 hash of the source; the `-I` list; for each `use`, the candidate paths tried before the match |
| `k`            | scalar constants                                                                 |
| `kh`           | handle constants                                                                 |
| `t`            | type descriptors (§8.4)                                                          |
| `p`            | patterns (§14) with capture layouts and source text for reports                 |
| `f`, `d`, `w`  | format, detail and switch tables                                                 |
| Globals        | `gs` and `gh` counts                                                             |
| Parameters     | name, type `t`, global index, default (`k`/`kh` or none)                         |
| Functions      | name, signature `t`, frame size, capture counts, code words, span table          |
| Spans          | `(module, byte start, byte end)`                                                 |
| Checksum       | BLAKE3 of all preceding bytes                                                    |

**Parameters.** Before `init`, the runtime parses each `-D` value with the
same routine as `str_parse` (Mend §9.5) and writes it, or the default, into
the parameter's global. Errors exit with code 4.

### 16.2 Use in version 1

Version 1 has no cache: compiling a patch program takes milliseconds. The
format is used by tests and by the assembler and disassembler (§8.5). The
Modules section already records what a future cache needs to validate an
entry (§23).

## 17. Verifier

A compiled program is verified before it runs in debug builds. A program
read from a `.mbc` file is always verified. Verification is linear in code
size.

**Structural checks** (mandatory; together they make interpretation
memory-safe):

1. Table indices, function ids and intrinsic ids are in range; the checksum
   matches.
2. Each opcode is known; `mod` and `type` are valid for its family; `ldi`
   immediates fit their type.
3. Instructions end inside their function. Jump and switch targets are
   instruction starts in the same function. The last instruction cannot fall
   through.
4. Every register run an instruction touches, with widths taken from types,
   signatures and pattern layouts, lies inside the frame. Empty runs are
   ignored.
5. Descriptors match their operand roles: `sget` takes an element type,
   `callc` a function type, `fmt` a format.
6. `mkclos` capture runs match the target's header. Captures are never
   written. `callc`'s closure register is outside its argument run.
7. **Batch balance.** A forward dataflow assigns each `pc` a batch depth of
   0 or 1. `bbeg` requires 0, `bend` requires 1, `ret` requires 0, and paths
   that meet at a join must agree. `raise` and `unreachable` end a path.

**Type checks** (debug builds): a dataflow pass tracks the scalar type code
of each register and the handle kind of each handle register, and rejects any
instruction whose inputs don't match its encoding. In release builds a type
mismatch can produce wrong values or an internal error, never memory
unsafety.

## 18. Lowering from Typed Core

### 18.1 Register allocation

- Each Typed Core value gets a virtual run; runs are assigned to registers by
  liveness within a function. No spilling: a function needing more than the
  limit is a compile error (E0701, §21).
- On the last use of a handle-valued binding, lowering emits `htake` instead
  of `hmov`, so in-place mutation (`push`, `sset`, intrinsics) finds the
  object unique.

### 18.2 Schemes

| Typed Core                                  | MBC                                                                 |
|---------------------------------------------|---------------------------------------------------------------------|
| `load[T, e]` / `store[T, e]`                | `ld`/`st`/`ldv`/`stv`/`ldd`/`std` with the endian mode (§13.2)      |
| view use, struct field                      | offset code inline at each use, then the load or store              |
| `p op= v`                                   | `v`; the place's offset once; load; op; store (Mend §17)            |
| `T.get(at)`                                 | `ldo`                                                               |
| `&&`, `\|\|`, `??`                          | branches                                                            |
| decision trees (`match`, `if pat :=`)       | `switch` or compare chains; the choice is not observable           |
| string `match` arms                         | `heq` chains                                                        |
| `for i : a..b`                              | `i < b` test, body with `tick`, `i + 1`                             |
| `for i : a..=b`                             | `a <= b` test; body with `tick`; exit when `i == b` before the increment, so `b == MAX` never overflows |
| `r.rev()`, `r.step_by(k)`                   | the same with decrement or checked step; a `step_by` overflow ends the loop |
| `for x : [T]` / `str` / `enumerate`         | index loop with `sget` / `str_char_at`                              |
| `for m : P`, `matches(P, within:)`          | `scan` + `snext` loop                                               |
| `for … else`, labels, `break v`, `continue` | jumps; `break v` writes the loop's result run                      |
| exits from `batch` (`break`, `continue`, `return`) | `bend` on each exit edge (Mend §18.3 item 7)                 |
| multi-match edit with target `P`            | `scan`; loop `snext` → edit; a flag records any match; `raise NotFound` with a detail naming `p` if none |
| `first P` / `last P` target                 | `find` / `rfind`; `raise NotFound` if `none`; one edit              |
| `insert v before/after P`                   | `edins` at `m.start` / `m.end`                                      |
| edit with a function value                  | `call`/`callc` per match before its edit                            |
| edit value `v`                              | `enc.cur` (or a static mode); folded into a `kh` byte constant when `v` is constant and the byte order is static |
| `assert c, msg`                             | `brt c`; `msg` code; `raise Assertion` with a comparison detail if `c` is a comparison |
| `unwrap`, `expect`                          | `brt tag`; `raise NotFound`                                         |
| format string                               | `fmt` with an `f` descriptor                                        |
| lambda                                      | function + `mkclos` (§11)                                           |
| `map`, `filter`, `fold`, `any`, `all`, `sum`| index loops with `call`/`callc`; `sum` uses checked `add`           |
| `[x; n]`, `++`                              | `sfill`, `scat`                                                     |
| `Name.contains(v)`, `Name.name_of(v)`       | `switch` to constants                                               |
| top-level bindings, parameters              | `gset`/`ghset` in `init`; `gget`/`ghget` elsewhere                  |
| constants                                   | `ldi`, `ldk`, `hldk`                                                |

### 18.3 Constant evaluation

Constants are evaluated by this interpreter. Each constant's initializer, and
everything it calls, is lowered to a function and run
with a step limit of 1 000 000. The result is interned in `k` or `kh`. A
runtime error or the step limit is a compile error (E0702) that shows the
constant's span and the error kind. One evaluator means constants and runtime
code cannot disagree (§24). Within Mend §18.1's pipeline, const-eval runs on
lowered code instead of on the Typed Core; the result is the same.

### 18.4 Optimisations

None are required. Any of these is permitted if it preserves §6.3:

- constant folding and propagation; dead-code removal;
- check removal (replacing `add` with `addw`, removing a bounds check) only
  with a proof that the check cannot fail (Mend §18.3 item 2);
- inlining, which keeps a `tick` at the inlined call site;
- copy propagation and register coalescing.

An optimisation may not reorder two instructions that can raise, print or
touch the content.

## 19. Contract with the language RFC

### 19.1 What this RFC relies on (Mend §18.3)

| Guarantee                              | Use here                                                      |
|----------------------------------------|---------------------------------------------------------------|
| 1. one concrete type per value         | type codes and layouts (§7.3, §8.3)                           |
| 2. explicit runtime checks             | fused into checked opcodes; unchecked forms only with proof (§18.4) |
| 3. intrinsic ids and type arguments; static or dynamic byte order | `intr` (§12); endian modes (§8.3, §13.2) |
| 4. one implicit content buffer         | no buffer operands (§9.9–§9.11)                               |
| 5. matcher descriptions                | fixed and program forms (§14)                                 |
| 6. spans on every node                 | span tables (§15.3, §16.1)                                    |
| 7. `batch_begin`/`batch_end` on every non-error exit | `bbeg`/`bend`; checked by the verifier (§17)    |
| 8. content intrinsics only in `main` and content functions | none needed; the `loaded` check (§13.1) reports a violation as an internal error (§15.2) |

### 19.2 What this RFC provides (Mend §18.3, "The runtime provides")

| Item                                       | Section          |
|--------------------------------------------|------------------|
| content buffer, snapshots, edit queues, markers | §13         |
| pattern engine                             | §14              |
| `-D` parsing into parameters               | §16.1            |
| reading input, committing output           | §6.1 (unchanged from Mend §5.2) |
| formatting                                 | `fmt` (§9.12), `f` tables |
| error reports                              | §15              |
| step counter                               | §9.12            |
| instruction set, encoding, calling convention, closure layout, code generation | §8–§12, §18 |

### 19.3 Requests to the language RFC

Applied to the language RFC on 2026-10-16 (§25): the error kinds of the
cases in §25 Q1 and the kind `StackOverflow` (Mend §15); `last`/`rfind`
(Mend §13.4, §14.1); `modified()` (Mend §16); `--emit asm` (Mend §5.1).

Still requested; this RFC adopts each until the language RFC decides:

- the diagnostic code range E07xx for backend and const-eval errors (§21);
- §18.1: const-eval runs on lowered code (§18.3).

## 20. Limits

| Item                                       | Limit          | When exceeded                |
|--------------------------------------------|----------------|------------------------------|
| scalar or handle registers per frame       | 65 535 each    | E0701                        |
| functions, intrinsics                      | 65 535         | E0701                        |
| entries per table (`k kh t p f d w`)       | 65 535         | E0701                        |
| globals per bank                           | 65 535         | E0701                        |
| jump distance                              | ±2³¹ words     | E0701                        |
| call depth                                 | 100 000 frames | `StackOverflow`              |
| const-eval steps per constant              | 1 000 000      | E0702                        |

## 21. Diagnostics

**Backend compile errors** (exit code 2):

```
error[E0701]: function `main` needs 70211 scalar registers; the limit is 65535
 --> big.mend:14:1
   |
14 | main :: () {
   | ^^^^ this function
   = help: move part of the body into helper functions
```

```
error[E0702]: constant evaluation failed: Overflow
 --> fix.mend:3:15
  |
3 | TOTAL : u8 : 200 + 100;
  |              ^^^^^^^^^ `u8` addition overflows
```

**Runtime errors** (exit code 1):

```
error: OutOfBounds: read of 4 bytes at 0x1000; content length is 0xff8
  --> blocks.mend:3:30
   |
 3 | block :: (i: usize) -> u32 { u32[4 + i * 4] }
   |                              ^^^^^^^^^^^^^^ `u32` at 0x1000
   = note: call stack:
           0: block  blocks.mend:3:30
           1: main   blocks.mend:10:14
   = note: no output written
```

```
error: Assertion: unsupported image version 2
 --> fw.mend:15:12
   |
15 |     assert Image.version >= 3, f"unsupported image version {Image.version}";
   |            ^^^^^^^^^^^^^^^^^^ left: 2u16, right: 3u16
```

```
error: BatchConflict: edits overlap in the batch snapshot
  --> fix.mend:9:9
   |
 8 |         Riff.size += 4;
   |         -------------- replaces 0x4..0x8
 9 |         delete 0x6..+4;
   |         ^^^^^^^^^^^^^^ deletes 0x6..0xa
   = note: detected when the batch ended (fix.mend:7:5)
```

**Internal errors and warnings:**

```
error: internal error in mend: null handle h2 used by `scat`
  = note: function `main`, pc 37 (fix.mend:12:5)
  = note: this is a bug in mend; please report it with the program
```

```
error: StackOverflow: call depth exceeds 100000 frames
  --> walk.mend:7:9
   |
 7 |         walk(at + 8);
   |         ^^^^^^^^^^^^ this call
   = note: the call stack repeats `walk` (walk.mend:7:9)
```

**Assembly listing** (`--emit asm`, Mend §5.1) uses §8.5 with a source comment
per statement. The text form is unstable and may change with any `mend`
version:

```
fn @main () frame s:6 h:0
    ; blocks.mend:6  for i : 0..count as usize {
    ldi.u64    s0, 0
    ...
```

## 22. Assembly grammar

The notation follows Mend §20.

```ebnf
listing         = { table-entry | function } ;
table-entry     = table-ref , "=" , table-value , newline ;
table-ref       = ( "k" | "kh" | "t" | "p" | "f" | "d" | "w" ) , digits ;
table-value     = ? the entry, in Mend syntax where one exists: a literal, a type, a pattern literal ? ;

function        = "fn" , fn-ref , signature , "frame" , "s:" , digits , "h:" , digits ,
                  [ "caps" , "s:" , digits , "h:" , digits ] , newline ,
                  { line } , "end" , newline ;
signature       = ? a Mend function type, optionally naming result runs ? ;
line            = [ label , ":" ] , [ instruction ] , [ comment ] , newline ;
instruction     = mnemonic , [ operand , { "," , operand } ] ;
mnemonic        = identifier , { "." , identifier } ;
operand         = register | global | table-ref | fn-ref | intrinsic-ref
                | label | immediate | kind | "-" ;
register        = ( "s" | "h" ) , digits ;
global          = ( "gs" | "gh" ) , digits ;
fn-ref          = "@" , identifier , { "." , ( identifier | digits ) } ;
intrinsic-ref   = "$" , identifier , [ "[" , type , "]" ] ;
label           = "L" , digits ;
immediate       = [ "-" ] , integer-literal ;
kind            = identifier ;                    (* an error kind, §15.1 *)
comment         = ";" , { any-char-except-newline } ;
```

Disambiguation:

1. **Operand prefixes.** The longest prefix wins: `kh3` is a handle
   constant, `k3` a scalar constant, `gs3` a global, `s3` a register.
2. **Mnemonic parts.** After the family name, a part that is an endian mode
   (`le`, `be`, `cur`, `reg`) is the modifier, and a type name is the type.
   The modifier comes first.
3. **Pattern source.** `find`, `rfind`, `matat`, `count`, `findall`, `scan`
   and `snext` take `mod` from their pattern operand: `p…` → `0`, `h…` or
   `-` (only `snext`) → `1`.
4. **`ret` and `call` operands.** Trailing empty runs may be written `-` or
   left out: `ret` is `ret -, -`.

## 23. Future possibilities

- **Native code** for hot functions, through Cranelift, with this bytecode as
  the input and the interpreter as the fallback.
- **Superinstructions** (`lt`+`brf`, `ld`+`xor`+`st`) chosen from profiles.
- **Register windows**: callee frames overlapping the caller's argument run,
  removing the argument copy.
- **Piece-table content buffer**, making edits outside `batch` cheaper than
  `O(len)`.
- **Fused content intrinsics** such as `crc32` over a content range, avoiding
  the `ldv` copy.
- **Debugger and tracing** built on span tables and a `--trace` mode.
- **`--profile`**: step and instruction counts per span.
- **Compiled-program cache** in `$XDG_CACHE_HOME/mend/` (default
  `~/.cache/mend/`), keyed by the root module's canonical path and source
  hash. An entry is valid when the compiler version and `-I` list match,
  every listed module has the same hash, and no candidate path tried before a
  match now exists. Entries are written through a temporary file and a
  rename. A checksum or verifier failure discards the entry and recompiles
  with a warning; a cache I/O error is a warning, never a failure.

## 24. Alternatives considered

| Alternative                                     | Reason for rejection                                                   |
|-------------------------------------------------|------------------------------------------------------------------------|
| Stack machine (Wasm, JVM)                       | More instructions, and so more dispatches, per Mend operation.         |
| One bank of tagged values                       | Runtime type dispatch on every operation, against Mend §18.3's typed core; larger registers. |
| Native code generation                          | Compile latency exceeds typical run time; much larger implementation.   |
| Tree-walking interpreter over Typed Core        | Per-node dispatch and dynamic layouts; nothing to serialize or verify. |
| Tracing garbage collector                       | Values cannot form cycles; copy-on-write needs counts anyway.          |
| 32-bit words with 8-bit operands (Lua)          | 256 registers is too few without spilling; typed opcodes don't fit 8 bits. |
| Variable-length byte encoding                   | Slower decoding and harder jump arithmetic, for a size win a future cache doesn't need. |
| Separate check instructions (`chk_add` + `add`) | Two dispatches for every checked operation.                            |
| 16-byte registers for `u128`/`i128`             | Doubles every frame for rare types. Register pairs cost nothing for other types. |
| Boxed 128-bit integers                          | An allocation per arithmetic operation.                                |
| Intrinsics that call bytecode (`map` as an intrinsic) | Re-entrant interpreter; step counting inside native code. Loops keep one dispatch loop. |
| Copying the content on `bbeg`                   | Unnecessary: queued edits leave the buffer unchanged, so it is the snapshot. |
| A `content` flag in function headers, checked by the verifier | Redundant: the front end enforces Mend §5.3, and the `loaded` check (§13.1) catches violations. A static rule also misses `callc` targets. |
| Explicit `scan_close`                           | Every exit path would need it; releasing the last handle does the same. |
| Separate Typed Core evaluator for constants     | Two implementations of the same semantics that can drift apart.        |
| Mandatory type verification                     | Doubles verification cost; structural checks already give memory safety. |
| Copying handle arguments on `call`              | Leaves a second reference, so the callee's first mutation copies the object. |
| Generic instructions with a runtime type operand | A runtime type dispatch per instruction, which typed opcodes avoid.    |
| `InvalidArgument` kind for `align_*`, `clamp`, `Pat.bytes([])` | One more kind for cases `InvalidValue` already describes. |
| No call-depth limit                             | Exhausting memory aborts without a Mend report, and the failure point depends on the host. |
| `rfind` as the match with the greatest start    | Can overlap the `findall` sequence (`p#/AA AA/#` on `AAAAA` gives `3..5`, not `2..4`). |
| `modified()` set by any write                   | Writing identical bytes returns true, so a re-run cannot detect an already-patched file. |
| Compiled-program cache in version 1             | Compilation takes milliseconds; the cache adds invalidation rules and a cache directory. |

## 25. Unresolved questions

None. Questions resolved on 2026-10-16:

| Question                                  | Decision                                                        | See                  |
|-------------------------------------------|-----------------------------------------------------------------|----------------------|
| Q1. Error kinds Mend §15 did not name     | Nested `bbeg` and `endian` in a batch: `BatchConflict`; partial-element `T[r]`: `LengthMismatch`; `align_*`, `clamp`, `Pat.bytes([])`: `InvalidValue`; float arithmetic: none (IEEE 754) | Mend §10.2, §15; §15.1 |
| Q2. Unbounded recursion                   | `StackOverflow` at a fixed depth of 100 000 frames              | Mend §15; §10, §20   |
| Q3. `last P` and `rfind`                  | Last match of the forward, non-overlapping sequence             | Mend §13.4, §14.1; §14.3 |
| Q4. `modified()`                          | True if the content differs from the input; compared lazily     | Mend §16; §13.1      |
| Q5. `--emit asm`                          | CLI flag; the text form is unstable                             | Mend §5.1; §21       |
| Q6. Cache in version 1                    | Deferred                                                        | §16.2, §23           |

## Appendix A. Worked lowerings

Frames and tables are shown in full; source lines are comments.

### A.1 Views, assertions and a checksum (Mend §4)

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
main :: () {
    assert Image.magic == MAGIC, "not a firmware image";
    assert Image.version >= 3, f"unsupported image version {Image.version}";
    Image.boot_delay = DELAY_MS;
    Image.checksum = crc32(u8[0x10..]);
}
```

No function contains `endian`, so every access is statically `.le`. The
top-level `assert DELAY_MS <= 9_999` folds away.

```
kh0 = b"FWIM"
kh1 = "not a firmware image"
t0  = [u8; 4]
f0  = f"unsupported image version {u16}"
d0  = cmp "==" (t0 h0) (t0 h1)
d1  = cmp ">=" (u16 s1) (u16 s2)

fn @main () frame s:5 h:3
    ; assert Image.magic == MAGIC, "not a firmware image";
    ldi.u64    s0, 0
    ldi.u64    s1, 4
    ldv.u8     h0, s0             ; [u8; 4] at 0..4
    hldk       h1, kh0
    heq        s2, h0, h1, t0
    brt        s2, L1
    hldk       h2, kh1
    raise      Assertion, h2, d0
L1: ; assert Image.version >= 3, f"…";
    ldi.u64    s0, 4
    ld.le.u16  s1, s0
    ldi.u16    s2, 3
    ge.u16     s3, s1, s2
    brt        s3, L2
    ld.le.u16  s4, s0             ; the message is evaluated only on failure
    fmt        h2, s4, -, f0
    raise      Assertion, h2, d1
L2: ; Image.boot_delay = DELAY_MS;   right-hand side, then the place
    ldi.u32    s0, 250
    ldi.u64    s1, 4
    ld.le.u16  s2, s1             ; version, for boot_delay's offset
    ldi.u16    s3, 5
    ge.u16     s3, s2, s3
    brf        s3, L3
    ldi.u64    s4, 0x40
    jmp        L4
L3: ldi.u64    s4, 0x38
L4: st.le.u32  s4, s0
    ; Image.checksum = crc32(u8[0x10..]);
    ldi.u64    s0, 0x10
    len        s1                 ; open range end
    ldv.u8     h0, s0
    intr       $crc32, s0, h0     ; [u8] moved in; u32 out in s0
    ldi.u64    s1, 0x0C
    st.le.u32  s1, s0
    ret
end
```

### A.2 A multi-match edit with markers

```mend
replace p#/FF 00 FF/# with x"00 FF 00";
```

```
p0  = fixed (FF/FF 00/FF FF/FF)
kh0 = x"00 FF 00"
d0  = pattern p0

    ldi.u64    s0, 0
    len        s1                 ; window 0..len()
    scan       h0, p0, s0         ; pos = 0, lim = len
    hldk       h1, kh0
    ldi.bool   s2, 0              ; any match yet?
L0: snext      s3, h0, p0         ; s3 tag; s4..s5 match
    brf        s3, L1
    edrep      s4, h1             ; [a, b) → 3 bytes; pos (= b) becomes a + 3
    ldi.bool   s2, 1
    jmp        L0
L1: hdrop      h0                 ; unregisters the markers
    brt        s2, L2
    raise      NotFound, -, d0
L2:
```

The new bytes `00 FF 00` are never rescanned, because `pos` moves past them
(Mend §14.4, row `x >= b`).

### A.3 `batch` (Mend A.1)

```mend
batch {
    Riff.size += chunk.len() as u32;
    insert chunk at 12;
}
```

`chunk` is a top-level binding in `gh0`.

```
    bbeg
    ghget      h0, gh0
    slen       s0, h0
    cvt.u32    s0, s0, u64        ; chunk.len() as u32: right-hand side first
    ldi.u64    s1, 4              ; place Riff.size
    ld.le.u32  s2, s1             ; reads the snapshot
    add.u32    s2, s2, s0         ; Overflow is checked
    st.le.u32  s1, s2             ; queued: replace 4..8, seq 0
    ldi.u64    s3, 12
    edins      s3, h0             ; queued: insert at 12, seq 1
    bend                          ; no overlap; applies both in one pass
```

### A.4 A closure called from a sequence method (Mend A.3)

```mend
xor_with :: (key: u8) -> (u8) -> u8 { (b: u8) -> u8 { b ^ key } }
decode := xor_with(KEY);
main :: () { u8[0x10..] = u8[0x10..].map(decode); }
```

```
t0 = u8
t1 = (u8) -> u8

fn @init () frame s:1 h:1
    ldi.u8     s0, 0x5A
    call       s0, h0, @xor_with  ; u8 in s0; closure out in h0
    ghset      gh0, h0
    ret
end

fn @xor_with (u8) -> (u8) -> u8 frame s:1 h:1
    mkclos     h0, @xor_with.0, s0, -   ; capture key by value
    ret        -, h0
end

fn @xor_with.0 (u8) -> u8 frame s:2 h:0 caps s:1 h:0
    xor.u8     s0, s0, s1         ; b ^ key (s1 is the capture)
    ret        s0, -
end

fn @main () frame s:7 h:3
    ldi.u64    s0, 0x10           ; right-hand side: u8[0x10..].map(decode)
    len        s1
    ldv.u8     h0, s0
    ghget      h1, gh0
    slen       s2, h0
    snew       h2, s2, t0
    ldi.u64    s3, 0
L0: lt.u64     s4, s3, s2
    brf        s4, L1             ; desugared loop: no tick
    sget       s5, -, h0, s3, t0
    callc      s5, -, h1, t1      ; one step per call
    spush      h2, s5, -, t0
    addi.u64   s3, s3, 1
    jmp        L0
L1: ldi.u64    s5, 0x10           ; place: u8[0x10..]
    len        s6
    stv.u8     s5, h2             ; LengthMismatch if lengths differ
    ret
end
```

### A.5 A loop with `break` value, `else` and a typed match (Mend A.4)

```mend
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
```

`Opt[Match[CHUNK]]` is 7 scalar words: tag, start, end, `tag` capture
(start, end), `size` capture (start, end).

```
p0  = fixed (??×8) caps tag@0+4 size@4+4
t0  = [u8]
kh0 = "truncated chunk header"

fn @find_chunk ([u8; 4]) -> Opt[Match[p0]] frame s:19 h:2
    ldi.u64    s7, 12             ; mut at
L0: len        s8
    lt.u64     s9, s7, s8
    brf        s9, L3             ; ended without break: else
    tick
    matat      s10, p0, s7        ; s10 tag; s11..s16 match
    brt        s10, L1
    hldk       h1, kh0
    raise      NotFound, h1, -
L1: ldv.u8     h1, s13            ; u8[m.tag]
    heq        s17, h1, h0, t0
    brf        s17, L2
    ldi.bool   s0, 1              ; break some(m)
    movn       s1, s11, 6
    ret        s0, -
L2: ld.le.u32  s17, s15           ; u32[m.size.start]
    cvt.u64    s17, s17, u32
    add.u64    s17, s12, s17      ; m.end + …
    ldi.u64    s18, 2
    intr       $align_up[u64], s17, -   ; (s17, s18) → s17
    mov        s7, s17
    jmp        L0
L3: ldi.bool   s0, 0              ; else { none }
    clr        s1, 6
    ret        s0, -
end
```
