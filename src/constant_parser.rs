#[derive(PartialEq, Eq, Debug)]
pub(crate) struct IntegerConst<'a> {
    pub(crate) value: u64,
    pub(crate) has_overflowed: bool,
    pub(crate) suffix: Option<&'a str>,
    pub(crate) radix: u32,
    pub(crate) has_no_digits: bool,
    /// The byte offset, in the lexeme, of the first digit too large for the radix.
    pub(crate) invalid_digit_offset: Option<usize>,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct FloatConst<'a> {
    /// The integer part, fraction and exponent, without the suffix.
    digits: &'a str,
    pub(crate) suffix: Option<&'a str>,
}

impl FloatConst<'_> {
    pub(crate) fn to_f32(&self) -> f32 {
        self.digits.replace('_', "").parse().unwrap()
    }

    pub(crate) fn to_f64(&self) -> f64 {
        self.digits.replace('_', "").parse().unwrap()
    }
}

pub(crate) fn parse_integer_constant(token_lexeme: &str) -> IntegerConst<'_> {
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

    let (value, has_overflowed) = eval_integer_constant(digits, radix);

    IntegerConst {
        value,
        has_overflowed,
        suffix: (!suffix.is_empty()).then_some(suffix),
        radix,
        has_no_digits,
        invalid_digit_offset,
    }
}

pub(crate) fn parse_float_constant(token_lexeme: &str) -> FloatConst<'_> {
    let mut cursor = Cursor::new(token_lexeme);

    cursor.bump_while(is_decimal_digit);

    if cursor.try_bump(|ch| ch == b'.') {
        cursor.bump_while(is_decimal_digit);
    }

    if cursor.try_bump(|ch| matches!(ch, b'e' | b'E')) {
        cursor.try_bump(|ch| matches!(ch, b'+' | b'-'));
        cursor.bump_while(is_decimal_digit);
    }

    let digits = cursor.slice_from(0);
    let suffix = cursor.rest();

    FloatConst {
        digits,
        suffix: (!suffix.is_empty()).then_some(suffix),
    }
}

fn is_decimal_digit(ch: u8) -> bool {
    ch == b'_' || ch.is_ascii_digit()
}

fn eval_integer_constant(digits: &str, radix: u32) -> (u64, bool) {
    let mut value = 0u64;
    let mut has_overflowed = false;
    let radix = radix as u64;

    for digit in digits.bytes() {
        if digit == b'_' {
            continue;
        }

        let (low_mul, overflowed) = value.overflowing_mul(radix);
        has_overflowed |= overflowed;
        value = low_mul;

        let (low_add, overflowed) = value.overflowing_add(convert_digit_to_int(digit));
        has_overflowed |= overflowed;
        value = low_add;
    }

    (value, has_overflowed)
}

fn convert_digit_to_int(digit: u8) -> u64 {
    match digit {
        b'0'..=b'9' => (digit - b'0') as u64,
        b'a'..=b'f' => (digit - b'a') as u64 + 10,
        b'A'..=b'F' => (digit - b'A') as u64 + 10,
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
