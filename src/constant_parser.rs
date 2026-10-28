#[derive(PartialEq, Eq, Debug)]
pub(crate) struct ParseResult {
    pub(crate) num_const: NumConst,
    pub(crate) has_overflowed: bool,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum NumConst {
    Int(IntegerConst),
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct IntegerConst {
    pub(crate) value: u64,
}

pub(crate) fn parse_numeric_constant(token_lexeme: &str) -> ParseResult {
    let (value, has_overflowed) = eval_integer_constant(token_lexeme.as_bytes());

    ParseResult {
        num_const: NumConst::Int(IntegerConst { value }),
        has_overflowed,
    }
}

fn eval_integer_constant(digits: &[u8]) -> (u64, bool) {
    let mut value = 0u64;
    let mut has_overflowed = false;

    for &digit in digits {
        let (low_mul, overflowed) = value.overflowing_mul(10);
        has_overflowed |= overflowed;
        value = low_mul;

        let (low_add, overflowed) = value.overflowing_add((digit - b'0') as u64);
        has_overflowed |= overflowed;
        value = low_add;
    }

    (value, has_overflowed)
}
