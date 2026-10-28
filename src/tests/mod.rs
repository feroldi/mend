use crate::driver;

mod test_lexing;
mod test_main;

fn compile(source_code: &str) -> Result<String, String> {
    driver::compile(&strip_margin(source_code))
}

fn check(program: Result<String, String>, expected_program: &str) {
    use pretty_assertions::assert_eq;

    let program = match program {
        Ok(program) => program,
        Err(diagnostics) => panic!("compilation failed:\n{}", diagnostics),
    };

    assert_eq!(
        program.trim(),
        remove_assembly_comments(strip_margin(expected_program)).trim()
    );
}

fn check_fail(program: Result<String, String>, expected_diagnostics: &str) {
    use pretty_assertions::assert_eq;

    let diagnostics = match program {
        Ok(program) => panic!("compilation succeeded:\n{}", program),
        Err(diagnostics) => diagnostics,
    };

    assert_eq!(
        diagnostics.trim(),
        strip_margin_preserving_blank_lines(expected_diagnostics).trim()
    );
}

fn strip_margin(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let mut stripped_line = line.chars().skip_while(|&c| c != '|');
            stripped_line.next();

            stripped_line.collect::<String>()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_margin_preserving_blank_lines(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let mut stripped_line = line.chars().skip_while(|&c| c != '|');
            stripped_line.next();

            stripped_line.collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_owned()
}

fn remove_assembly_comments(text: String) -> String {
    text.split('\n')
        .map(|line| {
            line.chars()
                .take_while(|&c| c != ';')
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
