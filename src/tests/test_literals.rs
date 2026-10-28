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
