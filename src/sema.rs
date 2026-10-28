use std::collections::HashSet;

use crate::ast::{BindDef, BindRef, Expr, Program};
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diag, Diagnostic, DiagnosticBag};
use crate::interner::Symbol;
use crate::source_map::Span;

pub(crate) struct Sema<'ctx> {
    ctx: &'ctx CompilerContext,
    scope_stack: Vec<HashSet<Symbol>>,
}

impl<'ctx> Sema<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> Sema<'ctx> {
        Sema {
            ctx,
            scope_stack: vec![],
        }
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scope_stack.push(HashSet::new());
    }

    pub(crate) fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn get_this_scope_mut(&mut self) -> &mut HashSet<Symbol> {
        self.scope_stack.last_mut().unwrap()
    }

    fn scope_contains(&self, identifier: Symbol) -> bool {
        self.scope_stack
            .iter()
            .any(|scope| scope.contains(&identifier))
    }

    pub(crate) fn act_on_bind_def(&mut self, bind_def: BindDef<'ctx>) -> Expr<'ctx> {
        self.get_this_scope_mut().insert(bind_def.identifier);

        Expr::BindDef(bind_def)
    }

    pub(crate) fn act_on_bind_ref(
        &mut self,
        bind_ref: BindRef,
        span: Span,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        if !self.scope_contains(bind_ref.identifier) {
            return Err(DiagnosticBag::with_diag(
                Diagnostic::builder(Diag::BindingNotFound {
                    ident: bind_ref.identifier,
                })
                .label(span, "not found in this scope")
                .build(),
            ));
        }

        Ok(Expr::BindRef(bind_ref))
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
