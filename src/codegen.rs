use std::collections::HashMap;
use std::fmt;

use crate::ast::{
    AssignExpr, BindDef, BindRef, CompoundExpr, Const, Decl, Expr, ExprKind, Function, Program,
    Type,
};
use crate::compiler_context::CompilerContext;
use crate::interner::Symbol;

pub(crate) struct CodeGen<'ctx> {
    ctx: &'ctx CompilerContext,
    emitted_insts: Vec<Inst>,
    scalar_frame_size: usize,
    scope_stack: Vec<HashMap<Symbol, Binding>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> CodeGen<'ctx> {
        CodeGen {
            ctx,
            emitted_insts: vec![],
            scalar_frame_size: 0,
            scope_stack: vec![],
        }
    }

    pub(crate) fn gen_program(&mut self, program: Program) -> MbcProgram<'ctx> {
        let functions = program
            .decls
            .iter()
            .map(|decl| self.gen_decl(decl))
            .collect();

        MbcProgram {
            ctx: self.ctx,
            functions,
        }
    }

    fn gen_decl(&mut self, decl: &Decl) -> MbcFunction {
        match decl.value.kind {
            ExprKind::Function(Function { body }) => self.gen_function(decl.identifier, body),
            _ => todo!("other top-level exprs"),
        }
    }

    fn gen_function(&mut self, name: Symbol, body: CompoundExpr) -> MbcFunction {
        self.scalar_frame_size = 0;

        self.gen_compound_expr(body);

        if !matches!(self.emitted_insts.last(), Some(Inst::Ret)) {
            self.emit(Inst::Ret);
        }

        MbcFunction {
            name,
            scalar_frame_size: self.scalar_frame_size,
            handle_frame_size: 0,
            insts: std::mem::take(&mut self.emitted_insts),
        }
    }

    fn gen_expr(&mut self, expr: &Expr) {
        match expr.kind {
            ExprKind::Semi(expr) => self.gen_expr(expr),
            ExprKind::Return => self.gen_return_expr(),
            ExprKind::BindDef(bind_def) => self.gen_bind_def_expr(bind_def),
            ExprKind::Assign(assign_expr) => self.gen_assign_expr(assign_expr),
            ExprKind::Compound(compound_expr) => self.gen_compound_expr(compound_expr),
            ExprKind::Const(_) | ExprKind::BindRef(_) | ExprKind::Function(_) => unimplemented!(),
        }
    }

    fn gen_expr_to_reg(&mut self, expr: &Expr, dest: ScalarReg) {
        let scalar_ty = scalar_type_of(expr.ty);

        match expr.kind {
            ExprKind::Const(constant) => self.gen_constant_expr(constant, scalar_ty, dest),
            ExprKind::BindRef(bind_ref) => self.gen_bind_ref_expr(bind_ref, dest),
            _ => unimplemented!(),
        }
    }

    fn gen_constant_expr(&mut self, constant: Const, ty: ScalarType, dest: ScalarReg) {
        match constant {
            Const::Integer { value, .. } => self.emit(Inst::Ldi {
                ty,
                dest,
                imm: value as i32,
            }),
        }
    }

    fn gen_bind_def_expr(&mut self, bind_def: BindDef) {
        let dest = self.alloc_scalar_reg();

        self.gen_expr_to_reg(bind_def.value, dest);

        self.get_this_scope_mut()
            .insert(bind_def.identifier, Binding { reg: dest });
    }

    fn gen_bind_ref_expr(&mut self, bind_ref: BindRef, dest: ScalarReg) {
        let source = self.find_in_scope(bind_ref.identifier).reg;

        self.emit(Inst::Mov { dest, source });
    }

    fn gen_assign_expr(&mut self, assign_expr: AssignExpr) {
        let binding = self.find_in_scope(assign_expr.identifier);
        let value_reg = self.alloc_scalar_reg();

        self.gen_expr_to_reg(assign_expr.value, value_reg);

        self.emit(Inst::Mov {
            dest: binding.reg,
            source: value_reg,
        });
    }

    fn gen_compound_expr(&mut self, compound_expr: CompoundExpr) {
        self.enter_scope();

        for expr in compound_expr.exprs {
            self.gen_expr(expr);
        }

        self.exit_scope();
    }

    fn enter_scope(&mut self) {
        self.scope_stack.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn get_this_scope_mut(&mut self) -> &mut HashMap<Symbol, Binding> {
        self.scope_stack.last_mut().unwrap()
    }

    fn find_in_scope(&self, identifier: Symbol) -> Binding {
        self.scope_stack
            .iter()
            .rev()
            .find_map(|scope| scope.get(&identifier).copied())
            .unwrap()
    }

    fn alloc_scalar_reg(&mut self) -> ScalarReg {
        let reg = ScalarReg(self.scalar_frame_size);
        self.scalar_frame_size += 1;

        reg
    }

    fn gen_return_expr(&mut self) {
        self.emit(Inst::Ret);
    }

    fn emit(&mut self, inst: Inst) {
        self.emitted_insts.push(inst);
    }
}

fn scalar_type_of(ty: Type) -> ScalarType {
    match ty {
        Type::U8 => ScalarType::U8,
        Type::U16 => ScalarType::U16,
        Type::U32 => ScalarType::U32,
        Type::U64 => ScalarType::U64,
        Type::I8 => ScalarType::I8,
        Type::I16 => ScalarType::I16,
        Type::I32 => ScalarType::I32,
        Type::I64 => ScalarType::I64,
        Type::IntLiteral | Type::Unit | Type::Never | Type::Function => {
            unreachable!("{ty:?} is not a scalar type")
        }
    }
}

#[derive(Clone, Copy)]
struct Binding {
    reg: ScalarReg,
}

pub(crate) struct MbcProgram<'ctx> {
    ctx: &'ctx CompilerContext,
    functions: Vec<MbcFunction>,
}

struct MbcFunction {
    name: Symbol,
    scalar_frame_size: usize,
    handle_frame_size: usize,
    insts: Vec<Inst>,
}

#[derive(Clone, Copy)]
enum Inst {
    Ldi {
        ty: ScalarType,
        dest: ScalarReg,
        imm: i32,
    },
    Mov {
        dest: ScalarReg,
        source: ScalarReg,
    },
    Ret,
}

#[derive(Clone, Copy)]
struct ScalarReg(usize);

#[derive(Clone, Copy)]
enum ScalarType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

impl fmt::Display for MbcProgram<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for function in &self.functions {
            writeln!(
                f,
                "fn @{} () frame s:{} h:{}",
                self.ctx.resolve_symbol(function.name),
                function.scalar_frame_size,
                function.handle_frame_size
            )?;

            for inst in &function.insts {
                writeln!(f, "    {}", inst)?;
            }

            writeln!(f, "end")?;
        }

        Ok(())
    }
}

impl fmt::Display for Inst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inst::Ldi { ty, dest, imm } => {
                write!(f, "ldi.{ty} {dest}, {imm}")
            }
            Inst::Mov { dest, source } => write!(f, "mov {dest}, {source}"),
            Inst::Ret => write!(f, "ret"),
        }
    }
}

impl fmt::Display for ScalarReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalarType::U8 => write!(f, "u8"),
            ScalarType::U16 => write!(f, "u16"),
            ScalarType::U32 => write!(f, "u32"),
            ScalarType::U64 => write!(f, "u64"),
            ScalarType::I8 => write!(f, "i8"),
            ScalarType::I16 => write!(f, "i16"),
            ScalarType::I32 => write!(f, "i32"),
            ScalarType::I64 => write!(f, "i64"),
        }
    }
}
