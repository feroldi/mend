use crate::interner::Symbol;
use crate::source_map::Span;

#[derive(Clone, Copy)]
pub(crate) struct Program<'ctx> {
    pub(crate) decls: &'ctx [Decl<'ctx>],
}

#[derive(Clone, Copy)]
pub(crate) struct Decl<'ctx> {
    pub(crate) identifier: Symbol,
    pub(crate) identifier_span: Span,
    pub(crate) value: &'ctx Expr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) enum Expr<'ctx> {
    Function(Function<'ctx>),
    Return,
    Semi(&'ctx Expr<'ctx>),
}

#[derive(Clone, Copy)]
pub(crate) struct Function<'ctx> {
    pub(crate) body: CompoundExpr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) struct CompoundExpr<'ctx> {
    pub(crate) exprs: &'ctx [Expr<'ctx>],
}
