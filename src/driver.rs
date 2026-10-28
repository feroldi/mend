use crate::codegen::CodeGen;
use crate::compiler_context::CompilerContext;
use crate::parser::Parser;
use crate::scanner::Scanner;
use crate::source_map::SourceMap;

pub(crate) fn compile(source_code: &str) -> Result<String, String> {
    let source_map = SourceMap::new(SourceMap::DEFAULT_SOURCE_NAME.into(), source_code.into());
    let context = CompilerContext::new(source_map);

    let scanner = Scanner::new(&context);
    let mut parser = Parser::new(scanner, &context);
    let program = parser
        .parse_program()
        .map_err(|diagnostics| diagnostics.render(context.get_source_map()))?;

    let mut codegen = CodeGen::new(&context);
    let mbc_program = codegen.gen_program(program);

    Ok(format!("{}", mbc_program))
}
