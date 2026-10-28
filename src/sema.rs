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
        let mut main_decls = program
            .decls
            .iter()
            .filter(|decl| self.ctx.resolve_symbol(decl.identifier) == "main");

        if main_decls.next().is_none() {
            return Err(DiagnosticBag::with_diag(
                Diagnostic::builder(Diag::MainNotFound)
                    .help("add `main :: () {}` to the root module")
                    .build(),
            ));
        }

        if let Some(redeclared_main) = main_decls.next() {
            return Err(DiagnosticBag::with_diag(
                Diagnostic::builder(Diag::MainDeclaredTwice)
                    .label(redeclared_main.identifier_span, "`main` redeclared here")
                    .build(),
            ));
        }

        Ok(program)
    }
}
