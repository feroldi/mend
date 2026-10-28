use std::cell::RefCell;

use bumpalo::Bump;

use crate::ast::{Decl, Expr};
use crate::interner::{StringInterner, Symbol};
use crate::source_map::SourceMap;

pub(crate) struct CompilerContext {
    source_map: SourceMap,
    string_interner: RefCell<StringInterner>,
    exprs: Bump,
    decls: Bump,
}

impl<'ctx> CompilerContext {
    pub(crate) fn new(source_map: SourceMap) -> CompilerContext {
        CompilerContext {
            source_map,
            string_interner: Default::default(),
            exprs: Default::default(),
            decls: Default::default(),
        }
    }

    pub(crate) fn get_source_map(&'ctx self) -> &'ctx SourceMap {
        &self.source_map
    }

    pub(crate) fn get_or_intern_str(&'ctx self, string: &str) -> Symbol {
        self.string_interner.borrow_mut().get_or_intern(string)
    }

    pub(crate) fn resolve_symbol(&'ctx self, symbol: Symbol) -> &'static str {
        self.string_interner.borrow().resolve(symbol)
    }

    pub(crate) fn alloc_slice_of_decl<'a>(
        &'ctx self,
        decls: &'a [Decl<'ctx>],
    ) -> &'ctx [Decl<'ctx>] {
        self.decls.alloc_slice_copy(decls)
    }

    pub(crate) fn alloc_expr(&'ctx self, expr: Expr<'ctx>) -> &'ctx Expr<'ctx> {
        self.exprs.alloc(expr)
    }

    pub(crate) fn alloc_slice_of_expr<'a>(
        &'ctx self,
        exprs: &'a [Expr<'ctx>],
    ) -> &'ctx [Expr<'ctx>] {
        self.exprs.alloc_slice_copy(exprs)
    }
}
