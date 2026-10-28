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

#[test]
fn test_decl_without_identifier() {
    let program = compile(
        r#"
        |() {}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0002]: expected identifier, found `(`
        | --> <source>:1:1
        |  |
        |1 | () {}
        |  | ^ expected identifier
        |"#,
    );
}

#[test]
fn test_decl_without_colon_colon() {
    let program = compile(
        r#"
        |main :: () {}
        |foo () {}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0002]: expected `::`, found `(`
        | --> <source>:2:5
        |  |
        |2 | foo () {}
        |  |     ^ expected `::`
        |"#,
    );
}

#[test]
fn test_decl_without_colon_colon_at_end_of_file() {
    let program = compile(
        r#"
        |main
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0002]: expected `::`, found end of file
        | --> <source>:1:5
        |  |
        |1 | main
        |  |     ^ expected `::`
        |"#,
    );
}

#[test]
fn test_decl_without_value() {
    let program = compile(
        r#"
        |main :: ;
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0003]: expected expression, found `;`
        | --> <source>:1:9
        |  |
        |1 | main :: ;
        |  |         ^ expected expression
        |"#,
    );
}

#[test]
fn test_main_declared_twice() {
    let program = compile(
        r#"
        |main :: () {}
        |main :: () {}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0005]: `main` is declared more than once
        | --> <source>:2:1
        |  |
        |2 | main :: () {}
        |  | ^^^^ `main` redeclared here
        |"#,
    );
}

#[test]
fn test_function_without_closing_paren() {
    let program = compile(
        r#"
        |main :: ( {}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0002]: expected `)`, found `{`
        | --> <source>:1:11
        |  |
        |1 | main :: ( {}
        |  |           ^ expected `)`
        |"#,
    );
}

#[test]
fn test_function_without_body() {
    let program = compile(
        r#"
        |main :: () return;
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0002]: expected `{`, found `return`
        | --> <source>:1:12
        |  |
        |1 | main :: () return;
        |  |            ^^^^^^ expected `{`
        |"#,
    );
}

#[test]
fn test_multiple_functions() {
    let program = compile(
        r#"
        |foo :: () {}
        |main :: () {}
        |bar :: () {}
        |"#,
    );

    check(
        program,
        r#"
        |fn @foo () frame s:0 h:0
        |    ret
        |end
        |fn @main () frame s:0 h:0
        |    ret
        |end
        |fn @bar () frame s:0 h:0
        |    ret
        |end
        |"#,
    );
}
