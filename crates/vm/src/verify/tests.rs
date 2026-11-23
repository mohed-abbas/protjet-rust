//! Unit tests of the verifier: what it proves, and each reason it refuses.

use super::{EXCLUDED_FAULTS, MAX_SLOTS, Reason, Rejection, Verified, verify};
use crate::{CALL_STACK_DEPTH, DATA_STACK_SLOTS, Fault};
use pbc::{Instruction as I, Program, encode_all};

/// An opcode that does not exist (B.3 stops well before 0xFF).
const GARBAGE: u8 = 0xFF;

fn check_bytes(constants: &[i64], code: Vec<u8>) -> Result<Verified, Rejection> {
    verify(&Program::from_parts(constants.to_vec(), code).unwrap())
}

fn check(code: &[I]) -> Result<Verified, Rejection> {
    check_bytes(&[], encode_all(code))
}

/// `code` followed by raw bytes, for unreachable or reachable garbage.
fn with_bytes(code: &[I], tail: &[u8]) -> Vec<u8> {
    let mut bytes = encode_all(code);
    bytes.extend_from_slice(tail);
    bytes
}

fn rejected(pc: usize, reason: Reason) -> Result<Verified, Rejection> {
    Err(Rejection { pc, reason })
}

fn may_fault(pc: usize, fault: Fault) -> Result<Verified, Rejection> {
    rejected(pc, Reason::MayFault(fault))
}

#[test]
fn garbage_is_not_an_opcode() {
    assert_eq!(
        pbc::decode_at(&[GARBAGE], 0),
        Err(pbc::DecodeError::BadOpcode)
    );
}

// --- What an accepted program gets ---

#[test]
fn accepts_straight_line_code() {
    assert_eq!(
        check(&[I::PushI16(1), I::Trace, I::Halt]),
        Ok(Verified {
            max_stack: 1,
            max_call_depth: 0,
            states: 3,
        })
    );
}

#[test]
fn bounds_the_stack_exactly() {
    let mut code = vec![I::PushI16(7); DATA_STACK_SLOTS];
    code.push(I::Halt);
    assert_eq!(check(&code).unwrap().max_stack, DATA_STACK_SLOTS);

    // One more push overflows, at the 65th push.
    code.insert(0, I::PushI16(7));
    assert_eq!(
        check(&code),
        may_fault(3 * DATA_STACK_SLOTS, Fault::StackOverflow)
    );
}

#[test]
fn accepts_a_loop_whose_stack_stays_level() {
    // Corpus case 208: sum 5 + 4 + 3 + 2 + 1 with a counter.
    let code = [
        I::PushI16(0),
        I::PushI16(5),
        I::JumpDest, // 6
        I::Swap,
        I::Over,
        I::Add,
        I::Swap,
        I::PushI16(-1),
        I::Add,
        I::Dup,
        I::Jnz(-13), // 16, next 19, back to 6
        I::Drop,
        I::Trace,
        I::Halt,
    ];
    let verified = check(&code).unwrap();
    assert_eq!(verified.max_stack, 3);
    assert_eq!(verified.max_call_depth, 0);
}

// --- Falling off the code, decoding ---

#[test]
fn rejects_falling_off_the_end() {
    assert_eq!(check(&[I::PushI16(1)]), may_fault(0, Fault::PcOutOfRange));
}

#[test]
fn rejects_reachable_garbage() {
    let code = with_bytes(&[I::JumpDest], &[GARBAGE]);
    assert_eq!(check_bytes(&[], code), may_fault(1, Fault::BadOpcode));

    let cut = with_bytes(&[I::JumpDest], &[I::PushI16(0).opcode(), 0x00]);
    assert_eq!(check_bytes(&[], cut), may_fault(1, Fault::Truncated));
}

#[test]
fn ignores_unreachable_garbage() {
    // A linear sweep would refuse this; only reachable code matters.
    let code = with_bytes(&[I::Halt], &[GARBAGE, GARBAGE]);
    assert_eq!(check_bytes(&[], code).unwrap().states, 1);
}

// --- Stack ---

#[test]
fn rejects_underflow_where_it_happens() {
    assert_eq!(
        check(&[I::PushI16(1), I::Add, I::Halt]),
        may_fault(3, Fault::StackUnderflow)
    );
}

