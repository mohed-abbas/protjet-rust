//! Unit tests of the engine, instruction by instruction and clause by clause.
//!
//! The official corpus checks the same machine end to end; these tests pin each rule
//! on its own, so a failure names the rule.

use crate::{
    ActRecord, Action, CALL_STACK_DEPTH, DATA_STACK_SLOTS, Fault, Host, MEMORY_CELLS, Memory,
    SenseAnswer, TurnResult, run_turn,
};
use pbc::{Instruction as I, Program, encode_all};
use std::collections::VecDeque;

/// A callback the machine made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Sense(u8),
    Act(Action, i64),
    Rand,
}

/// An answer the scripted host gives, in order.
#[derive(Debug, Clone, Copy)]
enum Answer {
    Sense(SenseAnswer),
    Act(bool),
    Rand(i64),
}

/// A host that answers from a script and logs every callback.
#[derive(Debug, Default)]
struct Script {
    answers: VecDeque<Answer>,
    calls: Vec<Call>,
}

impl Host for Script {
    type Error = &'static str;

    fn sense(&mut self, k: u8) -> Result<SenseAnswer, Self::Error> {
        self.calls.push(Call::Sense(k));
        match self.answers.pop_front() {
            Some(Answer::Sense(answer)) => Ok(answer),
            _ => Err("unexpected SENSE"),
        }
    }

    fn act(&mut self, action: Action, arg: i64) -> Result<bool, Self::Error> {
        self.calls.push(Call::Act(action, arg));
        match self.answers.pop_front() {
            Some(Answer::Act(ok)) => Ok(ok),
            _ => Err("unexpected ACT"),
        }
    }

    fn rand(&mut self) -> Result<i64, Self::Error> {
        self.calls.push(Call::Rand);
        match self.answers.pop_front() {
            Some(Answer::Rand(value)) => Ok(value),
            _ => Err("unexpected RAND"),
        }
    }
}

/// Everything a test looks at after a turn.
struct Outcome {
    result: TurnResult,
    mem: Memory,
    calls: Vec<Call>,
}

impl Outcome {
    fn fault(&self) -> Option<Fault> {
        self.result.fault
    }

    fn gas(&self) -> u16 {
        self.result.gas_used
    }

    fn trace(&self) -> &[i64] {
        &self.result.trace
    }
}

/// Runs raw code with a constant table, a budget and host answers, from zeroed memory.
/// Fails the test if the host failed or some answers were not asked for.
fn exec_raw(constants: &[i64], code: Vec<u8>, budget: u16, answers: &[Answer]) -> Outcome {
    let program = Program::from_parts(constants.to_vec(), code).unwrap();
    let mut mem = [0; MEMORY_CELLS];
    let mut host = Script {
        answers: answers.iter().copied().collect(),
        calls: Vec::new(),
    };
    let result = run_turn(&program, &mut mem, budget, &mut host).unwrap();
    assert!(
        host.answers.is_empty(),
        "unused answers: {:?}",
        host.answers
    );
    Outcome {
        result,
        mem,
        calls: host.calls,
    }
}

/// Runs instructions with the usual budget of 1000, no constants and no callbacks.
fn exec(code: &[I]) -> Outcome {
    exec_raw(&[], encode_all(code), 1000, &[])
}

/// Runs `code`, then traces the `n` values it left on the stack, top first.
fn stack_after(code: &[I], n: usize) -> Vec<i64> {
    let mut all = code.to_vec();
    all.extend(std::iter::repeat_n(I::Trace, n));
    all.push(I::Halt);
    let out = exec(&all);
    assert_eq!(out.fault(), None);
    out.result.trace
}

/// What `op` leaves on the stack for `a b`, `b` on top.
fn binary(a: i64, b: i64, op: I) -> i64 {
    let code = encode_all(&[I::Const(0), I::Const(1), op, I::Trace, I::Halt]);
    let out = exec_raw(&[a, b], code, 1000, &[]);
    assert_eq!(out.fault(), None, "{} {a} {b}", op.mnemonic());
    out.trace()[0]
}

/// The fault `op` raises for `a b`, `b` on top.
fn binary_fault(a: i64, b: i64, op: I) -> Option<Fault> {
    let code = encode_all(&[I::Const(0), I::Const(1), op, I::Halt]);
    exec_raw(&[a, b], code, 1000, &[]).fault()
}

