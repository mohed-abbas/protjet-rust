//! Shared tools for the property tests: a seeded random generator, a random host, and
//! random programs.
//!
//! The README §3 lists the crates we may use, and no property-testing crate is on it, so
//! the generator is written here. It is seeded: a failing case prints its seed and
//! replays exactly.

// Each test file uses a different part of this module. And this is test code (README
// §3), but clippy.toml only exempts `#[test]` functions, not their helpers.
#![allow(dead_code, clippy::expect_used, clippy::indexing_slicing)]

use pbc::{Instruction as I, JUMPDEST, Program, encode_all};
use vm::{Action, Fault, Host, MEMORY_CELLS, Memory, SenseAnswer};

/// xorshift64*: small, fast, and good enough to spread test cases.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // xorshift has a fixed point at 0, so mix the seed first.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`, for `n > 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// True with probability `1 / n`.
    pub fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    /// Mostly small values, sometimes the extremes, sometimes anything.
    pub fn value(&mut self) -> i64 {
        match self.below(8) {
            0 => i64::MIN,
            1 => i64::MAX,
            2 => self.next() as i64,
            _ => self.below(21) as i64 - 10,
        }
    }

    pub fn memory(&mut self) -> Memory {
        let mut mem = [0; MEMORY_CELLS];
        for cell in &mut mem {
            if self.one_in(4) {
                *cell = self.value();
            }
        }
        mem
    }
}

/// A host whose answers come from its own seeded generator, so two hosts with the same
/// seed answer the same calls identically.
#[derive(Debug)]
pub struct RandomHost {
    rng: Rng,
    /// If true, a callback sometimes fails, as when the client goes away.
    pub can_fail: bool,
    /// How many times `act` was called.
    pub acts: usize,
}

impl RandomHost {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            can_fail: false,
            acts: 0,
        }
    }

    fn fail(&mut self) -> Result<(), &'static str> {
        if self.can_fail && self.rng.one_in(20) {
            Err("client gone")
        } else {
            Ok(())
        }
    }
}

impl Host for RandomHost {
    type Error = &'static str;

    fn sense(&mut self, _k: u8) -> Result<SenseAnswer, Self::Error> {
        self.fail()?;
        if self.rng.one_in(10) {
            Ok(SenseAnswer::Fault(Fault::BadSensor))
        } else {
            Ok(SenseAnswer::Value(self.rng.value()))
        }
    }

    fn act(&mut self, _action: Action, _arg: i64) -> Result<bool, Self::Error> {
        self.fail()?;
        self.acts += 1;
        Ok(self.rng.one_in(2))
    }

    fn rand(&mut self) -> Result<i64, Self::Error> {
        self.fail()?;
        Ok(self.rng.value())
    }
}

/// A random instruction. Operands are biased toward values that do something: small
/// constant indices, memory cells, sensors and actions, short jumps.
pub fn instruction(rng: &mut Rng) -> I {
    let small = |rng: &mut Rng| rng.below(6) as u8;
    let offset = |rng: &mut Rng| rng.below(41) as i16 - 20;
    match rng.below(43) {
        0 => I::Halt,
        1 => I::Const(rng.below(5) as u16),
        2 => I::PushI16(rng.value() as i16),
        3 => I::Drop,
        4 => I::Dup,
        5 => I::Swap,
        6 => I::Over,
        7 => I::Rot,
        8 => I::ConstX,
        9 => I::Add,
        10 => I::Sub,
        11 => I::Mul,
        12 => I::Div,
        13 => I::Mod,
        14 => I::Neg,
        15 => I::Eq,
        16 => I::Lt,
        17 => I::Gt,
        18 => I::Not,
        19 => I::And,
        20 => I::Or,
        21 => I::Xor,
        22 => I::Shl,
        23 => I::Shr,
        24 => I::BNot,
        25 => I::Sar,
        26 => I::LoadX,
        27 => I::StoreX,
        28 => I::Load(small(rng)),
        29 => I::Store(small(rng)),
        30 => I::Jmp(offset(rng)),
        31 => I::Jz(offset(rng)),
        32 => I::Jnz(offset(rng)),
        33 => I::JumpDest,
        34 => I::JmpX,
        35 => I::Call(offset(rng)),
        36 => I::Return,
        37 => I::CallX,
        38 => I::Sense(small(rng)),
        39 => I::Act(small(rng)),
        40 => I::Rand,
        41 => I::Gas,
        _ => I::Trace,
    }
}

/// A random program: a few pushes, then random instructions.
///
/// Relative jumps are often aimed at a `JUMPDEST` of the program, so that many programs
/// get past their first jump. Sometimes the code is cut or a byte is flipped, to reach
/// the decoding faults too.
pub fn program(rng: &mut Rng) -> Program {
    let constants: Vec<i64> = (0..rng.below(5)).map(|_| rng.value()).collect();
    let mut instructions: Vec<I> = (0..rng.below(4))
        .map(|_| I::PushI16(rng.value() as i16))
        .collect();
    let len = 1 + rng.below(40);
    instructions.extend((0..len).map(|_| {
        // Extra pushes and JUMPDESTs keep random code from emptying its stack at
        // once and give jumps somewhere to land.
        match rng.below(6) {
            0 => I::JumpDest,
            1 | 2 => I::PushI16(rng.value() as i16),
            _ => instruction(rng),
        }
    }));
    if rng.one_in(2) {
        instructions.push(I::Halt);
    }
    aim_jumps(rng, &mut instructions);
    let mut code = encode_all(&instructions);
    if rng.one_in(10) && code.len() > 1 {
        code.pop();
    }
    if rng.one_in(10) {
        let at = rng.below(code.len() as u64) as usize;
        code[at] = rng.next() as u8;
    }
    Program::from_parts(constants, code).expect("a generated program respects the A.1 limits")
}

/// Re-aims most relative jumps at a `JUMPDEST` of the program.
pub fn aim_jumps(rng: &mut Rng, instructions: &mut [I]) {
    let mut starts = Vec::with_capacity(instructions.len());
    let mut at = 0usize;
    for instruction in instructions.iter() {
        starts.push(at);
        at += instruction.size();
    }
    let dests: Vec<usize> = instructions
        .iter()
        .zip(&starts)
        .filter(|(i, _)| i.opcode() == JUMPDEST)
        .map(|(_, &start)| start)
        .collect();
    if dests.is_empty() {
        return;
    }
    for (instruction, &start) in instructions.iter_mut().zip(&starts) {
        if rng.one_in(5) {
            continue;
        }
        let next = (start + instruction.size()) as i64;
        let target = dests[rng.below(dests.len() as u64) as usize] as i64;
        let d = (target - next) as i16;
        match instruction {
            I::Jmp(_) => *instruction = I::Jmp(d),
            I::Jz(_) => *instruction = I::Jz(d),
            I::Jnz(_) => *instruction = I::Jnz(d),
            I::Call(_) => *instruction = I::Call(d),
            _ => {}
        }
    }
}
