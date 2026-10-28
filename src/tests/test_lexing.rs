use crate::tests::{check_fail, compile};

#[test]
fn test_unknown_character() {
    let program = compile(
        r#"
        |main :: () {
        |    $
        |}
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0005]: unknown character `$`
        | --> <source>:2:5
        |  |
        |2 |     $
        |  |     ^ unknown character
        |"#,
    );
}

#[test]
fn test_unknown_non_ascii_character() {
    let program = compile(
        r#"
        |main :: () { é }
        |"#,
    );

    check_fail(
        program,
        r#"
        |error[E0005]: unknown character `é`
        | --> <source>:1:14
        |  |
        |1 | main :: () { é }
        |  |              ^ unknown character
        |"#,
    );
}
