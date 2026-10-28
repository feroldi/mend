use std::fmt;

use crate::ast::{CompoundExpr, Decl, Expr, Function, Program};
use crate::compiler_context::CompilerContext;
use crate::interner::Symbol;

pub(crate) struct CodeGen<'ctx> {
    ctx: &'ctx CompilerContext,
}

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> CodeGen<'ctx> {
        CodeGen { ctx }
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
        }
    }

    fn gen_function(&mut self, name: Symbol, body: CompoundExpr) -> MbcFunction {
        let mut insts: Vec<Inst> = body.exprs.iter().flat_map(|e| self.gen_expr(e)).collect();

        insts.push(Inst::Ret);

        MbcFunction {
            name,
            scalar_frame_size: 0,
            handle_frame_size: 0,
            insts,
        }
    }

    fn gen_expr(&mut self, expr: &Expr) -> Vec<Inst> {
        match expr {
            Expr::Function(_) => unimplemented!(),
        }
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
    Ret,
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
            Inst::Ret => write!(f, "ret"),
        }
    }
}
