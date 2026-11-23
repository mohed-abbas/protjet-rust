//! The pure operations of B.3, the constant lookup and the jump check of B.4.
//!
//! Each is written once, here, and used by the engine ([`crate::run_turn`]) and by
//! anything else that must compute what the engine computes, such as a static analysis
//! folding constants: the two can then never disagree on a value.
//! Binary operations take `(a, b)` for a stack `a b`, `b` on top (B.3).

use crate::Fault;
use pbc::JUMPDEST;

pub(crate) fn add(a: i64, b: i64) -> i64 {
    a.wrapping_add(b)
}

pub(crate) fn sub(a: i64, b: i64) -> i64 {
    a.wrapping_sub(b)
}

pub(crate) fn mul(a: i64, b: i64) -> i64 {
    a.wrapping_mul(b)
}

/// `DIV`: truncates toward zero, `i64::MIN / -1 = i64::MIN` (section 0).
pub(crate) fn div(a: i64, b: i64) -> Result<i64, Fault> {
    if b == 0 {
        return Err(Fault::DivByZero);
    }
    Ok(a.wrapping_div(b))
}

/// `MOD`: the sign of `a`, `i64::MIN % -1 = 0` (section 0).
pub(crate) fn rem(a: i64, b: i64) -> Result<i64, Fault> {
    if b == 0 {
        return Err(Fault::DivByZero);
    }
    Ok(a.wrapping_rem(b))
}

pub(crate) fn neg(a: i64) -> i64 {
    a.wrapping_neg()
}

pub(crate) fn eq(a: i64, b: i64) -> i64 {
    i64::from(a == b)
}

pub(crate) fn lt(a: i64, b: i64) -> i64 {
    i64::from(a < b)
}

pub(crate) fn gt(a: i64, b: i64) -> i64 {
    i64::from(a > b)
}

pub(crate) fn not(a: i64) -> i64 {
    i64::from(a == 0)
}

pub(crate) fn and(a: i64, b: i64) -> i64 {
    a & b
}

pub(crate) fn or(a: i64, b: i64) -> i64 {
    a | b
}

pub(crate) fn xor(a: i64, b: i64) -> i64 {
    a ^ b
}

pub(crate) fn bnot(a: i64) -> i64 {
    !a
}

/// The shift count `n` if it is in `0..=63`.
fn shift_count(n: i64) -> Option<u32> {
    u32::try_from(n).ok().filter(|&n| n < 64)
}

/// `SHL`: logical, `0` for a count outside `0..=63` (B.6).
pub(crate) fn shl(a: i64, n: i64) -> i64 {
    shift_count(n).map_or(0, |n| ((a as u64) << n) as i64)
}

/// `SHR`: logical, `0` for a count outside `0..=63` (B.6).
pub(crate) fn shr(a: i64, n: i64) -> i64 {
    shift_count(n).map_or(0, |n| ((a as u64) >> n) as i64)
}

/// `SAR`: arithmetic, the sign fill for a count outside `0..=63` (B.6).
pub(crate) fn sar(a: i64, n: i64) -> i64 {
    shift_count(n).map_or(if a < 0 { -1 } else { 0 }, |n| a >> n)
}

/// `constants[k]`, or `BadConst` for an index outside the table, negative included (B.3).
pub(crate) fn constant(constants: &[i64], k: i64) -> Result<i64, Fault> {
    usize::try_from(k)
        .ok()
        .and_then(|k| constants.get(k))
        .copied()
        .ok_or(Fault::BadConst)
}

/// Checks a jump target, relative or absolute: inside the code, and on a `JUMPDEST`
/// byte (B.4). Only the byte at the target is looked at, never what precedes it (A.3).
pub(crate) fn jump_target(code: &[u8], target: i64) -> Result<usize, Fault> {
    usize::try_from(target)
        .ok()
        .filter(|&t| code.get(t) == Some(&JUMPDEST))
        .ok_or(Fault::BadJump)
}

/// The target of a relative jump: `next + d`, where `next` follows the jump (B.4).
pub(crate) fn relative(code: &[u8], next: usize, d: i16) -> Result<usize, Fault> {
    let next = i64::try_from(next).map_err(|_| Fault::BadJump)?;
    jump_target(code, next + i64::from(d))
}
