use crate::ast::Program;
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diagnostic, DiagnosticCode};

pub(crate) struct Sema<'ctx> {
    ctx: &'ctx CompilerContext,
}

impl<'ctx> Sema<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> Sema<'ctx> {
        Sema { ctx }
    }

    pub(crate) fn act_on_program(
        &mut self,
        program: Program<'ctx>,
    ) -> Result<Program<'ctx>, Diagnostic> {
        let has_main = program
            .decls
            .iter()
            .any(|decl| self.ctx.resolve_symbol(decl.identifier) == "main");

        if !has_main {
            return Err(Diagnostic {
                code: DiagnosticCode::E0001,
                message: "`main` function not found".into(),
                helps: vec!["add `main :: () {}` to the root module".into()],
            });
        }

        Ok(program)
    }
}
