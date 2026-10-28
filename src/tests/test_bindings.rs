use crate::tests::{check, compile};

#[test]
fn test_immutable_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 10;
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
fn test_binding_with_type_annotation() {
    let program = compile(
        r#"
        |main :: () {
        |    x: u8 = 10;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:1 h:0
        |    ldi.u8 s0, 10
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_mutable_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    mut x := 10;
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
