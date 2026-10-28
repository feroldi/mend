use std::collections::HashMap;

use crate::ast::{AssignExpr, BindDef, BindRef, Expr, Program};
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diag, Diagnostic, DiagnosticBag};
use crate::interner::Symbol;
use crate::source_map::Span;

pub(crate) struct Sema<'ctx> {
    ctx: &'ctx CompilerContext,
    scope_stack: Vec<HashMap<Symbol, Binding>>,
}

#[derive(Clone, Copy)]
struct Binding {
    is_mutable: bool,
}

impl<'ctx> Sema<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> Sema<'ctx> {
        Sema {
            ctx,
            scope_stack: vec![],
        }
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scope_stack.push(HashMap::new());
    }

    pub(crate) fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn get_this_scope_mut(&mut self) -> &mut HashMap<Symbol, Binding> {
        self.scope_stack.last_mut().unwrap()
    }

    fn scope_contains(&self, identifier: Symbol) -> bool {
        self.find_in_scope(identifier).is_some()
    }

    fn find_in_scope(&self, identifier: Symbol) -> Option<Binding> {
        self.scope_stack
            .iter()
            .rev()
            .find_map(|scope| scope.get(&identifier).copied())
    }

    pub(crate) fn act_on_bind_def(&mut self, bind_def: BindDef<'ctx>) -> Expr<'ctx> {
        self.get_this_scope_mut().insert(
            bind_def.identifier,
            Binding {
                is_mutable: bind_def.is_mutable,
            },
        );

        Expr::BindDef(bind_def)
    }

    pub(crate) fn act_on_bind_ref(
        &mut self,
        bind_ref: BindRef,
        span: Span,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        if !self.scope_contains(bind_ref.identifier) {
            return Err(binding_not_found(bind_ref.identifier, span));
        }

        Ok(Expr::BindRef(bind_ref))
    }

    pub(crate) fn act_on_assign(
        &mut self,
        assign_expr: AssignExpr<'ctx>,
        span: Span,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        let Some(binding) = self.find_in_scope(assign_expr.identifier) else {
            return Err(binding_not_found(assign_expr.identifier, span));
        };

        if !binding.is_mutable {
            let name = self.ctx.resolve_symbol(assign_expr.identifier);

            return Err(DiagnosticBag::with_diag(
                Diagnostic::builder(Diag::AssignToImmutableBinding {
                    ident: assign_expr.identifier,
                })
                .label(span, "cannot assign")
                .help(format!("declare the binding as `mut {name}`"))
                .build(),
            ));
        }

        Ok(Expr::Assign(assign_expr))
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

fn binding_not_found(ident: Symbol, span: Span) -> DiagnosticBag {
    DiagnosticBag::with_diag(
        Diagnostic::builder(Diag::BindingNotFound { ident })
            .label(span, "not found in this scope")
            .build(),
    )
}
