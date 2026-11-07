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
            ':' if self.peek() == '=' => {
                self.bump();

                TokenKind::ColonEqual
            }
            ':' => TokenKind::Colon,
            '=' => TokenKind::Equal,
            '(' => TokenKind::Open(Delim::Paren),
            ')' => TokenKind::Closed(Delim::Paren),
            '{' => TokenKind::Open(Delim::Curly),
            '}' => TokenKind::Closed(Delim::Curly),
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(span_start),
            ch @ '0'..='9' => self.scan_numeric_literal(ch),
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
            "mut" => TokenKind::Keyword(Keyword::Mut),
            "return" => TokenKind::Keyword(Keyword::Return),
            _ => TokenKind::Identifier,
        }
    }

    fn scan_numeric_literal(&mut self, first_digit: char) -> TokenKind {
        let is_decimal = !(first_digit == '0' && matches!(self.peek(), 'x' | 'b' | 'o'));

        let mut is_float = self.bump_numeric_literal_chars(is_decimal, first_digit);

        // A `.` is part of the literal only if a digit follows it, so that `0..10` is a range.
        if self.peek() == '.' && self.char_stream.lookahead(1).is_ascii_digit() {
            let period_ch = self.bump();
            is_float |= is_decimal;
            is_float |= self.bump_numeric_literal_chars(is_decimal, period_ch);
        }

        if is_float {
            TokenKind::FloatLiteral
        } else {
            TokenKind::IntegerLiteral
        }
    }

    /// Returns whether the chars have an exponent or a float suffix, which make a decimal literal
    /// a float.
    fn bump_numeric_literal_chars(&mut self, is_decimal: bool, mut prev_char: char) -> bool {
        let mut is_float = false;

        while matches!(self.peek(), 'a'..='z' | 'A'..='Z' | '_' | '0'..='9') {
            let ch = self.bump();

            if is_decimal && matches!(ch, 'e' | 'E' | 'f') && matches!(prev_char, '0'..='9' | '_') {
                is_float = true;

                if ch != 'f' && matches!(self.peek(), '+' | '-') {
                    self.bump();
                }
            }

            prev_char = ch;
        }

        is_float
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
    IntegerLiteral,
    FloatLiteral,
    Colon,
    ColonColon,
    ColonEqual,
    Equal,
    Semi,
    Keyword(Keyword),
    Open(Delim),
    Closed(Delim),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Keyword {
    Mut,
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
            TokenKind::IntegerLiteral => write!(f, "integer literal"),
            TokenKind::FloatLiteral => write!(f, "float literal"),
            TokenKind::Colon => write!(f, "`:`"),
            TokenKind::ColonColon => write!(f, "`::`"),
            TokenKind::ColonEqual => write!(f, "`:=`"),
            TokenKind::Equal => write!(f, "`=`"),
            TokenKind::Semi => write!(f, "`;`"),
            TokenKind::Keyword(Keyword::Mut) => write!(f, "`mut`"),
            TokenKind::Keyword(Keyword::Return) => write!(f, "`return`"),
            TokenKind::Open(Delim::Paren) => write!(f, "`(`"),
            TokenKind::Closed(Delim::Paren) => write!(f, "`)`"),
            TokenKind::Open(Delim::Curly) => write!(f, "`{{`"),
            TokenKind::Closed(Delim::Curly) => write!(f, "`}}`"),
        }
    }
}
