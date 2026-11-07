use std::cmp::Ordering;

use bumpalo::Bump;

use crate::big_num::{BigFloat, BigInt};

fn big_int_from_decimal<'a>(arena: &'a Bump, bit_width: u32, digits: &str) -> BigInt<'a> {
    let mut value = BigInt::new(arena, bit_width, 0);

    for digit in digits.bytes() {
        value *= 10;
        value += (digit - b'0') as u32;
    }

    value
}

#[test]
fn test_zero_big_int() {
    let arena = Bump::new();
    let zero = BigInt::new(&arena, 64, 0);

    assert!(zero.is_zero());
    assert_eq!(zero.to_string(), "0");
    assert_eq!(zero.bit_width(), 64);
}

#[test]
fn test_big_int_from_u64() {
    let arena = Bump::new();

    assert_eq!(BigInt::new(&arena, 64, 42).to_string(), "42");
    assert_eq!(
        BigInt::new(&arena, 64, u64::MAX).to_string(),
        "18446744073709551615"
    );
}

#[test]
fn test_big_int_truncates_to_bit_width() {
    let arena = Bump::new();

    assert_eq!(BigInt::new(&arena, 8, 0x1FF).to_string(), "255");
    assert_eq!(BigInt::new(&arena, 1, 2).to_string(), "0");
}

#[test]
fn test_big_int_mul_add_past_one_word() {
    let arena = Bump::new();
    let value = big_int_from_decimal(&arena, 129, "340282366920938463463374607431768211456");

    assert_eq!(value.to_string(), "340282366920938463463374607431768211456");
    assert!(value > BigInt::new(&arena, 64, u64::MAX));
}

#[test]
fn test_big_int_mul_add_wraps_at_bit_width() {
    let arena = Bump::new();

    let mut value = BigInt::new(&arena, 8, 200);
    value *= 2;
    assert_eq!(value.to_string(), "144");

    value += 120;
    assert_eq!(value.to_string(), "8");
}

#[test]
fn test_big_int_shl() {
    let arena = Bump::new();

    assert_eq!((BigInt::new(&arena, 8, 1) << 0).to_string(), "1");
    assert_eq!((BigInt::new(&arena, 8, 3) << 4).to_string(), "48");
    assert_eq!(
        (BigInt::new(&arena, 129, 1) << 128).to_string(),
        "340282366920938463463374607431768211456"
    );
    assert_eq!((BigInt::new(&arena, 8, 3) << 7).to_string(), "128");
    assert!((BigInt::new(&arena, 8, 1) << 8).is_zero());
    assert!((BigInt::new(&arena, 200, 0) << 100).is_zero());
}

#[test]
fn test_big_int_zext() {
    let arena = Bump::new();
    let value = BigInt::new(&arena, 8, 255).zext(200);

    assert_eq!(value.bit_width(), 200);
    assert_eq!(value.to_string(), "255");
    assert_eq!(
        (value << 192).to_string(),
        (BigInt::new(&arena, 200, 255) << 192).to_string()
    );
}

#[test]
fn test_big_int_active_bits() {
    let arena = Bump::new();

    assert_eq!(BigInt::new(&arena, 64, 0).active_bits(), 0);
    assert_eq!(BigInt::new(&arena, 64, 1).active_bits(), 1);
    assert_eq!(BigInt::new(&arena, 64, 255).active_bits(), 8);
    assert_eq!((BigInt::new(&arena, 200, 1) << 150).active_bits(), 151);
}

#[test]
fn test_big_int_ordering_ignores_bit_width() {
    let arena = Bump::new();
    let small = BigInt::new(&arena, 64, u64::MAX);
    let large = BigInt::new(&arena, 65, 1) << 64;

    assert_eq!(small.cmp(&large), Ordering::Less);
    assert_eq!(large.cmp(&small), Ordering::Greater);
    assert_eq!(BigInt::new(&arena, 8, 7), BigInt::new(&arena, 200, 7));
    assert!(BigInt::new(&arena, 200, 0) < BigInt::new(&arena, 1, 1));
}

#[test]
fn test_big_int_is_copy() {
    let arena = Bump::new();
    let value = BigInt::new(&arena, 64, 7);
    let mut copy = value;

    copy *= 6;

    assert_eq!(value.to_string(), "7");
    assert_eq!(copy.to_string(), "42");
}

#[test]
fn test_big_float_normalizes_trailing_zeros() {
    let arena = Bump::new();

    assert_eq!(
        BigFloat::new(BigInt::new(&arena, 64, 20), -4),
        BigFloat::new(BigInt::new(&arena, 64, 2), -3)
    );
    assert_eq!(
        BigFloat::new(BigInt::new(&arena, 64, 0), 5),
        BigFloat::new(BigInt::new(&arena, 64, 0), 0)
    );
}

#[test]
fn test_big_float_display_plain() {
    let arena = Bump::new();
    let float = |mantissa, exponent| BigFloat::new(BigInt::new(&arena, 64, mantissa), exponent);

    assert_eq!(float(15, -1).to_string(), "1.5");
    assert_eq!(float(20, -4).to_string(), "0.002");
    assert_eq!(float(1, 3).to_string(), "1000.0");
    assert_eq!(float(1, -4).to_string(), "0.0001");
    assert_eq!(float(123, 13).to_string(), "1230000000000000.0");
    assert_eq!(float(0, 0).to_string(), "0.0");
}

#[test]
fn test_big_float_display_scientific() {
    let arena = Bump::new();
    let float = |mantissa, exponent| BigFloat::new(BigInt::new(&arena, 64, mantissa), exponent);

    assert_eq!(float(1, 16).to_string(), "1e16");
    assert_eq!(float(1, -5).to_string(), "1e-5");
    assert_eq!(float(15, -6).to_string(), "1.5e-5");
    assert_eq!(float(1, 999_999_999).to_string(), "1e999999999");
}

#[test]
fn test_big_float_compared_with_big_int() {
    let arena = Bump::new();
    let float = |mantissa, exponent| BigFloat::new(BigInt::new(&arena, 64, mantissa), exponent);
    let int = |value| BigInt::new(&arena, 64, value);

    assert!(float(25, -1) > int(2));
    assert!(float(25, -1) < int(3));
    assert!(float(3, 0) == int(3));
    assert!(float(3, 2) == int(300));
    assert!(float(0, 0) == int(0));
    assert!(float(0, 0) < int(2));
}

#[test]
fn test_big_float_compared_with_big_int_with_huge_exponents() {
    let arena = Bump::new();
    let float = |exponent| BigFloat::new(BigInt::new(&arena, 64, 1), exponent);
    let threshold = BigInt::new(&arena, 1025, 1) << 1024;

    assert!(float(999_999_999) > threshold);
    assert!(float(-999_999_999) < threshold);
    assert!(float(i64::MAX) > threshold);
    assert!(float(i64::MIN) < threshold);
}
