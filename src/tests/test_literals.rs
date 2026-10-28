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
