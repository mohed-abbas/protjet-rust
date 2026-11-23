//! The static bytecode verifier (bonus of the README §6, journal D-014).
//!
//! [`verify`] reads a program without running it and either proves that no execution
//! can raise one of the [`EXCLUDED_FAULTS`] (bad decoding, bad jump, stack or call
//! stack overflow or underflow), with a bound on the stack height, or says where one
//! might happen.
//!
//! # How
//!
//! An abstract interpretation from `pc = 0`, the way every turn starts. Each state is a
//! `pc`, the exact call stack (return addresses) and the exact stack height, plus, for
//! each stack slot, the set of values it may hold: a few known values, or "any value".
//! Values the turn cannot know in advance are "any": memory (it persists between turns),
//! `SENSE`, `RAND`, `GAS`. Known values flow through the same operations the engine runs
//! (`ops`), which is what lets the verifier follow a dispatch table read by `CONSTX`.
//!
//! The exploration follows every path the engine could take: both sides of a `JZ` whose
//! condition is unknown, every target of a `JMPX` from a known set. A path ends at
//! `HALT`, or at an instruction that always faults in a way the verifier does not exclude
//! (`DivByZero` by a known 0, `BadConst`, `BadAction`). Decoding starts at each reached
//! `pc`, never from a linear sweep: a jump into an operand that holds `0x43` is legal
//! (A.3), and unreachable bytes are never looked at.
//!
//! # What it rejects that may be fine
//!
//! It is sound, not complete. It rejects a `JMPX`/`CALLX` whose target it cannot narrow
//! down ([`Reason::UnknownTarget`]); a loop whose stack grows on each turn round, even if
//! a counter would stop it in time; recursion deeper than the call stack, even if a base
//! case would stop it; and programs too large for its limits ([`Reason::TooComplex`]).

use crate::host::Action;
use crate::{CALL_STACK_DEPTH, DATA_STACK_SLOTS, Fault, ops};
use pbc::{Instruction, Program};
use std::collections::{HashMap, VecDeque};
use std::fmt;

/// The most distinct values tracked for one stack slot. One more and the slot is "any
/// value". Enough for small dispatch tables and loop counters that soon become unknown.
pub const MAX_KNOWN_VALUES: usize = 8;

/// The most stack slots the verifier stores, summed over all the states it explores.
///
/// This bounds its memory and time on any input, however crafted, as A.2 requires of the
/// loader. A program needing more is rejected with [`Reason::TooComplex`].
pub const MAX_SLOTS: usize = 1 << 18;

/// The faults a verified program can never raise, whatever its memory, budget and host.
pub const EXCLUDED_FAULTS: [Fault; 8] = [
    Fault::PcOutOfRange,
    Fault::BadOpcode,
    Fault::Truncated,
    Fault::BadJump,
    Fault::StackUnderflow,
    Fault::StackOverflow,
    Fault::CallStackOverflow,
    Fault::CallStackUnderflow,
];

/// What the verifier proved about an accepted program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    /// The highest the data stack can get, at most [`DATA_STACK_SLOTS`].
    pub max_stack: usize,
    /// The deepest the call stack can get, at most [`CALL_STACK_DEPTH`].
    pub max_call_depth: usize,
    /// How many states (`pc`, call stack, stack height) the exploration visited.
    pub states: usize,
}

/// Why the verifier refused a program, and at which instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejection {
    /// The instruction where the problem was found. For a decoding fault, the address
    /// where decoding fails; for falling off the code, the instruction that falls.
    pub pc: usize,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Some execution may raise this fault here. Always one of [`EXCLUDED_FAULTS`].
    MayFault(Fault),
    /// A `JMPX` or `CALLX` whose target the verifier cannot narrow to known values.
    UnknownTarget,
    /// The program needs more than [`MAX_SLOTS`] to explore.
    TooComplex,
}

impl From<Fault> for Reason {
    fn from(fault: Fault) -> Self {
        Self::MayFault(fault)
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.reason {
            Reason::MayFault(fault) => write!(f, "pc {}: may fault with {fault}", self.pc),
            Reason::UnknownTarget => write!(f, "pc {}: jump target not known statically", self.pc),
            Reason::TooComplex => write!(f, "pc {}: program too complex to verify", self.pc),
        }
    }
}

impl std::error::Error for Rejection {}

/// Proves that `program` cannot raise any of the [`EXCLUDED_FAULTS`], or says where it
/// might. Deterministic: the same program always gets the same answer.
pub fn verify(program: &Program) -> Result<Verified, Rejection> {
    let mut verifier = Verifier {
        code: program.code(),
        constants: program.constants(),
        states: HashMap::new(),
        queue: VecDeque::new(),
        slots: 0,
        max_stack: 0,
        max_call_depth: 0,
    };
    verifier.run()
}

