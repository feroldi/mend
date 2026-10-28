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
