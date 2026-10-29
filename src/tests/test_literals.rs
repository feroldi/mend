use crate::tests::{check, compile};

#[test]
fn test_hexadecimal_integer() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 0x7F;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.i32 s0, 127
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_binary_integer() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 0b1010;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.i32 s0, 10
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_octal_integer() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 0o17;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.i32 s0, 15
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_integer_with_digit_separators() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 1_000_000;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.i32 s0, 1000000
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_integer_with_suffix() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 42u8;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.u8 s0, 42
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_integer_with_each_suffix() {
    let program = compile(
        r#"
        |main :: () {
        |    a := 1u16;
        |    b := 2u32;
        |    c := 3u64;
        |    d := 4i8;
        |    e := 5i16;
        |    f := 6i32;
        |    g := 7i64;
        |    h := 8usize;
        |    i := 9isize;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:9 h:0
        |    ldi.u16 s0, 1
        |    ldi.u32 s1, 2
        |    ldi.u64 s2, 3
        |    ldi.i8 s3, 4
        |    ldi.i16 s4, 5
        |    ldi.i32 s5, 6
        |    ldi.i64 s6, 7
        |    ldi.u64 s7, 8
        |    ldi.i64 s8, 9
        |    ret
        |end
        |"#,
    );
}
