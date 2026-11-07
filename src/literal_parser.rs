use bumpalo::Bump;

use crate::big_num::{BigFloat, BigInt};
use crate::source_map::BytePos;

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct ParsedIntegerLit<'a> {
    pub(crate) value: BigInt<'a>,
    pub(crate) suffix: Option<&'a str>,
    pub(crate) radix: u32,
    pub(crate) has_no_digits: bool,
    /// The byte offset, in the lexeme, of the first digit too large for the radix.
    pub(crate) invalid_digit_offset: Option<usize>,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct ParsedFloatLit<'a> {
    pub(crate) value: BigFloat<'a>,
    pub(crate) suffix: Option<&'a str>,
    pub(crate) exponent_missing_digits_loc: Option<BytePos>,
}

pub(crate) fn parse_integer<'a>(arena: &'a Bump, token_lexeme: &'a str) -> ParsedIntegerLit<'a> {
    let mut cursor = Cursor::new(token_lexeme);

    let radix = match (cursor.peek(), cursor.lookahead(1)) {
        (Some(b'0'), Some(b'x')) => 16,
        (Some(b'0'), Some(b'b')) => 2,
        (Some(b'0'), Some(b'o')) => 8,
        _ => 10,
    };

    if radix != 10 {
        cursor.bump_n(2);
    }

    let digits_start = cursor.pos;

    // For binary and octal integers, read all the decimal digits, so that they can be reported as
    // invalid digits. In the case of hexadecimal, all digits are valid, so letters are reported as
    // invalid suffixes instead.
    let digit_radix = radix.max(10);
    cursor.bump_while(|ch| ch == b'_' || (ch as char).is_digit(digit_radix));

    let digits = cursor.slice_from(digits_start);
    let suffix = cursor.rest();

    let has_no_digits = digits.bytes().all(|ch| ch == b'_');

    let invalid_digit_offset = digits
        .bytes()
        .position(|ch| ch != b'_' && !(ch as char).is_digit(radix))
        .map(|index| digits_start + index);

    ParsedIntegerLit {
        value: eval_digits(arena, &[digits], radix),
        suffix: (!suffix.is_empty()).then_some(suffix),
        radix,
        has_no_digits,
        invalid_digit_offset,
    }
}

pub(crate) fn parse_float<'a>(arena: &'a Bump, token_lexeme: &'a str) -> ParsedFloatLit<'a> {
    let mut cursor = Cursor::new(token_lexeme);

    let integer_digits_start = cursor.pos;
    cursor.bump_while(is_decimal_digit);
    let integer_digits = cursor.slice_from(integer_digits_start);

    let mut fraction_digits = "";

    if cursor.try_bump(|ch| ch == b'.') {
        let fraction_digits_start = cursor.pos;
        cursor.bump_while(is_decimal_digit);
        fraction_digits = cursor.slice_from(fraction_digits_start);
    }

    let mantissa = eval_digits(arena, &[integer_digits, fraction_digits], 10);
    let fraction_digit_count = count_digits(fraction_digits) as i64;

    let mut explicit_exponent = 0i64;
    let mut exponent_missing_digits_loc = None;

    let exponent_start = cursor.pos;

    if cursor.try_bump(|ch| matches!(ch, b'e' | b'E')) {
        let is_negative = cursor.peek() == Some(b'-');
        cursor.try_bump(|ch| matches!(ch, b'+' | b'-'));

        let exponent_digits_start = cursor.pos;
        cursor.bump_while(is_decimal_digit);
        let exponent_digits = cursor.slice_from(exponent_digits_start);

        if exponent_digits.bytes().all(|ch| ch == b'_') {
            exponent_missing_digits_loc = Some(BytePos::from_usize(exponent_start));
        }

        for digit in exponent_digits.bytes().filter(|&ch| ch != b'_') {
            explicit_exponent = explicit_exponent
                .saturating_mul(10)
                .saturating_add(convert_digit_to_int(digit) as i64);
        }

        if is_negative {
            explicit_exponent = -explicit_exponent;
        }
    }

    let suffix = cursor.rest();

    ParsedFloatLit {
        value: BigFloat::new(
            mantissa,
            explicit_exponent.saturating_sub(fraction_digit_count),
        ),
        suffix: (!suffix.is_empty()).then_some(suffix),
        exponent_missing_digits_loc,
    }
}

fn is_decimal_digit(ch: u8) -> bool {
    ch == b'_' || ch.is_ascii_digit()
}

fn eval_digits<'a>(arena: &'a Bump, digit_groups: &[&str], radix: u32) -> BigInt<'a> {
    let bits_per_digit = u32::BITS - (radix - 1).leading_zeros();
    let digit_count: usize = digit_groups.iter().map(|digits| count_digits(digits)).sum();
    let bit_width = (digit_count as u32 * bits_per_digit).max(1);

    let mut value = BigInt::new(arena, bit_width, 0);

    for digits in digit_groups {
        for digit in digits.bytes().filter(|&ch| ch != b'_') {
            value *= radix;
            value += convert_digit_to_int(digit);
        }
    }

    value
}

fn count_digits(digits: &str) -> usize {
    digits.bytes().filter(|&ch| ch != b'_').count()
}

fn convert_digit_to_int(digit: u8) -> u32 {
    match digit {
        b'0'..=b'9' => (digit - b'0') as u32,
        b'a'..=b'f' => (digit - b'a') as u32 + 10,
        b'A'..=b'F' => (digit - b'A') as u32 + 10,
        _ => unreachable!("should not convert '{}'", digit),
    }
}

/// Walks a numeric lexeme byte by byte. Numeric lexemes are ASCII, so each byte is a character.
struct Cursor<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Cursor<'a> {
        Cursor { text, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.lookahead(0)
    }

    fn lookahead(&self, n: usize) -> Option<u8> {
        self.text.as_bytes().get(self.pos + n).copied()
    }

    fn bump_n(&mut self, n: usize) {
        self.pos += n;
    }

    fn try_bump(&mut self, pred: impl Fn(u8) -> bool) -> bool {
        let is_bumped = self.peek().is_some_and(pred);

        if is_bumped {
            self.pos += 1;
        }

        is_bumped
    }

    fn bump_while(&mut self, pred: impl Fn(u8) -> bool) {
        while self.try_bump(&pred) {}
    }

    fn slice_from(&self, start: usize) -> &'a str {
        &self.text[start..self.pos]
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }
}
