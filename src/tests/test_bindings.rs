use crate::tests::{check, check_fail, compile};

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

#[test]
fn test_read_binding_by_value() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 10;
        |    y := x;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:2 h:0
        |    ldi.i32 s0, 10
        |    mov s1, s0
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_read_unknown_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    y := z;
        |}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0006]: cannot find binding `z` in this scope
        | --> <source>:2:10
        |  |
        |2 |     y := z;
        |  |          ^ not found in this scope
        |"#,
    );
}

#[test]
fn test_shadowed_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 10;
        |    x := x;
        |    y := x;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:3 h:0
        |    ldi.i32 s0, 10
        |    mov s1, s0
        |    mov s2, s1
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_shadowed_binding_inside_block() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 1;
        |    {
        |        x := 2;
        |    }
        |    y := x;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:3 h:0
        |    ldi.i32 s0, 1
        |    ldi.i32 s1, 2
        |    mov s2, s0
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_read_binding_out_of_its_block() {
    let program = compile(
        r#"
        |main :: () {
        |    {
        |        z := 1;
        |    }
        |    y := z;
        |}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0006]: cannot find binding `z` in this scope
        | --> <source>:5:10
        |  |
        |5 |     y := z;
        |  |          ^ not found in this scope
        |"#,
    );
}

#[test]
fn test_assign_to_mutable_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    mut x := 10;
        |    x = 20;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:2 h:0
        |    ldi.i32 s0, 10
        |    ldi.i32 s1, 20
        |    mov s0, s1
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_assign_to_immutable_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 10;
        |    x = 20;
        |}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0007]: cannot assign to immutable binding `x`
        | --> <source>:3:5
        |  |
        |3 |     x = 20;
        |  |     ^ cannot assign
        |  = help: declare the binding as `mut x`
        |"#,
    );
}

#[test]
fn test_assign_to_unknown_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    z = 20;
        |}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0006]: cannot find binding `z` in this scope
        | --> <source>:2:5
        |  |
        |2 |     z = 20;
        |  |     ^ not found in this scope
        |"#,
    );
}

#[test]
fn test_assign_to_copied_binding() {
    let program = compile(
        r#"
        |main :: () {
        |    x := 10;
        |    mut y := x;
        |    y = 20;
        |    z := x;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:4 h:0
        |    ldi.i32 s0, 10
        |    mov s1, s0
        |    ldi.i32 s2, 20
        |    mov s1, s2
        |    mov s3, s0
        |    ret
        |end
        |"#,
    );
}
