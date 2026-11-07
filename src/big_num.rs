use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Shl};

use bumpalo::Bump;

const BITS_PER_WORD: u32 = u64::BITS;

#[derive(Clone, Copy)]
pub(crate) struct BigInt<'a> {
    arena: &'a Bump,
    bit_width: u32,
    words: Words<'a>,
}

#[derive(Clone, Copy)]
enum Words<'a> {
    Single(u64),
    Multi(&'a [u64]),
}

impl<'a> BigInt<'a> {
    pub(crate) fn new(arena: &'a Bump, bit_width: u32, value: u64) -> BigInt<'a> {
        BigInt::build(arena, bit_width, |words| words[0] = value)
    }

    pub(crate) fn bit_width(&self) -> u32 {
        self.bit_width
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.words().iter().all(|&word| word == 0)
    }

    pub(crate) fn active_bits(&self) -> u32 {
        let words = self.words();

        match words.iter().rposition(|&word| word != 0) {
            Some(index) => {
                index as u32 * BITS_PER_WORD + (BITS_PER_WORD - words[index].leading_zeros())
            }
            None => 0,
        }
    }

    pub(crate) fn zext(self, bit_width: u32) -> BigInt<'a> {
        debug_assert!(bit_width >= self.bit_width);

        let source = self.words();

        BigInt::build(self.arena, bit_width, |words| {
            words[..source.len()].copy_from_slice(source)
        })
    }

    fn div_rem_small(self, divisor: u32) -> (BigInt<'a>, u32) {
        let source = self.words();
        let divisor = divisor as u128;
        let mut remainder = 0u128;

        let quotient = BigInt::build(self.arena, self.bit_width, |words| {
            for (word, &source_word) in words.iter_mut().zip(source).rev() {
                let dividend = (remainder << BITS_PER_WORD) | source_word as u128;
                *word = (dividend / divisor) as u64;
                remainder = dividend % divisor;
            }
        });

        (quotient, remainder as u32)
    }

    fn count_decimal_digits(&self) -> usize {
        self.to_string().len()
    }

    fn words(&self) -> &[u64] {
        match &self.words {
            Words::Single(word) => std::slice::from_ref(word),
            Words::Multi(words) => words,
        }
    }

    fn build(arena: &'a Bump, bit_width: u32, fill: impl FnOnce(&mut [u64])) -> BigInt<'a> {
        debug_assert!(bit_width > 0);

        let word_count = bit_width.div_ceil(BITS_PER_WORD) as usize;

        if word_count == 1 {
            let mut word = [0u64];
            fill(&mut word);
            clear_unused_bits(&mut word, bit_width);

            return BigInt {
                arena,
                bit_width,
                words: Words::Single(word[0]),
            };
        }

        let words = arena.alloc_slice_fill_copy(word_count, 0u64);
        fill(words);
        clear_unused_bits(words, bit_width);

        BigInt {
            arena,
            bit_width,
            words: Words::Multi(words),
        }
    }
}

fn clear_unused_bits(words: &mut [u64], bit_width: u32) {
    let used_bits_in_last_word = bit_width % BITS_PER_WORD;

    if used_bits_in_last_word != 0 {
        *words.last_mut().unwrap() &= (1u64 << used_bits_in_last_word) - 1;
    }
}

impl<'a> Mul<u32> for BigInt<'a> {
    type Output = BigInt<'a>;

    fn mul(self, multiplier: u32) -> BigInt<'a> {
        let source = self.words();

        BigInt::build(self.arena, self.bit_width, |words| {
            let mut carry = 0u128;

            for (word, &source_word) in words.iter_mut().zip(source) {
                let product = source_word as u128 * multiplier as u128 + carry;
                *word = product as u64;
                carry = product >> BITS_PER_WORD;
            }
        })
    }
}

impl MulAssign<u32> for BigInt<'_> {
    fn mul_assign(&mut self, multiplier: u32) {
        *self = *self * multiplier;
    }
}

impl<'a> Add<u32> for BigInt<'a> {
    type Output = BigInt<'a>;

    fn add(self, addend: u32) -> BigInt<'a> {
        let source = self.words();

        BigInt::build(self.arena, self.bit_width, |words| {
            let mut carry = addend as u128;

            for (word, &source_word) in words.iter_mut().zip(source) {
                let sum = source_word as u128 + carry;
                *word = sum as u64;
                carry = sum >> BITS_PER_WORD;
            }
        })
    }
}

impl AddAssign<u32> for BigInt<'_> {
    fn add_assign(&mut self, addend: u32) {
        *self = *self + addend;
    }
}

impl<'a> Shl<u32> for BigInt<'a> {
    type Output = BigInt<'a>;

    fn shl(self, bits: u32) -> BigInt<'a> {
        let source = self.words();
        let word_shift = (bits / BITS_PER_WORD) as usize;
        let bit_shift = bits % BITS_PER_WORD;

        BigInt::build(self.arena, self.bit_width, |words| {
            for index in (word_shift..words.len()).rev() {
                let source_index = index - word_shift;
                let mut word = source[source_index] << bit_shift;

                if bit_shift != 0 && source_index > 0 {
                    word |= source[source_index - 1] >> (BITS_PER_WORD - bit_shift);
                }

                words[index] = word;
            }
        })
    }
}

