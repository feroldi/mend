use crate::ast::*;
use crate::compiler_context::CompilerContext;
use crate::scanner::{Delim, Token, TokenKind};

pub(crate) struct Parser<'ctx> {
    ctx: &'ctx CompilerContext,
    tokens: Vec<Token>,
    current_token_idx: usize,
}

impl<'ctx> Parser<'ctx> {
    pub(crate) fn new(tokens: Vec<Token>, ctx: &'ctx CompilerContext) -> Parser<'ctx> {
        Parser {
            ctx,
            tokens,
            current_token_idx: 0,
        }
    }

    pub(crate) fn parse_program(&mut self) -> Option<Program<'ctx>> {
        let mut decls = vec![];

        // In the future, diagnostics will be added to a diags vector from parse_ functions as
        // `Result<..., Diagnostic>`.
        while let Some(decl) = self.parse_decl() {
            decls.push(decl);
        }

        Some(Program {
            decls: self.ctx.alloc_slice_of_decl(&decls),
        })
    }

    fn parse_decl(&mut self) -> Option<Decl<'ctx>> {
        let ident_tok = self.consume()?;
        debug_assert_eq!(ident_tok.kind, TokenKind::Identifier);

        let op = self.consume()?;
        debug_assert_eq!(op.kind, TokenKind::ColonColon);

        let expr = self.parse_expr()?;

        let identifier = self.ctx.get_or_intern_str(
            &self.ctx.get_source_code()[ident_tok.span.start.0..ident_tok.span.end.0],
        );

        Some(Decl {
            identifier,
            value: self.ctx.alloc_expr(expr),
        })
    }

    fn parse_expr(&mut self) -> Option<Expr<'ctx>> {
        let tok = self.consume()?;

        match tok.kind {
            TokenKind::Open(Delim::Paren) => self.parse_function(),
            _ => None,
        }
    }

    fn parse_function(&mut self) -> Option<Expr<'ctx>> {
        let closed_paren = self.consume()?;
        debug_assert_eq!(closed_paren.kind, TokenKind::Closed(Delim::Paren));

        let open_curly_tok = self.consume()?;
        debug_assert_eq!(open_curly_tok.kind, TokenKind::Open(Delim::Curly));

        let compound_expr = self.parse_compound_expr(open_curly_tok)?;

        Some(Expr::Function(Function {
            body: compound_expr,
        }))
    }

    fn parse_compound_expr(&mut self, open_curly_tok: Token) -> Option<CompoundExpr<'ctx>> {
        debug_assert_eq!(open_curly_tok.kind, TokenKind::Open(Delim::Curly));

        let mut exprs = vec![];

        while self.peek()?.kind != TokenKind::Closed(Delim::Curly) {
            let expr = self.parse_expr()?;
            exprs.push(expr);
        }

        let closed_curly_tok = self.consume()?;
        debug_assert_eq!(closed_curly_tok.kind, TokenKind::Closed(Delim::Curly));

        Some(CompoundExpr {
            exprs: self.ctx.alloc_slice_of_expr(&exprs),
        })
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.current_token_idx).copied()
    }

    fn consume(&mut self) -> Option<Token> {
        let peeked_tok = self.peek();

        if self.current_token_idx < self.tokens.len() {
            self.current_token_idx += 1;
        }

        peeked_tok
    }
}
