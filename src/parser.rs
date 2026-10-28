use crate::ast::*;
use crate::compiler_context::CompilerContext;
use crate::constant_parser::{NumConst, parse_numeric_constant};
use crate::diagnostics::{Diag, Diagnostic, DiagnosticBag};
use crate::scanner::{Delim, Keyword, Scanner, Token, TokenKind};
use crate::sema::Sema;

pub(crate) struct Parser<'ctx> {
    ctx: &'ctx CompilerContext,
    sema: Sema<'ctx>,
    scanner: Scanner<'ctx>,
    cached_peek_tok: Option<Token>,
}

impl<'ctx> Parser<'ctx> {
    pub(crate) fn new(scanner: Scanner<'ctx>, ctx: &'ctx CompilerContext) -> Parser<'ctx> {
        Parser {
            ctx,
            sema: Sema::new(ctx),
            scanner,
            cached_peek_tok: None,
        }
    }

    pub(crate) fn parse_program(&mut self) -> Result<Program<'ctx>, DiagnosticBag> {
        let mut decls = vec![];

        while self.peek_tok()?.kind != TokenKind::Eof {
            let decl = self.parse_decl()?;
            decls.push(decl);
        }

        let program = Program {
            decls: self.ctx.alloc_slice_of_decl(&decls),
        };

        self.sema.act_on_program(program)
    }

    fn parse_decl(&mut self) -> Result<Decl<'ctx>, DiagnosticBag> {
        let ident_tok = self.expect_and_consume_tok(TokenKind::Identifier)?;

        self.expect_and_consume_tok(TokenKind::ColonColon)?;

        let expr = self.parse_statement_expr()?;

        let identifier = self
            .ctx
            .get_or_intern_str(self.ctx.get_source_map().get_text_snippet(ident_tok.span));

        Ok(Decl {
            identifier,
            identifier_span: ident_tok.span,
            value: self.ctx.alloc_expr(expr),
        })
    }

    fn parse_statement_expr(&mut self) -> Result<Expr<'ctx>, DiagnosticBag> {
        let tok = self.consume_tok()?;

        match tok.kind {
            TokenKind::NumericConstant => self.parse_numeric_constant(tok),
            TokenKind::Identifier => self.parse_identifier_expr(tok),
            TokenKind::Keyword(Keyword::Mut) => self.parse_mut_bind_def_expr(),
            TokenKind::Keyword(Keyword::Return) => self.parse_return_expr(),
            TokenKind::Open(Delim::Paren) => self.parse_function(),
            found => Err(Diagnostic::builder(Diag::ExpectedExpression { found })
                .label(tok.span, "expected expression")
                .build()
                .into()),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr<'ctx>, DiagnosticBag> {
        let stmt_expr = self.parse_statement_expr()?;

        if self.try_consume_tok(TokenKind::Semi)? {
            Ok(Expr::Semi(self.ctx.alloc_expr(stmt_expr)))
        } else {
            Ok(stmt_expr)
        }
    }

    fn parse_numeric_constant(&mut self, tok: Token) -> Result<Expr<'ctx>, DiagnosticBag> {
        let lexeme = self.ctx.get_source_map().get_text_snippet(tok.span);

        match parse_numeric_constant(lexeme).num_const {
            NumConst::Int(integer) => Ok(Expr::Const(Const::Integer {
                value: integer.value,
            })),
        }
    }

    fn parse_identifier_expr(&mut self, ident_tok: Token) -> Result<Expr<'ctx>, DiagnosticBag> {
        if matches!(
            self.peek_tok()?.kind,
            TokenKind::Colon | TokenKind::ColonEqual
        ) {
            return self.parse_bind_def_expr(ident_tok, false);
        }

        let identifier = self
            .ctx
            .get_or_intern_str(self.ctx.get_source_map().get_text_snippet(ident_tok.span));

        self.sema
            .act_on_bind_ref(BindRef { identifier }, ident_tok.span)
    }

    fn parse_mut_bind_def_expr(&mut self) -> Result<Expr<'ctx>, DiagnosticBag> {
        let ident_tok = self.expect_and_consume_tok(TokenKind::Identifier)?;

        self.parse_bind_def_expr(ident_tok, true)
    }

    fn parse_bind_def_expr(
        &mut self,
        ident_tok: Token,
        is_mutable: bool,
    ) -> Result<Expr<'ctx>, DiagnosticBag> {
        let ty = if self.try_consume_tok(TokenKind::Colon)? {
            let ty = self.parse_type()?;
            self.expect_and_consume_tok(TokenKind::Equal)?;

            Some(ty)
        } else {
            self.expect_and_consume_tok(TokenKind::ColonEqual)?;

            None
        };

        let value = self.parse_statement_expr()?;

        let identifier = self
            .ctx
            .get_or_intern_str(self.ctx.get_source_map().get_text_snippet(ident_tok.span));

        Ok(self.sema.act_on_bind_def(BindDef {
            identifier,
            is_mutable,
            ty,
            value: self.ctx.alloc_expr(value),
        }))
    }

    fn parse_type(&mut self) -> Result<Type, DiagnosticBag> {
        let ident_tok = self.expect_and_consume_tok(TokenKind::Identifier)?;

        match self.ctx.get_source_map().get_text_snippet(ident_tok.span) {
            "u8" => Ok(Type::U8),
            _ => todo!("other types"),
        }
    }

    fn parse_return_expr(&mut self) -> Result<Expr<'ctx>, DiagnosticBag> {
        Ok(Expr::Return)
    }

    fn parse_function(&mut self) -> Result<Expr<'ctx>, DiagnosticBag> {
        self.expect_and_consume_tok(TokenKind::Closed(Delim::Paren))?;

        let open_curly_tok = self.expect_and_consume_tok(TokenKind::Open(Delim::Curly))?;

        let compound_expr = self.parse_compound_expr(open_curly_tok)?;

        Ok(Expr::Function(Function {
            body: compound_expr,
        }))
    }

    fn parse_compound_expr(
        &mut self,
        open_curly_tok: Token,
    ) -> Result<CompoundExpr<'ctx>, DiagnosticBag> {
        debug_assert_eq!(open_curly_tok.kind, TokenKind::Open(Delim::Curly));

        let mut exprs = vec![];

        self.sema.enter_scope();

        while self.peek_tok()?.kind != TokenKind::Closed(Delim::Curly) {
            let expr = self.parse_expr()?;
            exprs.push(expr);
        }

        self.sema.exit_scope();

        let closed_curly_tok = self.consume_tok()?;
        debug_assert_eq!(closed_curly_tok.kind, TokenKind::Closed(Delim::Curly));

        Ok(CompoundExpr {
            exprs: self.ctx.alloc_slice_of_expr(&exprs),
        })
    }

    fn expect_and_consume_tok(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        let tok = self.peek_tok()?;

        if tok.kind != kind {
            let diag = Diag::ExpectedButFound {
                expected: kind,
                found: tok.kind,
            };

            return Err(Diagnostic::builder(diag)
                .label(tok.span, format!("expected {kind}"))
                .build());
        }

        self.consume_tok()
    }

    fn consume_tok(&mut self) -> Result<Token, Diagnostic> {
        // Guarantees that `self.cached_peek_tok` has a peeked token.
        self.peek_tok()?;
        debug_assert!(self.cached_peek_tok.is_some());

        let peeked_tok = self.cached_peek_tok.unwrap();

        if peeked_tok.kind != TokenKind::Eof {
            self.cached_peek_tok = None;
        }

        Ok(peeked_tok)
    }

    fn peek_tok(&mut self) -> Result<Token, Diagnostic> {
        match self.cached_peek_tok {
            Some(peeked_tok) => Ok(peeked_tok),
            _ => {
                let peeked_tok = self.scanner.scan_next_token()?;
                self.cached_peek_tok = Some(peeked_tok);

                Ok(peeked_tok)
            }
        }
    }

    fn try_consume_tok(&mut self, kind: TokenKind) -> Result<bool, Diagnostic> {
        if self.peek_tok()?.kind == kind {
            self.consume_tok()?;

            Ok(true)
        } else {
            Ok(false)
        }
    }
}
