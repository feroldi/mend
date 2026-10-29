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
pub(crate) struct Expr<'ctx> {
    pub(crate) kind: ExprKind<'ctx>,
    pub(crate) ty: Type,
}

#[derive(Clone, Copy)]
pub(crate) enum ExprKind<'ctx> {
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Type {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    /// The type of an unsuffixed integer literal before its use decides it.
    IntLiteral,
    Unit,
    Never,
    /// The type of a function that takes no arguments and returns `()`.
    Function,
}

#[derive(Clone, Copy)]
pub(crate) struct Function<'ctx> {
    pub(crate) body: CompoundExpr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) struct CompoundExpr<'ctx> {
    pub(crate) exprs: &'ctx [Expr<'ctx>],
}