/// What `op` leaves on the stack for `a`.
fn unary(a: i64, op: I) -> i64 {
    let code = encode_all(&[I::Const(0), op, I::Trace, I::Halt]);
    let out = exec_raw(&[a], code, 1000, &[]);
    assert_eq!(out.fault(), None, "{} {a}", op.mnemonic());
    out.trace()[0]
}

/// `n` pushes: a full stack when `n` is 64.
fn pushes(n: usize) -> Vec<I> {
    vec![I::PushI16(0); n]
}

// ---------------------------------------------------------------------------------------
// The cycle (B.2)

#[test]
fn halt_ends_the_turn_and_costs_nothing() {
    let out = exec(&[I::Halt]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.gas(), 0);
    assert_eq!(out.result.act, None);
    assert!(out.trace().is_empty());
}

#[test]
fn running_off_the_end_is_pc_out_of_range() {
    let out = exec(&[I::PushI16(1)]);
    assert_eq!(out.fault(), Some(Fault::PcOutOfRange));
    assert_eq!(out.gas(), 1);
}

#[test]
fn an_unknown_opcode_faults_without_charging() {
    let out = exec_raw(&[], vec![0x09], 1000, &[]);
    assert_eq!(out.fault(), Some(Fault::BadOpcode));
    assert_eq!(out.gas(), 0);
}

#[test]
fn a_truncated_operand_faults_without_charging() {
    let out = exec_raw(&[], vec![0x02, 0x00], 1000, &[]);
    assert_eq!(out.fault(), Some(Fault::Truncated));
    assert_eq!(out.gas(), 0);
}

#[test]
fn a_refused_instruction_is_not_charged() {
    // PUSHI16 (1) then DIV (7) with 5 gas: DIV is refused and its 7 are not charged.
    let out = exec_raw(&[], encode_all(&[I::PushI16(1), I::Div]), 5, &[]);
    assert_eq!(out.fault(), Some(Fault::OutOfGas));
    assert_eq!(out.gas(), 1);
}

#[test]
fn gas_is_charged_before_the_instruction_runs() {
    // A full stack and no gas left: OutOfGas, not StackOverflow (B.2, normative).
    let code = pushes(DATA_STACK_SLOTS + 1);
    let budget = DATA_STACK_SLOTS as u16;
    let out = exec_raw(&[], encode_all(&code), budget, &[]);
    assert_eq!(out.fault(), Some(Fault::OutOfGas));
    assert_eq!(out.gas(), budget);
}

#[test]
fn a_faulting_instruction_keeps_its_gas() {
    let out = exec(&[I::PushI16(1), I::PushI16(0), I::Div]);
    assert_eq!(out.fault(), Some(Fault::DivByZero));
    assert_eq!(out.gas(), 1 + 1 + 7);
}

#[test]
fn halt_still_runs_with_no_gas_left() {
    let out = exec_raw(&[], encode_all(&[I::PushI16(1), I::Halt]), 1, &[]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.gas(), 1);
}

#[test]
fn an_endless_loop_stops_on_gas() {
    // 0: JUMPDEST (1 gas), 1: JMP back to 0 (2 gas).
    let out = exec(&[I::JumpDest, I::Jmp(-4)]);
    assert_eq!(out.fault(), Some(Fault::OutOfGas));
    // 333 laps use 999, the next JUMPDEST uses the last unit, then JMP is refused.
    assert_eq!(out.gas(), 1000);
}

// ---------------------------------------------------------------------------------------
// Constants and the stack (B.3)

#[test]
fn const_reads_the_table() {
    let code = encode_all(&[I::Const(1), I::Const(0), I::Trace, I::Trace, I::Halt]);
    let out = exec_raw(&[i64::MIN, 42], code, 1000, &[]);
    assert_eq!(out.trace(), &[i64::MIN, 42]);
}

#[test]
fn const_outside_the_table_is_bad_const() {
    let out = exec(&[I::Const(0)]);
    assert_eq!(out.fault(), Some(Fault::BadConst));
    assert_eq!(out.gas(), 1);
}

#[test]
fn constx_pops_its_index() {
    let code = encode_all(&[I::PushI16(2), I::ConstX, I::Trace, I::Halt]);
    let out = exec_raw(&[10, 20, 30], code, 1000, &[]);
    assert_eq!(out.trace(), &[30]);
    assert_eq!(out.gas(), 1 + 2 + 1);
}