impl<'b> PartialEq<BigInt<'b>> for BigInt<'_> {
    fn eq(&self, other: &BigInt<'b>) -> bool {
        cmp_words(self.words(), other.words()) == Ordering::Equal
    }
}

impl Eq for BigInt<'_> {}

impl<'b> PartialOrd<BigInt<'b>> for BigInt<'_> {
    fn partial_cmp(&self, other: &BigInt<'b>) -> Option<Ordering> {
        Some(cmp_words(self.words(), other.words()))
    }
}

impl Ord for BigInt<'_> {
    fn cmp(&self, other: &BigInt<'_>) -> Ordering {
        cmp_words(self.words(), other.words())
    }
}

fn cmp_words(lhs: &[u64], rhs: &[u64]) -> Ordering {
    let word_count = lhs.len().max(rhs.len());

    (0..word_count)
        .rev()
        .map(|index| {
            let lhs_word = lhs.get(index).copied().unwrap_or(0);
            let rhs_word = rhs.get(index).copied().unwrap_or(0);

            lhs_word.cmp(&rhs_word)
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

impl fmt::Display for BigInt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const CHUNK_DIVISOR: u32 = 1_000_000_000;

        let mut reversed_digits = String::new();
        let mut quotient = *self;

        loop {
            let (next_quotient, chunk) = quotient.div_rem_small(CHUNK_DIVISOR);
            quotient = next_quotient;

            if quotient.is_zero() {
                reversed_digits.extend(chunk.to_string().chars().rev());
                break;
            }

            reversed_digits.extend(format!("{chunk:09}").chars().rev());
        }

        write!(f, "{}", reversed_digits.chars().rev().collect::<String>())
    }
}

impl fmt::Debug for BigInt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}u{}", self, self.bit_width)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BigFloat<'a> {
    mantissa: BigInt<'a>,
    exponent: i64,
}

impl<'a> BigFloat<'a> {
    pub(crate) fn new(mut mantissa: BigInt<'a>, mut exponent: i64) -> BigFloat<'a> {
        if mantissa.is_zero() {
            return BigFloat {
                mantissa,
                exponent: 0,
            };
        }

        loop {
            let (quotient, remainder) = mantissa.div_rem_small(10);

            if remainder != 0 {
                break;
            }

            mantissa = quotient;
            exponent = exponent.saturating_add(1);
        }

        BigFloat { mantissa, exponent }
    }

    fn cmp_big_int(&self, other: &BigInt) -> Ordering {
        const BITS_PER_DECIMAL_DIGIT: u32 = 4;

        if self.mantissa.is_zero() || other.is_zero() {
            return self.mantissa.cmp(other);
        }

        let self_digit_count = self.mantissa.count_decimal_digits() as i128;
        let other_digit_count = other.count_decimal_digits() as i128;
        let self_magnitude = self_digit_count + self.exponent as i128;

        if self_magnitude < other_digit_count {
            return Ordering::Less;
        }

        if self_magnitude > other_digit_count {
            return Ordering::Greater;
        }

        let scale = self.exponent.unsigned_abs() as u32;

        if self.exponent >= 0 {
            let mut scaled_self = self
                .mantissa
                .zext(self.mantissa.bit_width() + scale * BITS_PER_DECIMAL_DIGIT);

            for _ in 0..scale {
                scaled_self *= 10;
            }

            scaled_self.cmp(other)
        } else {
            let source = other.words();
            let mut scaled_other = BigInt::build(
                self.mantissa.arena,
                other.bit_width() + scale * BITS_PER_DECIMAL_DIGIT,
                |words| words[..source.len()].copy_from_slice(source),
            );

            for _ in 0..scale {
                scaled_other *= 10;
            }

            self.mantissa.cmp(&scaled_other)
        }
    }
}

impl<'b> PartialEq<BigInt<'b>> for BigFloat<'_> {
    fn eq(&self, other: &BigInt<'b>) -> bool {
        self.cmp_big_int(other) == Ordering::Equal
    }
}

impl<'b> PartialOrd<BigInt<'b>> for BigFloat<'_> {
    fn partial_cmp(&self, other: &BigInt<'b>) -> Option<Ordering> {
        Some(self.cmp_big_int(other))
    }
}

impl fmt::Display for BigFloat<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mantissa.is_zero() {
            return write!(f, "0.0");
        }

        let digits = self.mantissa.to_string();
        let digit_count = digits.len() as i128;
        let scientific_exponent = digit_count - 1 + self.exponent as i128;

        if !(-4..16).contains(&scientific_exponent) {
            let (leading_digit, rest_digits) = digits.split_at(1);

            write!(f, "{leading_digit}")?;

            if !rest_digits.is_empty() {
                write!(f, ".{rest_digits}")?;
            }

            return write!(f, "e{scientific_exponent}");
        }

        if self.exponent >= 0 {
            return write!(f, "{digits}{}.0", "0".repeat(self.exponent as usize));
        }

        let point_pos = digit_count + self.exponent as i128;

        if point_pos > 0 {
            let (integer_digits, fraction_digits) = digits.split_at(point_pos as usize);

            write!(f, "{integer_digits}.{fraction_digits}")
        } else {
            write!(
                f,
                "0.{}{digits}",
                "0".repeat(point_pos.unsigned_abs() as usize)
            )
        }
    }
}
