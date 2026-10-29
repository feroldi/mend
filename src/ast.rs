use std::fmt;

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
    Integer {
        value: u64,
        has_overflowed: bool,
        span: Span,
    },
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

impl Type {
    /// The inclusive bounds of an integer type.
    pub(crate) fn integer_bounds(self) -> (i64, u64) {
        debug_assert!(self.is_integer_type());

        let bounds = match self {
            Type::U8 => (0, u8::MAX as u64),
            Type::U16 => (0, u16::MAX as u64),
            Type::U32 => (0, u32::MAX as u64),
            Type::U64 => (0, u64::MAX),
            Type::I8 => (i8::MIN as i64, i8::MAX as u64),
            Type::I16 => (i16::MIN as i64, i16::MAX as u64),
            Type::I32 => (i32::MIN as i64, i32::MAX as u64),
            Type::I64 => (i64::MIN, i64::MAX as u64),
            Type::IntLiteral | Type::Unit | Type::Never | Type::Function => unreachable!(),
        };

        bounds
    }

    pub(crate) fn is_integer_type(self) -> bool {
        match self {
            Type::U8 => true,
            Type::U16 => true,
            Type::U32 => true,
            Type::U64 => true,
            Type::I8 => true,
            Type::I16 => true,
            Type::I32 => true,
            Type::I64 => true,
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::U8 => write!(f, "u8"),
            Type::U16 => write!(f, "u16"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::I8 => write!(f, "i8"),
            Type::I16 => write!(f, "i16"),
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::IntLiteral => write!(f, "{{integer}}"),
            Type::Unit => write!(f, "()"),
            Type::Never => write!(f, "!"),
            Type::Function => write!(f, "() -> ()"),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Function<'ctx> {
    pub(crate) body: CompoundExpr<'ctx>,
}

#[derive(Clone, Copy)]
pub(crate) struct CompoundExpr<'ctx> {
    pub(crate) exprs: &'ctx [Expr<'ctx>],
}