#[test]
fn constx_refuses_negative_and_too_large_indices() {
    for k in [-1, 3, i16::MAX] {
        let code = encode_all(&[I::PushI16(k), I::ConstX]);
        let out = exec_raw(&[10, 20, 30], code, 1000, &[]);
        assert_eq!(out.fault(), Some(Fault::BadConst), "k = {k}");
    }
}

#[test]
fn pushi16_sign_extends() {
    assert_eq!(
        stack_after(&[I::PushI16(-1), I::PushI16(i16::MIN)], 2),
        [-32768, -1]
    );
}

#[test]
fn stack_shuffles() {
    assert_eq!(
        stack_after(&[I::PushI16(1), I::PushI16(2), I::Drop], 1),
        [1]
    );
    assert_eq!(stack_after(&[I::PushI16(1), I::Dup], 2), [1, 1]);
    // a b -- b a, with a = 1 and b = 2: 1 ends on top.
    assert_eq!(
        stack_after(&[I::PushI16(1), I::PushI16(2), I::Swap], 2),
        [1, 2]
    );
    // a b -- a b a.
    assert_eq!(
        stack_after(&[I::PushI16(1), I::PushI16(2), I::Over], 3),
        [1, 2, 1]
    );
    // a b c -- b c a: 1 2 3 becomes 2 3 1, so 1 ends on top.
    let rot = [I::PushI16(1), I::PushI16(2), I::PushI16(3), I::Rot];
    assert_eq!(stack_after(&rot, 3), [1, 3, 2]);
}

#[test]
fn every_popping_instruction_underflows_on_an_empty_stack() {
    let popping = [
        I::Drop,
        I::Dup,
        I::Swap,
        I::Over,
        I::Rot,
        I::ConstX,
        I::Add,
        I::Div,
        I::Neg,
        I::Not,
        I::Sar,
        I::LoadX,
        I::StoreX,
        I::Store(0),
        I::Jz(0),
        I::Jnz(0),
        I::JmpX,
        I::CallX,
        I::Act(0),
        I::Trace,
    ];
    for instruction in popping {
        let out = exec(&[instruction]);
        assert_eq!(out.fault(), Some(Fault::StackUnderflow), "{instruction:?}");
        assert!(out.calls.is_empty(), "{instruction:?}");
    }
}

#[test]
fn two_operand_instructions_underflow_on_one_value() {
    for instruction in [I::Swap, I::Over, I::Sub, I::StoreX, I::Shl] {
        let out = exec(&[I::PushI16(1), instruction]);
        assert_eq!(out.fault(), Some(Fault::StackUnderflow), "{instruction:?}");
    }
    let out = exec(&[I::PushI16(1), I::PushI16(2), I::Rot]);
    assert_eq!(out.fault(), Some(Fault::StackUnderflow));
}

#[test]
fn the_stack_holds_64_values() {
    let mut code = pushes(DATA_STACK_SLOTS);
    code.push(I::Halt);
    assert_eq!(exec(&code).fault(), None);

    let out = exec(&pushes(DATA_STACK_SLOTS + 1));
    assert_eq!(out.fault(), Some(Fault::StackOverflow));
    assert_eq!(out.gas(), DATA_STACK_SLOTS as u16 + 1);
}

#[test]
fn every_pushing_instruction_overflows_on_a_full_stack() {
    let pushing = [
        I::Const(0),
        I::PushI16(0),
        I::Dup,
        I::Over,
        I::Load(0),
        I::Sense(0),
        I::Rand,
        I::Gas,
    ];
    for instruction in pushing {
        let mut code = pushes(DATA_STACK_SLOTS);
        code.push(instruction);
        let out = exec_raw(&[5], encode_all(&code), 1000, &[]);
        assert_eq!(out.fault(), Some(Fault::StackOverflow), "{instruction:?}");
        // No callback for an instruction that could not push the answer (journal D-017).
        assert!(out.calls.is_empty(), "{instruction:?}");
    }
}

#[test]
fn const_checks_the_room_before_the_index() {
    // A full stack and a bad index: the stack check comes first (journal D-017).
    let mut code = pushes(DATA_STACK_SLOTS);
    code.push(I::Const(9));
    assert_eq!(exec(&code).fault(), Some(Fault::StackOverflow));
}

// ---------------------------------------------------------------------------------------
// Arithmetic and comparisons (B.3)