#[test]
fn rejects_a_loop_whose_stack_grows() {
    // 0: JUMPDEST, 1: PUSHI16 1, 4: JMP back to 0. The 65th push overflows.
    assert_eq!(
        check(&[I::JumpDest, I::PushI16(1), I::Jmp(-7)]),
        may_fault(1, Fault::StackOverflow)
    );
}

#[test]
fn rejects_a_callback_on_a_full_stack() {
    let mut code = vec![I::PushI16(0); DATA_STACK_SLOTS];
    code.extend([I::Sense(0), I::Halt]);
    assert_eq!(
        check(&code),
        may_fault(3 * DATA_STACK_SLOTS, Fault::StackOverflow)
    );
}

// --- Jumps ---

#[test]
fn rejects_a_jump_off_a_jumpdest() {
    // The target, 3, holds HALT.
    assert_eq!(check(&[I::Jmp(0), I::Halt]), may_fault(0, Fault::BadJump));
}

#[test]
fn accepts_a_jump_into_an_operand() {
    // A.3: 0: JMP to 5, 3: PUSHI16 0x0043 (bytes 3, 4, 5), 6: HALT. Byte 5 is 0x43.
    let verified = check(&[I::Jmp(2), I::PushI16(0x43), I::Halt]).unwrap();
    // pc 0, then the JUMPDEST at 5, then HALT at 6: the PUSHI16 is never decoded.
    assert_eq!(verified.states, 3);
}

#[test]
fn follows_only_the_side_a_known_condition_takes() {
    // Never taken: the bad target does not matter, as in the engine (B.4).
    assert!(check(&[I::PushI16(1), I::Jz(100), I::Halt]).is_ok());
    assert!(check(&[I::PushI16(0), I::Jnz(100), I::Halt]).is_ok());

    // Always taken: the garbage at 6 is never reached.
    let taken = with_bytes(&[I::PushI16(0), I::Jz(1)], &[GARBAGE]);
    let taken = [taken, encode_all(&[I::JumpDest, I::Halt])].concat();
    assert!(check_bytes(&[], taken).is_ok());
}

#[test]
fn follows_both_sides_of_an_unknown_condition() {
    assert_eq!(
        check(&[I::Rand, I::Jz(100), I::Halt]),
        may_fault(1, Fault::BadJump)
    );
    // 0: RAND, 1: JZ to 5, 4: garbage, 5: JUMPDEST. Falling through hits the garbage.
    let code = [
        with_bytes(&[I::Rand, I::Jz(1)], &[GARBAGE]),
        encode_all(&[I::JumpDest, I::Halt]),
    ]
    .concat();
    assert_eq!(check_bytes(&[], code), may_fault(4, Fault::BadOpcode));
}

#[test]
fn checks_a_known_absolute_target() {
    // 0: PUSHI16 4, 3: JMPX, 4: JUMPDEST, 5: HALT.
    assert!(check(&[I::PushI16(4), I::JmpX, I::JumpDest, I::Halt]).is_ok());
    assert_eq!(
        check(&[I::PushI16(5), I::JmpX, I::JumpDest, I::Halt]),
        may_fault(3, Fault::BadJump)
    );
}

#[test]
fn rejects_an_unknown_absolute_target() {
    assert_eq!(
        check(&[I::Rand, I::JmpX, I::JumpDest, I::Halt]),
        rejected(1, Reason::UnknownTarget)
    );
    // Memory persists between turns, so a target read from it is unknown too.
    assert_eq!(
        check(&[I::Load(0), I::JmpX, I::JumpDest, I::Halt]),
        rejected(2, Reason::UnknownTarget)
    );
}

// --- Calls ---

/// Corpus case 210: `CALLX` through a table of addresses read by `CONSTX`.
fn dispatch(constants: &[i64]) -> Result<Verified, Rejection> {
    let code = [
        I::Sense(5),
        I::PushI16(-1),
        I::Add,
        I::PushI16(0),
        I::Add,
        I::ConstX,
        I::CallX, // 11
        I::Trace,
        I::Halt,
        I::JumpDest, // 14
        I::PushI16(11),
        I::Return,
        I::JumpDest, // 19
        I::PushI16(22),
        I::Return,
        I::JumpDest, // 24
        I::PushI16(33),
        I::Return,
    ];
    check_bytes(constants, encode_all(&code))
}

