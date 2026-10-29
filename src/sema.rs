use std::collections::HashMap;

use crate::ast::{
    AssignExpr, BindDef, BindRef, CompoundExpr, Const, Expr, ExprKind, Function, Program, Type,
};
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
    ty: Type,
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

    fn find_in_scope(&self, identifier: Symbol) -> Option<Binding> {
        self.scope_stack
            .iter()
            .rev()
            .find_map(|scope| scope.get(&identifier).copied())
    }

    pub(crate) fn act_on_integer_constant(
        &mut self,
        value: u64,
        suffix: Option<Type>,
        span: Span,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        if let Some(ty) = suffix {
            check_integer_fits_type(value, ty, span)?;
        }

        Ok(Expr {
            kind: ExprKind::Const(Const::Integer { value }),
            ty: suffix.unwrap_or(Type::IntLiteral),
        })
    }

    pub(crate) fn act_on_bind_def(&mut self, bind_def: BindDef<'ctx>) -> Expr<'ctx> {
        let value = self.resolve_literal_type(*bind_def.value, bind_def.ty);

        self.get_this_scope_mut().insert(
            bind_def.identifier,
            Binding {
                is_mutable: bind_def.is_mutable,
                ty: value.ty,
            },
        );

        Expr {
            kind: ExprKind::BindDef(BindDef {
                value: self.ctx.alloc_expr(value),
                ..bind_def
            }),
            ty: Type::Unit,
        }
    }

    pub(crate) fn act_on_bind_ref(
        &mut self,
        bind_ref: BindRef,
        span: Span,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        let Some(binding) = self.find_in_scope(bind_ref.identifier) else {
            return Err(binding_not_found(bind_ref.identifier, span));
        };

        Ok(Expr {
            kind: ExprKind::BindRef(bind_ref),
            ty: binding.ty,
        })
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

        let value = self.resolve_literal_type(*assign_expr.value, Some(binding.ty));

        Ok(Expr {
            kind: ExprKind::Assign(AssignExpr {
                value: self.ctx.alloc_expr(value),
                ..assign_expr
            }),
            ty: Type::Unit,
        })
    }

    pub(crate) fn act_on_compound_expr(&mut self, compound_expr: CompoundExpr<'ctx>) -> Expr<'ctx> {
        Expr {
            kind: ExprKind::Compound(compound_expr),
            ty: Type::Unit,
        }
    }

    pub(crate) fn act_on_semi_expr(&mut self, expr: Expr<'ctx>) -> Expr<'ctx> {
        Expr {
            kind: ExprKind::Semi(self.ctx.alloc_expr(expr)),
            ty: Type::Unit,
        }
    }

    pub(crate) fn act_on_return_expr(&mut self) -> Expr<'ctx> {
        Expr {
            kind: ExprKind::Return,
            ty: Type::Never,
        }
    }

    pub(crate) fn act_on_function(&mut self, function: Function<'ctx>) -> Expr<'ctx> {
        Expr {
            kind: ExprKind::Function(function),
            ty: Type::Function,
        }
    }

    fn resolve_literal_type(&self, expr: Expr<'ctx>, expected_ty: Option<Type>) -> Expr<'ctx> {
        if expr.ty != Type::IntLiteral {
            return expr;
        }

        Expr {
            ty: expected_ty.unwrap_or(Type::I32),
            ..expr
        }
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

fn check_integer_fits_type(value: u64, ty: Type, span: Span) -> Result<(), DiagnosticBag> {
    let (min, max) = ty.integer_bounds();

    if value <= max {
        return Ok(());
    }

    Err(DiagnosticBag::with_diag(
        Diagnostic::builder(Diag::IntegerLiteralOutOfRange { ty })
            .label(span, format!("does not fit in `{ty}`"))
            .help(format!("the range of `{ty}` is `{min}..={max}`"))
            .build(),
    ))
}

fn binding_not_found(ident: Symbol, span: Span) -> DiagnosticBag {
    DiagnosticBag::with_diag(
        Diagnostic::builder(Diag::BindingNotFound { ident })
            .label(span, "not found in this scope")
            .build(),
    )
}
