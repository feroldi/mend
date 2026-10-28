use std::fmt;

use crate::char_stream::CharStream;
use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diag, Diagnostic};
use crate::source_map::{BytePos, Span};

pub(crate) struct Scanner<'ctx> {
    ctx: &'ctx CompilerContext,
    char_stream: CharStream<'ctx>,
}

impl<'ctx> Scanner<'ctx> {
    pub(crate) fn new(ctx: &'ctx CompilerContext) -> Scanner<'ctx> {
        Scanner {
            ctx,
            char_stream: CharStream::with_text(ctx.get_source_map().get_text()),
        }
    }

    pub(crate) fn scan_next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_whitespace();

        let span_start = self.char_stream.peek_byte_pos();

        let token_kind = match self.bump() {
            CharStream::EOF_CHAR => TokenKind::Eof,
            ';' => TokenKind::Semi,
            ':' if self.peek() == ':' => {
                self.bump();

                TokenKind::ColonColon
            }
            '(' => TokenKind::Open(Delim::Paren),
            ')' => TokenKind::Closed(Delim::Paren),
            '{' => TokenKind::Open(Delim::Curly),
            '}' => TokenKind::Closed(Delim::Curly),
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(span_start),
            ch => {
                let span = Span {
                    start: span_start,
                    end: self.char_stream.peek_byte_pos(),
                };

                return Err(Diagnostic::builder(Diag::UnknownCharacter(ch))
                    .label(span, "unknown character")
                    .build());
            }
        };

        let token_span = Span {
            start: span_start,
            end: self.char_stream.peek_byte_pos(),
        };

        Ok(Token {
            kind: token_kind,
            span: token_span,
        })
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_ascii_whitespace() {
            self.bump();
        }
    }

    fn scan_identifier(&mut self, ident_span_start: BytePos) -> TokenKind {
        while matches!(self.peek(), 'a'..='z' | 'A'..='Z' | '_' | '0'..='9') {
            self.bump();
        }

        let ident_text = self.ctx.get_source_map().get_text_snippet(Span {
            start: ident_span_start,
            end: self.char_stream.peek_byte_pos(),
        });

        match ident_text {
            "return" => TokenKind::Keyword(Keyword::Return),
            _ => TokenKind::Identifier,
        }
    }

    fn peek(&mut self) -> char {
        self.char_stream.peek()
    }

    fn bump(&mut self) -> char {
        self.char_stream.consume()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum TokenKind {
    Eof,
    Identifier,
    ColonColon,
    Semi,
    Keyword(Keyword),
    Open(Delim),
    Closed(Delim),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Keyword {
    Return,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Delim {
    Paren,
    Curly,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Eof => write!(f, "end of file"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::ColonColon => write!(f, "`::`"),
            TokenKind::Semi => write!(f, "`;`"),
            TokenKind::Keyword(Keyword::Return) => write!(f, "`return`"),
            TokenKind::Open(Delim::Paren) => write!(f, "`(`"),
            TokenKind::Closed(Delim::Paren) => write!(f, "`)`"),
            TokenKind::Open(Delim::Curly) => write!(f, "`{{`"),
            TokenKind::Closed(Delim::Curly) => write!(f, "`}}`"),
        }
    }
}
