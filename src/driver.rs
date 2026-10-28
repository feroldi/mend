use crate::codegen::CodeGen;
use crate::compiler_context::CompilerContext;
use crate::parser::Parser;
use crate::scanner::Scanner;

pub(crate) fn compile(source_code: &str) -> String {
    let context = CompilerContext::new(source_code.into());

    let tokens = {
        let mut scanner = Scanner::new(&context);
        scanner.scan_all_tokens()
    };

    let mut parser = Parser::new(tokens, &context);
    let program = parser.parse_program().unwrap();

    let mut codegen = CodeGen::new(&context);
    let mbc_program = codegen.gen_program(program);

    format!("{}", mbc_program)
}