#[test]
fn follows_a_dispatch_table() {
    let verified = dispatch(&[14, 19, 24]).unwrap();
    assert_eq!(verified.max_call_depth, 1);
    // One wrong entry is enough: the sensed index could pick it.
    assert_eq!(dispatch(&[14, 15, 24]), may_fault(11, Fault::BadJump));
}

#[test]
fn accepts_a_subroutine_called_twice() {
    // Corpus case 209: square 3, then 12.
    let code = [
        I::PushI16(3),
        I::Call(9), // 3, next 6, to 15
        I::Trace,
        I::PushI16(12),
        I::Call(2), // 10, next 13, to 15
        I::Trace,
        I::Halt,
        I::JumpDest, // 15
        I::Dup,
        I::Mul,
        I::Return,
    ];
    let verified = check(&code).unwrap();
    assert_eq!(verified.max_stack, 2);
    assert_eq!(verified.max_call_depth, 1);
}

#[test]
fn rejects_a_return_without_a_call() {
    assert_eq!(check(&[I::Return]), may_fault(0, Fault::CallStackUnderflow));
}

#[test]
fn rejects_unbounded_recursion() {
    // 0: JUMPDEST, 1: CALL 0. The 17th call overflows.
    assert_eq!(
        check(&[I::JumpDest, I::Call(-4)]),
        may_fault(1, Fault::CallStackOverflow)
    );
}

#[test]
fn accepts_calls_up_to_the_full_depth() {
    // 16 nested calls, each to the next JUMPDEST: depth 16 is allowed, 17 is not.
    let mut code = Vec::new();
    for _ in 0..CALL_STACK_DEPTH {
        code.extend([I::Call(0), I::JumpDest]);
    }
    code.push(I::Halt);
    assert_eq!(check(&code).unwrap().max_call_depth, CALL_STACK_DEPTH);

    code.insert(0, I::JumpDest);
    code.insert(0, I::Call(0));
    let pc = 4 * CALL_STACK_DEPTH;
    assert_eq!(check(&code), may_fault(pc, Fault::CallStackOverflow));
}

#[test]
fn rejects_a_return_past_the_end() {
    // 0: JMP to 5, 3: JUMPDEST, 4: RETURN, 5: JUMPDEST, 6: CALL 3 (last instruction).
    let code = [I::Jmp(2), I::JumpDest, I::Return, I::JumpDest, I::Call(-6)];
    assert_eq!(check(&code), may_fault(4, Fault::PcOutOfRange));
}

// --- Paths that always fault end there ---

#[test]
fn a_certain_fault_ends_the_path() {
    let divides_by_zero = with_bytes(&[I::PushI16(1), I::PushI16(0), I::Div], &[GARBAGE]);
    assert!(check_bytes(&[], divides_by_zero).is_ok());

    let bad_action = with_bytes(&[I::PushI16(0), I::Act(7)], &[GARBAGE]);
    assert!(check_bytes(&[], bad_action).is_ok());

    let bad_constant = with_bytes(&[I::Const(0)], &[GARBAGE]);
    assert!(check_bytes(&[], bad_constant).is_ok());

    // An unknown divisor may be non-zero: the garbage is reachable.
    let unknown = with_bytes(&[I::PushI16(1), I::Rand, I::Div], &[GARBAGE]);
    assert_eq!(check_bytes(&[], unknown), may_fault(5, Fault::BadOpcode));
}

// --- Limits ---

#[test]
fn refuses_a_program_beyond_its_limits() {
    // About 8000 states at height 63 or 64: more than MAX_SLOTS slots in all.
    let mut code = vec![I::PushI16(0); DATA_STACK_SLOTS - 1];
    for _ in 0..4000 {
        code.extend([I::Dup, I::Drop]);
    }
    code.push(I::Halt);
    const { assert!(8000 * (DATA_STACK_SLOTS - 1) > MAX_SLOTS) };
    let rejection = check(&code).unwrap_err();
    assert_eq!(rejection.reason, Reason::TooComplex);
}

#[test]
fn only_excludes_control_and_stack_faults() {
    for fault in EXCLUDED_FAULTS {
        assert!(!matches!(
            fault,
            Fault::OutOfGas
                | Fault::DivByZero
                | Fault::BadConst
                | Fault::OutOfBounds
                | Fault::BadSensor
                | Fault::BadAction
                | Fault::AlreadyActed
        ));
    }
    assert_eq!(EXCLUDED_FAULTS.len() + 7, Fault::ALL.len());
}
