#[derive(PartialEq, Eq, Debug)]
pub(crate) enum NumConst<'a> {
    Int(IntegerConst<'a>),
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct IntegerConst<'a> {
    pub(crate) value: u64,
    pub(crate) has_overflowed: bool,
    pub(crate) suffix: Option<&'a str>,
}

pub(crate) fn parse_numeric_constant(token_lexeme: &str) -> NumConst<'_> {
    let attrs = parse_num_const_attributes(token_lexeme);
    let suffix = attrs.suffix;

    let (value, has_overflowed) = eval_integer_constant(attrs);

    NumConst::Int(IntegerConst {
        value,
        has_overflowed,
        suffix,
    })
}

struct NumConstAttrs<'a> {
    radix: u64,
    digits: &'a [u8],
    suffix: Option<&'a str>,
}

fn parse_num_const_attributes(token_lexeme: &str) -> NumConstAttrs<'_> {
    let (radix, prefix_len) = match token_lexeme.as_bytes() {
        [b'0', b'x', ..] => (16, 2),
        [b'0', b'b', ..] => (2, 2),
        [b'0', b'o', ..] => (8, 2),
        _ => (10, 0),
    };

    let digits_and_suffix = &token_lexeme[prefix_len..];

    let digits_len = digits_and_suffix
        .bytes()
        .take_while(|&ch| ch == b'_' || (ch as char).is_digit(radix as u32))
        .count();

    let (digits, suffix) = digits_and_suffix.split_at(digits_len);

    NumConstAttrs {
        radix,
        digits: digits.as_bytes(),
        suffix: (!suffix.is_empty()).then_some(suffix),
    }
}

fn eval_integer_constant(attrs: NumConstAttrs) -> (u64, bool) {
    let mut value = 0u64;
    let mut has_overflowed = false;

    for &digit in attrs.digits {
        if digit == b'_' {
            continue;
        }

        let (low_mul, overflowed) = value.overflowing_mul(attrs.radix);
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
