use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

use crate::compiler_context::CompilerContext;
use crate::diagnostics::{Diagnostic, DiagnosticCode, Label};
use crate::source_map::{BytePos, Span};

pub(crate) struct Scanner<'ctx> {
    ctx: &'ctx CompilerContext,
    char_stream: Peekable<Chars<'ctx>>,
    current_peek_pos: BytePos,
}

impl<'ctx> Scanner<'ctx> {
    const EOF_CHAR: char = '\0';

    pub(crate) fn new(ctx: &'ctx CompilerContext) -> Scanner<'ctx> {
        Scanner {
            ctx,
            char_stream: ctx.get_source_map().get_text().chars().peekable(),
            current_peek_pos: BytePos(0),
        }
    }

    pub(crate) fn scan_next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_whitespace();

        let span_start = self.current_peek_pos;

        let token_kind = match self.bump() {
            Scanner::EOF_CHAR => TokenKind::Eof,
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
                return Err(Diagnostic {
                    code: DiagnosticCode::E0005,
                    message: format!("unknown character `{ch}`"),
                    label: Some(Label {
                        span: Span {
                            start: span_start,
                            end: self.current_peek_pos,
                        },
                        message: "unknown character".into(),
                    }),
                    helps: vec![],
                });
            }
        };

        let token_span = Span {
            start: span_start,
            end: self.current_peek_pos,
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
            end: self.current_peek_pos,
        });

        match ident_text {
            "return" => TokenKind::Keyword(Keyword::Return),
            _ => TokenKind::Identifier,
        }
    }

    fn peek(&mut self) -> char {
        self.char_stream
            .peek()
            .cloned()
            .unwrap_or(Scanner::EOF_CHAR)
    }

    fn bump(&mut self) -> char {
        let peeked = self.peek();

        if peeked != Scanner::EOF_CHAR {
            self.current_peek_pos = BytePos(self.current_peek_pos.0 + peeked.len_utf8());
            self.char_stream.next();
        }

        peeked
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