/// What a stack slot may hold.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    /// Anything.
    Any,
    /// One of these: sorted, without duplicates, `1..=MAX_KNOWN_VALUES` of them.
    OneOf(Vec<i64>),
}

impl Value {
    fn exactly(value: i64) -> Self {
        Self::OneOf(vec![value])
    }

    /// The set of `values`, `Any` if there are too many, `None` if there are none (the
    /// path that would produce them always faults).
    fn from_values(mut values: Vec<i64>) -> Option<Self> {
        values.sort_unstable();
        values.dedup();
        match values.len() {
            0 => None,
            n if n > MAX_KNOWN_VALUES => Some(Self::Any),
            _ => Some(Self::OneOf(values)),
        }
    }

    /// The smallest value that covers both.
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::OneOf(a), Self::OneOf(b)) => {
                let both = a.iter().chain(b).copied().collect();
                Self::from_values(both).unwrap_or(Self::Any)
            }
            _ => Self::Any,
        }
    }

    fn may_be_zero(&self) -> bool {
        match self {
            Self::Any => true,
            Self::OneOf(values) => values.contains(&0),
        }
    }

    fn may_be_nonzero(&self) -> bool {
        match self {
            Self::Any => true,
            Self::OneOf(values) => values.iter().any(|&v| v != 0),
        }
    }

    /// `op` on every value. `None` if it fails on all of them.
    fn map(&self, op: impl Fn(i64) -> Option<i64>) -> Option<Self> {
        match self {
            Self::Any => Some(Self::Any),
            Self::OneOf(values) => {
                Self::from_values(values.iter().filter_map(|&a| op(a)).collect())
            }
        }
    }

    /// `op` on every pair. `None` if it fails on all of them.
    fn map2(a: &Self, b: &Self, op: impl Fn(i64, i64) -> Option<i64>) -> Option<Self> {
        match (a, b) {
            (Self::OneOf(a), Self::OneOf(b)) => {
                let pairs = a.iter().flat_map(|&x| b.iter().map(move |&y| (x, y)));
                Self::from_values(pairs.filter_map(|(x, y)| op(x, y)).collect())
            }
            _ => Some(Self::Any),
        }
    }
}

/// A point of the exploration. The stack height is part of it, so two paths meet only
/// if their stacks have the same height: values merge, heights never do.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    pc: usize,
    calls: Vec<usize>,
    height: usize,
}

struct Entry {
    stack: Vec<Value>,
    queued: bool,
}

struct Verifier<'a> {
    code: &'a [u8],
    constants: &'a [i64],
    states: HashMap<Key, Entry>,
    /// States whose successors must be (re)computed, oldest first.
    queue: VecDeque<Key>,
    slots: usize,
    max_stack: usize,
    max_call_depth: usize,
}

impl Verifier<'_> {
    fn run(&mut self) -> Result<Verified, Rejection> {
        let start = Machine {
            stack: Vec::new(),
            calls: Vec::new(),
        };
        self.reach(0, start)
            .map_err(|reason| Rejection { pc: 0, reason })?;

        while let Some(key) = self.queue.pop_front() {
            let Some(entry) = self.states.get_mut(&key) else {
                continue;
            };
            entry.queued = false;
            let machine = Machine {
                stack: entry.stack.clone(),
                calls: key.calls.clone(),
            };
            let pc = key.pc;
            self.step(pc, machine)
                .map_err(|reason| Rejection { pc, reason })?;
        }

        Ok(Verified {
            max_stack: self.max_stack,
            max_call_depth: self.max_call_depth,
            states: self.states.len(),
        })
    }

    /// Runs the instruction at `pc` abstractly, and records the states it leads to.
    fn step(&mut self, pc: usize, mut machine: Machine) -> Result<(), Reason> {
        let instruction = pbc::decode_at(self.code, pc).map_err(Fault::from)?;
        let next = pc + instruction.size();
        let targets = match machine.execute(instruction, next, self.code, self.constants)? {
            Flow::Next => vec![next],
            Flow::Branch(target) => vec![next, target],
            Flow::To(targets) => targets,
            Flow::Stop => Vec::new(),
        };
        for target in targets {
            // Jump targets are already checked; falling through or returning past the
            // end is caught here, at the instruction responsible.
            if target >= self.code.len() {
                return Err(Fault::PcOutOfRange.into());
            }
            self.reach(target, machine.clone())?;
        }
        Ok(())
    }

    /// Records that `machine` can be at `pc`: a new state, or a widening of a known one.
    fn reach(&mut self, pc: usize, machine: Machine) -> Result<(), Reason> {
        let key = Key {
            pc,
            calls: machine.calls,
            height: machine.stack.len(),
        };
        if let Some(entry) = self.states.get_mut(&key) {
            let mut changed = false;
            for (old, new) in entry.stack.iter_mut().zip(&machine.stack) {
                let joined = old.join(new);
                if joined != *old {
                    *old = joined;
                    changed = true;
                }
            }
            if changed && !entry.queued {
                entry.queued = true;
                self.queue.push_back(key);
            }
            return Ok(());
        }

        self.slots += key.height + 1;
        if self.slots > MAX_SLOTS {
            return Err(Reason::TooComplex);
        }
        self.max_stack = self.max_stack.max(key.height);
        self.max_call_depth = self.max_call_depth.max(key.calls.len());
        self.queue.push_back(key.clone());
        self.states.insert(
            key,
            Entry {
                stack: machine.stack,
                queued: true,
            },
        );
        Ok(())
    }
}

