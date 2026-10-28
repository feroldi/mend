use std::collections::HashMap;
use std::fmt;

use crate::ast::{BindDef, BindRef, CompoundExpr, Const, Decl, Expr, Function, Program, Type};
use crate::compiler_context::CompilerContext;
use crate::interner::Symbol;

pub(crate) struct CodeGen<'ctx> {
    ctx: &'ctx CompilerContext,
    emitted_insts: Vec<Inst>,
    scalar_frame_size: usize,
    reg_by_binding: HashMap<Symbol, ScalarReg>,
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> CodeGen<'ctx> {
        CodeGen {
            ctx,
            emitted_insts: vec![],
            scalar_frame_size: 0,
            reg_by_binding: HashMap::new(),
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
        match decl.value {
            Expr::Function(Function { body }) => self.gen_function(decl.identifier, *body),
            _ => todo!("other top-level exprs"),
        }
    }

    fn gen_function(&mut self, name: Symbol, body: CompoundExpr) -> MbcFunction {
        self.scalar_frame_size = 0;
        self.reg_by_binding.clear();

        for expr in body.exprs {
            self.gen_expr(expr);
        }

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
        match expr {
            Expr::Semi(expr) => self.gen_expr(expr),
            Expr::Return => self.gen_return_expr(),
            Expr::BindDef(bind_def) => self.gen_bind_def_expr(*bind_def),
            Expr::Const(_) | Expr::BindRef(_) | Expr::Function(_) => unimplemented!(),
        }
    }

    fn gen_expr_to_reg(&mut self, expr: &Expr, ty: ScalarType, dest: ScalarReg) {
        match expr {
            Expr::Const(constant) => self.gen_constant_expr(*constant, ty, dest),
            Expr::BindRef(bind_ref) => self.gen_bind_ref_expr(*bind_ref, dest),
            _ => unimplemented!(),
        }
    }

    fn gen_constant_expr(&mut self, constant: Const, ty: ScalarType, dest: ScalarReg) {
        match constant {
            Const::Integer { value } => self.emit(Inst::Ldi {
                ty,
                dest,
                imm: value as i32,
            }),
        }
    }

    fn gen_bind_def_expr(&mut self, bind_def: BindDef) {
        let dest = self.alloc_scalar_reg();

        let ty = match bind_def.ty {
            Some(Type::U8) => ScalarType::U8,
            None => ScalarType::I32,
        };

        self.gen_expr_to_reg(bind_def.value, ty, dest);

        self.reg_by_binding.insert(bind_def.identifier, dest);
    }

    fn gen_bind_ref_expr(&mut self, bind_ref: BindRef, dest: ScalarReg) {
        let source = self.reg_by_binding[&bind_ref.identifier];

        self.emit(Inst::Mov { dest, source });
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
    I32,
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
            ScalarType::I32 => write!(f, "i32"),
        }
    }
}
