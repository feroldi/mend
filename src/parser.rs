use crate::ast::*;
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diagnostic, DiagnosticBag, DiagnosticCode, Label};
use crate::scanner::{Delim, Keyword, Token, TokenKind};
use crate::sema::Sema;
use crate::source_map::{BytePos, Span};

pub(crate) struct Parser<'ctx> {
    ctx: &'ctx CompilerContext,
    sema: Sema<'ctx>,
    tokens: Vec<Token>,
    current_token_idx: usize,
}

impl<'ctx> Parser<'ctx> {
    pub(crate) fn new(tokens: Vec<Token>, ctx: &'ctx CompilerContext) -> Parser<'ctx> {
        Parser {
            ctx,
            sema: Sema::new(ctx),
            tokens,
            current_token_idx: 0,
        }
    }

    pub(crate) fn parse_program(&mut self) -> Result<Program<'ctx>, DiagnosticBag> {
        let mut decls = vec![];

        while self.peek().is_some() {
            let decl = self.parse_decl()?;
            decls.push(decl);
        }

        let program = Program {
            decls: self.ctx.alloc_slice_of_decl(&decls),
        };

        self.sema.act_on_program(program)
    }

    fn parse_decl(&mut self) -> Result<Decl<'ctx>, DiagnosticBag> {
        let ident_tok = self.expect(TokenKind::Identifier, DiagnosticCode::E0002, "identifier")?;

        self.expect(TokenKind::ColonColon, DiagnosticCode::E0003, "`::`")?;

        let expr_tok = self.peek();
        let expr = self.parse_statement_expr().ok_or_else(|| {
            self.make_expected_diagnostic(DiagnosticCode::E0004, "expression", expr_tok)
        })?;

        let identifier = self
            .ctx
            .get_or_intern_str(self.ctx.get_source_map().get_text_snippet(ident_tok.span));

        Ok(Decl {
            identifier,
            value: self.ctx.alloc_expr(expr),
        })
    }

    fn parse_statement_expr(&mut self) -> Option<Expr<'ctx>> {
        let tok = self.consume()?;

        match tok.kind {
            TokenKind::Keyword(Keyword::Return) => self.parse_return_expr(),
            TokenKind::Open(Delim::Paren) => self.parse_function(),
            _ => None,
        }
    }

    fn parse_expr(&mut self) -> Option<Expr<'ctx>> {
        let stmt_expr = self.parse_statement_expr()?;

        if self.peek()?.kind == TokenKind::Semi {
            self.consume()?;

            Some(Expr::Semi(self.ctx.alloc_expr(stmt_expr)))
        } else {
            Some(stmt_expr)
        }
    }

    fn parse_return_expr(&mut self) -> Option<Expr<'ctx>> {
        Some(Expr::Return)
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

    fn expect(
        &mut self,
        kind: TokenKind,
        code: DiagnosticCode,
        expected: &str,
    ) -> Result<Token, DiagnosticBag> {
        match self.peek() {
            Some(tok) if tok.kind == kind => {
                self.consume();

                Ok(tok)
            }
            found_tok => Err(self.make_expected_diagnostic(code, expected, found_tok)),
        }
    }

    fn make_expected_diagnostic(
        &self,
        code: DiagnosticCode,
        expected: &str,
        found_tok: Option<Token>,
    ) -> DiagnosticBag {
        let (found, span) = match found_tok {
            Some(tok) => (
                format!("`{}`", self.ctx.get_source_map().get_text_snippet(tok.span)),
                tok.span,
            ),
            None => {
                let end_of_file = self.tokens.last().map_or(0, |tok| tok.span.end.0);

                (
                    "end of file".into(),
                    Span {
                        start: BytePos(end_of_file),
                        end: BytePos(end_of_file),
                    },
                )
            }
        };

        let mut diagnostics = DiagnosticBag::new();

        diagnostics.add(Diagnostic {
            code,
            message: format!("expected {expected}, found {found}"),
            label: Some(Label {
                span,
                message: format!("expected {expected}"),
            }),
            helps: vec![],
        });

        diagnostics
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
