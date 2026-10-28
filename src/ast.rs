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
    Const(Const),
    BindRef(BindRef),
    BindDef(BindDef<'ctx>),
    Assign(AssignExpr<'ctx>),
    Function(Function<'ctx>),
    Compound(CompoundExpr<'ctx>),
    Return,
    Semi(&'ctx Expr<'ctx>),
}

#[derive(Clone, Copy)]
pub(crate) enum Const {
    Integer { value: u64 },
}

#[derive(Clone, Copy)]
pub(crate) struct BindRef {
    pub(crate) identifier: Symbol,
}

#[derive(Clone, Copy)]
pub(crate) struct BindDef<'ctx> {
    pub(crate) identifier: Symbol,
    pub(crate) is_mutable: bool,
    pub(crate) ty: Option<Type>,
    pub(crate) value: &'ctx Expr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) struct AssignExpr<'ctx> {
    pub(crate) identifier: Symbol,
    pub(crate) value: &'ctx Expr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) enum Type {
    U8,
}

#[derive(Clone, Copy)]
pub(crate) struct Function<'ctx> {
    pub(crate) body: CompoundExpr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) struct CompoundExpr<'ctx> {
    pub(crate) exprs: &'ctx [Expr<'ctx>],
}