#[test]
fn arithmetic_wraps() {
    assert_eq!(binary(i64::MAX, 1, I::Add), i64::MIN);
    assert_eq!(binary(i64::MIN, 1, I::Sub), i64::MAX);
    assert_eq!(binary(i64::MAX, 2, I::Mul), -2);
    assert_eq!(unary(i64::MIN, I::Neg), i64::MIN);
    assert_eq!(binary(7, 10, I::Sub), -3);
}

#[test]
fn division_truncates_toward_zero() {
    assert_eq!(binary(-7, 2, I::Div), -3);
    assert_eq!(binary(7, -2, I::Div), -3);
    assert_eq!(binary(-7, 2, I::Mod), -1);
    assert_eq!(binary(7, -2, I::Mod), 1);
    assert_eq!(binary(i64::MIN, -1, I::Div), i64::MIN);
    assert_eq!(binary(i64::MIN, -1, I::Mod), 0);
}

#[test]
fn division_by_zero_faults() {
    assert_eq!(binary_fault(5, 0, I::Div), Some(Fault::DivByZero));
    assert_eq!(binary_fault(5, 0, I::Mod), Some(Fault::DivByZero));
    assert_eq!(binary_fault(0, 0, I::Mod), Some(Fault::DivByZero));
}

#[test]
fn comparisons_are_signed() {
    assert_eq!(binary(3, 3, I::Eq), 1);
    assert_eq!(binary(3, 4, I::Eq), 0);
    assert_eq!(binary(-1, 0, I::Lt), 1);
    assert_eq!(binary(0, -1, I::Lt), 0);
    assert_eq!(binary(0, -1, I::Gt), 1);
    assert_eq!(binary(i64::MIN, i64::MAX, I::Gt), 0);
}

#[test]
fn not_is_logical_and_bnot_is_bitwise() {
    assert_eq!(unary(0, I::Not), 1);
    assert_eq!(unary(5, I::Not), 0);
    assert_eq!(unary(0, I::BNot), -1);
    assert_eq!(unary(5, I::BNot), -6);
}

#[test]
fn bitwise_operations() {
    assert_eq!(binary(0b1100, 0b1010, I::And), 0b1000);
    assert_eq!(binary(0b1100, 0b1010, I::Or), 0b1110);
    assert_eq!(binary(0b1100, 0b1010, I::Xor), 0b0110);
    assert_eq!(binary(-1, i64::MIN, I::Xor), i64::MAX);
}

// ---------------------------------------------------------------------------------------
// Shifts (B.6)

#[test]
fn shl_and_shr_are_logical() {
    assert_eq!(binary(1, 63, I::Shl), i64::MIN);
    assert_eq!(binary(-1, 1, I::Shr), i64::MAX);
    assert_eq!(binary(i64::MIN, 63, I::Shr), 1);
    assert_eq!(binary(5, 0, I::Shl), 5);
    assert_eq!(binary(5, 0, I::Shr), 5);
}

#[test]
fn shl_and_shr_give_zero_outside_the_width() {
    for n in [64, 65, 1000, -1, i64::MIN] {
        assert_eq!(binary(-1, n, I::Shl), 0, "n = {n}");
        assert_eq!(binary(-1, n, I::Shr), 0, "n = {n}");
    }
}

#[test]
fn sar_propagates_the_sign() {
    assert_eq!(binary(-1, 1, I::Sar), -1);
    assert_eq!(binary(-7, 1, I::Sar), -4);
    assert_eq!(binary(7, 1, I::Sar), 3);
    assert_eq!(binary(-1, 63, I::Sar), -1);
    assert_eq!(binary(i64::MIN, 63, I::Sar), -1);
}

#[test]
fn sar_gives_the_sign_fill_outside_the_width() {
    for n in [64, 1000, -1, i64::MIN] {
        assert_eq!(binary(-5, n, I::Sar), -1, "n = {n}");
        assert_eq!(binary(5, n, I::Sar), 0, "n = {n}");
        assert_eq!(binary(0, n, I::Sar), 0, "n = {n}");
    }
}

#[test]
fn sar_is_not_div() {
    assert_eq!(binary(-7, 2, I::Div), -3);
    assert_eq!(binary(-7, 1, I::Sar), -4);
}

// ---------------------------------------------------------------------------------------
// Memory (B.1, B.3)

