use crate::ast::Program;
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diag, Diagnostic, DiagnosticBag};

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
    ) -> Result<Program<'ctx>, DiagnosticBag> {
        let has_main = program
            .decls
            .iter()
            .any(|decl| self.ctx.resolve_symbol(decl.identifier) == "main");

        if !has_main {
            return Err(DiagnosticBag::with_diag(
                Diagnostic::builder(Diag::MainNotFound)
                    .help("add `main :: () {}` to the root module")
                    .build(),
            ));
        }

        Ok(program)
    }
}
