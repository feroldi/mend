use crate::tests::{check, check_fail, compile};

#[test]
fn test_main_empty_function() {
    let program = compile(
        r#"
        |main :: () {}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:0 h:0
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_main_returns_early() {
    let program = compile(
        r#"
        |main :: () {
        |    return;
        |}
        |"#,
    );

    check(
        program,
        r#"
        |fn @main () frame s:0 h:0
        |    ret
        |end
        |"#,
    );
}

#[test]
fn test_main_not_found() {
    let program = compile(
        r#"
        |foo :: () {}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0001]: `main` function not found
        |  = help: add `main :: () {}` to the root module
        |"#,
    );
}