/// Where the abstract execution goes after an instruction.
enum Flow {
    /// The next instruction.
    Next,
    /// The next instruction or this target: a conditional jump that may go either way.
    Branch(usize),
    /// One of these targets, all checked.
    To(Vec<usize>),
    /// Nowhere: `HALT`, or an instruction that always faults with a fault the verifier
    /// does not exclude.
    Stop,
}

/// The abstract machine of one state. Mirrors the engine's `execute`, check for check
/// (journal D-017), so a stack or call-stack fault is found exactly where the engine
/// would raise it.
#[derive(Clone)]
struct Machine {
    stack: Vec<Value>,
    calls: Vec<usize>,
}

impl Machine {
    fn execute(
        &mut self,
        instruction: Instruction,
        next: usize,
        code: &[u8],
        constants: &[i64],
    ) -> Result<Flow, Reason> {
        use Instruction as I;
        match instruction {
            I::Halt => Ok(Flow::Stop),
            I::Const(k) => {
                self.need_room()?;
                match ops::constant(constants, i64::from(k)) {
                    Ok(value) => self.push_then_next(Value::exactly(value)),
                    Err(_) => Ok(Flow::Stop),
                }
            }
            I::PushI16(n) => self.push_then_next(Value::exactly(i64::from(n))),
            I::Drop => {
                self.pop()?;
                Ok(Flow::Next)
            }
            I::Dup => {
                let a = self.pop()?;
                self.push(a.clone())?;
                self.push_then_next(a)
            }
            I::Swap => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push_then_next(a)
            }
            I::Over => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.clone())?;
                self.push(b)?;
                self.push_then_next(a)
            }
            I::Rot => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(c)?;
                self.push_then_next(a)
            }
            I::ConstX => {
                let k = self.pop()?;
                let value = match k {
                    // Any index: any entry of the table.
                    Value::Any => Value::from_values(constants.to_vec()),
                    known => known.map(|k| ops::constant(constants, k).ok()),
                };
                self.push_or_stop(value)
            }
            I::Add => self.binary(|a, b| Some(ops::add(a, b))),
            I::Sub => self.binary(|a, b| Some(ops::sub(a, b))),
            I::Mul => self.binary(|a, b| Some(ops::mul(a, b))),
            I::Div => self.binary(|a, b| ops::div(a, b).ok()),
            I::Mod => self.binary(|a, b| ops::rem(a, b).ok()),
            I::Neg => self.unary(ops::neg),
            I::Eq => self.binary(|a, b| Some(ops::eq(a, b))),
            I::Lt => self.binary(|a, b| Some(ops::lt(a, b))),
            I::Gt => self.binary(|a, b| Some(ops::gt(a, b))),
            I::Not => self.unary(ops::not),
            I::And => self.binary(|a, b| Some(ops::and(a, b))),
            I::Or => self.binary(|a, b| Some(ops::or(a, b))),
            I::Xor => self.binary(|a, b| Some(ops::xor(a, b))),
            I::Shl => self.binary(|a, b| Some(ops::shl(a, b))),
            I::Shr => self.binary(|a, b| Some(ops::shr(a, b))),
            I::BNot => self.unary(ops::bnot),
            I::Sar => self.binary(|a, b| Some(ops::sar(a, b))),
            // Memory persists between turns: whatever is read is unknown.
            I::LoadX => {
                self.pop()?;
                self.push_then_next(Value::Any)
            }
            I::StoreX => {
                self.pop()?;
                self.pop()?;
                Ok(Flow::Next)
            }
            I::Load(_) => {
                self.need_room()?;
                self.push_then_next(Value::Any)
            }
            I::Store(_) => {
                self.pop()?;
                Ok(Flow::Next)
            }
            I::Jmp(d) => Ok(Flow::To(vec![ops::relative(code, next, d)?])),
            I::Jz(d) => {
                let condition = self.pop()?;
                branch(
                    code,
                    next,
                    d,
                    condition.may_be_zero(),
                    condition.may_be_nonzero(),
                )
            }
            I::Jnz(d) => {
                let condition = self.pop()?;
                branch(
                    code,
                    next,
                    d,
                    condition.may_be_nonzero(),
                    condition.may_be_zero(),
                )
            }
            I::JumpDest => Ok(Flow::Next),
            I::JmpX => {
                let target = self.pop()?;
                Ok(Flow::To(targets(code, &target)?))
            }
            I::Call(d) => {
                self.push_frame(next)?;
                Ok(Flow::To(vec![ops::relative(code, next, d)?]))
            }
            I::Return => {
                let back = self.calls.pop().ok_or(Fault::CallStackUnderflow)?;
                Ok(Flow::To(vec![back]))
            }
            I::CallX => {
                let target = self.pop()?;
                self.push_frame(next)?;
                Ok(Flow::To(targets(code, &target)?))
            }
            I::Sense(_) | I::Rand => {
                self.need_room()?;
                self.push_then_next(Value::Any)
            }
            I::Act(k) => {
                self.pop()?;
                if Action::from_code(k).is_none() {
                    return Ok(Flow::Stop); // always BadAction
                }
                self.push_then_next(Value::OneOf(vec![0, 1]))
            }
            I::Gas => self.push_then_next(Value::Any),
            I::Trace => {
                self.pop()?;
                Ok(Flow::Next)
            }
        }
    }

    fn pop(&mut self) -> Result<Value, Fault> {
        self.stack.pop().ok_or(Fault::StackUnderflow)
    }

    fn need_room(&self) -> Result<(), Fault> {
        if self.stack.len() >= DATA_STACK_SLOTS {
            Err(Fault::StackOverflow)
        } else {
            Ok(())
        }
    }

    fn push(&mut self, value: Value) -> Result<(), Fault> {
        self.need_room()?;
        self.stack.push(value);
        Ok(())
    }

    fn push_then_next(&mut self, value: Value) -> Result<Flow, Reason> {
        self.push(value)?;
        Ok(Flow::Next)
    }

    /// Pushes `value`, or ends the path if the instruction always faults (`None`).
    fn push_or_stop(&mut self, value: Option<Value>) -> Result<Flow, Reason> {
        match value {
            Some(value) => self.push_then_next(value),
            None => Ok(Flow::Stop),
        }
    }

    fn unary(&mut self, op: fn(i64) -> i64) -> Result<Flow, Reason> {
        let a = self.pop()?;
        let value = a.map(|a| Some(op(a)));
        self.push_or_stop(value)
    }

    /// `a b -- op(a, b)`, `b` popped first. `op` returns `None` where the engine faults.
    fn binary(&mut self, op: impl Fn(i64, i64) -> Option<i64>) -> Result<Flow, Reason> {
        let b = self.pop()?;
        let a = self.pop()?;
        let value = Value::map2(&a, &b, op);
        self.push_or_stop(value)
    }

    fn push_frame(&mut self, back: usize) -> Result<(), Fault> {
        if self.calls.len() >= CALL_STACK_DEPTH {
            return Err(Fault::CallStackOverflow);
        }
        self.calls.push(back);
        Ok(())
    }
}

/// A conditional jump that is `taken` and/or `falls` through, depending on what the
/// condition may be. The target is only checked if the jump may be taken, as in the
/// engine (B.4).
fn branch(code: &[u8], next: usize, d: i16, taken: bool, falls: bool) -> Result<Flow, Reason> {
    if !taken {
        return Ok(Flow::Next);
    }
    let target = ops::relative(code, next, d)?;
    Ok(if falls {
        Flow::Branch(target)
    } else {
        Flow::To(vec![target])
    })
}

/// The checked targets of a `JMPX` or `CALLX` that pops `target`.
fn targets(code: &[u8], target: &Value) -> Result<Vec<usize>, Reason> {
    match target {
        Value::Any => Err(Reason::UnknownTarget),
        Value::OneOf(values) => values
            .iter()
            .map(|&t| ops::jump_target(code, t).map_err(Reason::from))
            .collect(),
    }
}

#[cfg(test)]
mod tests;