#[test]
fn storex_and_loadx() {
    // v addr --: store 42 at 255, then read it back.
    let out = exec(&[
        I::PushI16(42),
        I::PushI16(255),
        I::StoreX,
        I::PushI16(255),
        I::LoadX,
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(out.trace(), &[42]);
    assert_eq!(out.mem[255], 42);
    assert_eq!(out.gas(), 1 + 1 + 5 + 1 + 4 + 1);
}

#[test]
fn store_and_load_with_a_constant_address() {
    let out = exec(&[I::PushI16(-9), I::Store(3), I::Load(3), I::Trace, I::Halt]);
    assert_eq!(out.trace(), &[-9]);
    assert_eq!(out.mem[3], -9);
    assert_eq!(out.gas(), 1 + 4 + 3 + 1);
}

#[test]
fn memory_addresses_outside_0_to_255_are_out_of_bounds() {
    for addr in [256, -1, i16::MIN] {
        let load = exec(&[I::PushI16(addr), I::LoadX]);
        assert_eq!(load.fault(), Some(Fault::OutOfBounds), "LOADX {addr}");
        let store = exec(&[I::PushI16(1), I::PushI16(addr), I::StoreX]);
        assert_eq!(store.fault(), Some(Fault::OutOfBounds), "STOREX {addr}");
    }
}

#[test]
fn memory_persists_between_turns_and_keeps_writes_before_a_fault() {
    // mem[0] += 1, trace it, then fault on purpose.
    let code = encode_all(&[
        I::Load(0),
        I::PushI16(1),
        I::Add,
        I::Dup,
        I::Store(0),
        I::Trace,
        I::PushI16(0),
        I::PushI16(0),
        I::Div,
    ]);
    let program = Program::from_parts(vec![], code).unwrap();
    let mut mem = [0; MEMORY_CELLS];
    let mut host = Script::default();
    for turn in 1..=3 {
        let result = run_turn(&program, &mut mem, 1000, &mut host).unwrap();
        assert_eq!(result.fault, Some(Fault::DivByZero));
        assert_eq!(result.trace, [turn]);
        assert_eq!(mem[0], turn);
    }
}

// ---------------------------------------------------------------------------------------
// Jumps (B.4)

#[test]
fn a_relative_jump_counts_from_the_next_instruction() {
    let out = exec(&[
        I::Jmp(2), // 0..3, target 3 + 2 = 5
        I::Dup,    // 3, skipped
        I::Dup,    // 4, skipped
        I::JumpDest,
        I::PushI16(7),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.trace(), &[7]);
    assert_eq!(out.gas(), 2 + 1 + 1 + 1);
}

#[test]
fn a_jump_target_must_hold_jumpdest() {
    // JMP +0 lands on the HALT right after it.
    let out = exec(&[I::Jmp(0), I::Halt]);
    assert_eq!(out.fault(), Some(Fault::BadJump));
    assert_eq!(out.gas(), 2);
}

#[test]
fn a_jump_outside_the_code_is_bad_jump() {
    assert_eq!(exec(&[I::Jmp(1)]).fault(), Some(Fault::BadJump));
    assert_eq!(exec(&[I::Jmp(-4)]).fault(), Some(Fault::BadJump));
    assert_eq!(exec(&[I::Jmp(i16::MIN)]).fault(), Some(Fault::BadJump));
}

#[test]
fn a_jump_may_land_inside_an_operand() {
    let out = exec(&[
        I::Jmp(2),        // 0..3, target 5
        I::PushI16(0x43), // 3..6: bytes 02 00 43, so byte 5 is a JUMPDEST on its own
        I::PushI16(9),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.trace(), &[9]);
}

#[test]
fn conditional_jumps() {
    let branch = |cond: i16, jump: I| {
        exec(&[
            I::PushI16(cond), // 0..3
            jump,             // 3..6, target 6 + 2 = 8
            I::Dup,           // 6: the stack is empty, so this underflows if reached
            I::Halt,          // 7
            I::JumpDest,      // 8
            I::Halt,
        ])
        .fault()
    };
    assert_eq!(branch(0, I::Jz(2)), None);
    assert_eq!(branch(5, I::Jz(2)), Some(Fault::StackUnderflow));
    assert_eq!(branch(-1, I::Jnz(2)), None);
    assert_eq!(branch(0, I::Jnz(2)), Some(Fault::StackUnderflow));

    // The target is only checked when the jump is taken.
    assert_eq!(exec(&[I::PushI16(1), I::Jz(100), I::Halt]).fault(), None);
    assert_eq!(
        exec(&[I::PushI16(0), I::Jz(100)]).fault(),
        Some(Fault::BadJump)
    );
}

#[test]
fn jmpx_jumps_to_an_absolute_target() {
    let out = exec(&[
        I::PushI16(6), // 0..3
        I::JmpX,       // 3
        I::Dup,        // 4, skipped
        I::Dup,        // 5, skipped
        I::JumpDest,   // 6
        I::PushI16(3),
        I::Trace,
        I::Halt,
    ]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.trace(), &[3]);
}

#[test]
fn jmpx_refuses_bad_targets() {
    // The code is 5 bytes: PUSHI16 (0..3), JMPX (3), HALT (4).
    for target in [-1, 4, 5, i16::MAX] {
        let out = exec(&[I::PushI16(target), I::JmpX, I::Halt]);
        assert_eq!(out.fault(), Some(Fault::BadJump), "target {target}");
    }
}

// ---------------------------------------------------------------------------------------
// Calls (B.3, B.4)

#[test]
fn call_and_return() {
    let out = exec(&[
        I::Call(5),    // 0..3, target 8, returns to 3
        I::PushI16(2), // 3..6
        I::Trace,      // 6
        I::Halt,       // 7
        I::JumpDest,   // 8
        I::PushI16(1), // 9..12
        I::Trace,      // 12
        I::Return,     // 13: back to 3, which needs no JUMPDEST
    ]);
    assert_eq!(out.fault(), None);
    assert_eq!(out.trace(), &[1, 2]);
    assert_eq!(out.gas(), 6 + 1 + 1 + 1 + 4 + 1 + 1);
}

#[test]
fn return_without_a_frame_underflows() {
    let out = exec(&[I::Return]);
    assert_eq!(out.fault(), Some(Fault::CallStackUnderflow));
    assert_eq!(out.gas(), 4);
}

#[test]
fn the_call_stack_holds_16_frames() {
    // 0: JUMPDEST, 1: CALL back to 0, forever. The 17th CALL overflows.
    let out = exec(&[I::JumpDest, I::Call(-4)]);
    assert_eq!(out.fault(), Some(Fault::CallStackOverflow));
    assert_eq!(out.gas(), (CALL_STACK_DEPTH as u16 + 1) * (1 + 6));
}

#[test]
fn call_to_a_bad_target_is_bad_jump() {
    assert_eq!(exec(&[I::Call(0), I::Halt]).fault(), Some(Fault::BadJump));
}

#[test]
fn callx_returns_like_call() {
    let out = exec(&[
        I::PushI16(8), // 0..3
        I::CallX,      // 3, returns to 4
        I::Gas,        // 4
        I::Trace,      // 5
        I::Halt,       // 6
        I::Halt,       // 7
        I::JumpDest,   // 8
        I::PushI16(1),
        I::Trace,
        I::Return,
    ]);
    assert_eq!(out.fault(), None);
    let used_up_to_gas = 1 + 7 + 1 + 1 + 1 + 4 + 2;
    assert_eq!(out.trace(), &[1, 1000 - used_up_to_gas]);
}

#[test]
fn callx_pops_before_pushing_its_frame() {
    let out = exec(&[I::CallX]);
    assert_eq!(out.fault(), Some(Fault::StackUnderflow));
    assert_eq!(out.gas(), 7);
}

// ---------------------------------------------------------------------------------------
// GAS and TRACE (B.3, B.7)

#[test]
fn gas_reports_what_is_left_after_its_own_charge() {
    let out = exec(&[I::Gas, I::Trace, I::Gas, I::Trace, I::Halt]);
    assert_eq!(out.trace(), &[998, 995]);
}

#[test]
fn the_trace_is_reserved_for_the_whole_budget() {
    // 0: JUMPDEST, 1: GAS, 2: TRACE, 3: JMP back to 0, until the gas runs out.
    let out = exec(&[I::JumpDest, I::Gas, I::Trace, I::Jmp(-6)]);
    assert_eq!(out.fault(), Some(Fault::OutOfGas));
    assert_eq!(out.trace().len(), 167);
    assert!(out.result.trace.capacity() >= 1000);
}

// ---------------------------------------------------------------------------------------
// SENSE, ACT and RAND (annex E, K.6)

#[test]
fn sense_pushes_the_host_value() {
    let code = encode_all(&[I::Sense(6), I::Trace, I::Halt]);
    let out = exec_raw(&[], code, 1000, &[Answer::Sense(SenseAnswer::Value(-4))]);
    assert_eq!(out.calls, [Call::Sense(6)]);
    assert_eq!(out.trace(), &[-4]);
    assert_eq!(out.gas(), 8 + 1);
}

#[test]
fn sense_raises_the_fault_the_host_answers() {
    let code = encode_all(&[I::Sense(200), I::Halt]);
    let answer = Answer::Sense(SenseAnswer::Fault(Fault::BadSensor));
    let out = exec_raw(&[], code, 1000, &[answer]);
    assert_eq!(out.calls, [Call::Sense(200)]);
    assert_eq!(out.fault(), Some(Fault::BadSensor));
    assert_eq!(out.gas(), 8);
}

#[test]
fn sense_without_gas_asks_nothing() {
    let out = exec_raw(&[], encode_all(&[I::Sense(0)]), 7, &[]);
    assert_eq!(out.fault(), Some(Fault::OutOfGas));
    assert!(out.calls.is_empty());
}

#[test]
fn act_pops_its_argument_and_pushes_the_outcome() {
    let code = encode_all(&[I::PushI16(2), I::Act(0), I::Trace, I::Halt]);
    let out = exec_raw(&[], code, 1000, &[Answer::Act(false)]);
    assert_eq!(out.calls, [Call::Act(Action::Move, 2)]);
    assert_eq!(out.trace(), &[0]);
    let act = ActRecord {
        action: Action::Move,
        arg: 2,
        ok: false,
    };
    assert_eq!(out.result.act, Some(act));
    assert_eq!(out.gas(), 1 + 12 + 1);
}

#[test]
fn a_second_act_is_already_acted() {
    // The second ACT pops the first one's outcome, then refuses without a callback.
    let code = encode_all(&[I::PushI16(0), I::Act(1), I::Act(3)]);
    let out = exec_raw(&[], code, 1000, &[Answer::Act(true)]);
    assert_eq!(out.calls, [Call::Act(Action::Take, 0)]);
    assert_eq!(out.fault(), Some(Fault::AlreadyActed));
    let act = ActRecord {
        action: Action::Take,
        arg: 0,
        ok: true,
    };
    assert_eq!(out.result.act, Some(act));
    assert_eq!(out.gas(), 1 + 12 + 12);
}

#[test]
fn an_unknown_action_is_bad_action_and_not_recorded() {
    let out = exec(&[I::PushI16(0), I::Act(4)]);
    assert_eq!(out.fault(), Some(Fault::BadAction));
    assert!(out.calls.is_empty());
    assert_eq!(out.result.act, None);
}

#[test]
fn every_action_reaches_the_host() {
    for action in Action::ALL {
        let code = encode_all(&[I::PushI16(-3), I::Act(action.code()), I::Halt]);
        let out = exec_raw(&[], code, 1000, &[Answer::Act(true)]);
        assert_eq!(out.calls, [Call::Act(action, -3)]);
    }
}

#[test]
fn act_on_a_full_stack_cannot_overflow() {
    let mut code = pushes(DATA_STACK_SLOTS);
    code.extend([I::Act(1), I::Halt]);
    let out = exec_raw(&[], encode_all(&code), 1000, &[Answer::Act(true)]);
    assert_eq!(out.fault(), None);
}

#[test]
fn rand_pushes_the_host_value() {
    let code = encode_all(&[I::Rand, I::Rand, I::Trace, I::Trace, I::Halt]);
    let out = exec_raw(&[], code, 1000, &[Answer::Rand(7), Answer::Rand(i64::MIN)]);
    assert_eq!(out.calls, [Call::Rand, Call::Rand]);
    assert_eq!(out.trace(), &[i64::MIN, 7]);
}

#[test]
fn a_host_failure_stops_the_turn_without_a_result() {
    let program = Program::from_parts(vec![], encode_all(&[I::Rand, I::Halt])).unwrap();
    let mut mem = [0; MEMORY_CELLS];
    let mut host = Script::default();
    let result = run_turn(&program, &mut mem, 1000, &mut host);
    assert_eq!(result, Err("unexpected RAND"));
}
