use crate::tests::{check, compile};

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
