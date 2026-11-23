//! One turn of one agent: the execution cycle of SPEC B.2.

use crate::Fault;
use crate::gas;
use crate::host::{ActRecord, Action, Host, SenseAnswer};
use crate::ops;
use crate::stack::BoundedStack;
use pbc::{Instruction, Program};

/// Cells of an agent's memory, indices `0..=255` (B.1).
pub const MEMORY_CELLS: usize = 256;
/// Slots of the data stack (B.1).
pub const DATA_STACK_SLOTS: usize = 64;
/// Return addresses the call stack holds (B.1).
pub const CALL_STACK_DEPTH: usize = 16;

/// An agent's memory. It outlives the turn: the caller owns it and passes it to every
/// [`run_turn`] (B.1).
pub type Memory = [i64; MEMORY_CELLS];

/// What a turn produced, as K.3's `RESULT` reports it (the memory is the caller's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    /// Gas charged, including that of an instruction that faulted while running, but not
    /// that of an instruction refused for lack of gas (B.2).
    pub gas_used: u16,
    /// Why the turn stopped early, or `None` if it reached `HALT`.
    pub fault: Option<Fault>,
    /// The action the turn attempted, even if it failed or a fault followed (E.2).
    pub act: Option<ActRecord>,
    /// The values passed to `TRACE`, in order. At most `budget` of them (B.3).
    pub trace: Vec<i64>,
}

/// Runs one turn of `program` from `pc = 0`, with empty stacks and `budget` gas.
///
/// `mem` is read and written in place. On a fault, the writes made before it stay:
/// rolling them back is the arena's decision, not the machine's (E.2).
///
/// Returns `Err` only when the host itself fails (a callback that cannot be answered);
/// a fault of the program is a normal [`TurnResult`]. Memory for the trace is reserved
/// once, before the first instruction: the loop itself never allocates (journal D-008).
pub fn run_turn<H: Host>(
    program: &Program,
    mem: &mut Memory,
    budget: u16,
    host: &mut H,
) -> Result<TurnResult, H::Error> {
    let mut machine = Machine {
        code: program.code(),
        constants: program.constants(),
        mem,
        host,
        stack: BoundedStack::new(),
        calls: BoundedStack::new(),
        gas_left: budget,
        act: None,
        trace: Vec::with_capacity(usize::from(budget)),
    };
    let fault = machine.run()?;
    Ok(TurnResult {
        gas_used: budget.saturating_sub(machine.gas_left),
        fault,
        act: machine.act,
        trace: machine.trace,
    })
}

/// Where execution goes after an instruction.
enum Flow {
    /// The next instruction in the code.
    Next,
    /// A jump, call or return to this address.
    Goto(usize),
    /// `HALT`.
    Halt,
}

/// Why an instruction stopped the turn.
enum Stop<E> {
    /// A fault of the program: the turn ends normally, with this fault.
    Fault(Fault),
    /// The host failed: the turn has no result.
    Host(E),
}

impl<E> From<Fault> for Stop<E> {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

struct Machine<'a, H> {
    code: &'a [u8],
    constants: &'a [i64],
    mem: &'a mut Memory,
    host: &'a mut H,
    stack: BoundedStack<i64, DATA_STACK_SLOTS>,
    calls: BoundedStack<usize, CALL_STACK_DEPTH>,
    gas_left: u16,
    act: Option<ActRecord>,
    trace: Vec<i64>,
}

impl<H: Host> Machine<'_, H> {
    /// The B.2 cycle, until `HALT` or a fault. Ends because every instruction but `HALT`
    /// costs at least 1 gas.
    fn run(&mut self) -> Result<Option<Fault>, H::Error> {
        let mut pc = 0;
        loop {
            // Steps 1 to 3: pc in range, known opcode, operand inside the code.
            let instruction = match pbc::decode_at(self.code, pc) {
                Ok(instruction) => instruction,
                Err(error) => return Ok(Some(error.into())),
            };
            // Step 4: charge the gas, or refuse the instruction without charging it.
            match self.gas_left.checked_sub(gas::cost(&instruction)) {
                Some(left) => self.gas_left = left,
                None => return Ok(Some(Fault::OutOfGas)),
            }
            // Steps 5 and 6. `pc < code.len() <= 65535`, so `next` cannot overflow.
            let next = pc + instruction.size();
            match self.execute(instruction, next) {
                Ok(Flow::Next) => pc = next,
                Ok(Flow::Goto(target)) => pc = target,
                Ok(Flow::Halt) => return Ok(None),
                Err(Stop::Fault(fault)) => return Ok(Some(fault)),
                Err(Stop::Host(error)) => return Err(error),
            }
        }
    }

    /// Runs one instruction whose gas is already charged. `next` is the address of the
    /// following instruction: the base of relative jumps and the return address of calls.
    ///
    /// Check order, where the spec leaves it open (journal D-017): first the stack
    /// (pop the operands, then make sure the result fits), then the rest.
    fn execute(&mut self, instruction: Instruction, next: usize) -> Result<Flow, Stop<H::Error>> {
        use Instruction as I;
        match instruction {
            I::Halt => return Ok(Flow::Halt),
            I::Const(k) => {
                self.need_room()?;
                let value = self.constant(i64::from(k))?;
                self.push(value)?;
            }
            I::PushI16(n) => self.push(i64::from(n))?,
            I::Drop => {
                self.pop()?;
            }
            I::Dup => {
                let a = self.pop()?;
                self.push(a)?;
                self.push(a)?;
            }
            I::Swap => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(a)?;
            }
            I::Over => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a)?;
                self.push(b)?;
                self.push(a)?;
            }
            I::Rot => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(b)?;
                self.push(c)?;
                self.push(a)?;
            }
            I::ConstX => {
                let k = self.pop()?;
                let value = self.constant(k)?;
                self.push(value)?;
            }
            I::Add => self.binary(ops::add)?,
            I::Sub => self.binary(ops::sub)?,
            I::Mul => self.binary(ops::mul)?,
            I::Div => self.division(ops::div)?,
            I::Mod => self.division(ops::rem)?,
            I::Neg => self.unary(ops::neg)?,
            I::Eq => self.binary(ops::eq)?,
            I::Lt => self.binary(ops::lt)?,
            I::Gt => self.binary(ops::gt)?,
            I::Not => self.unary(ops::not)?,
            I::And => self.binary(ops::and)?,
            I::Or => self.binary(ops::or)?,
            I::Xor => self.binary(ops::xor)?,
            I::Shl => self.binary(ops::shl)?,
            I::Shr => self.binary(ops::shr)?,
            I::BNot => self.unary(ops::bnot)?,
            I::Sar => self.binary(ops::sar)?,
            I::LoadX => {
                let addr = self.pop()?;
                let value = self.load(cell(addr)?)?;
                self.push(value)?;
            }
            I::StoreX => {
                let addr = self.pop()?;
                let value = self.pop()?;
                self.store(cell(addr)?, value)?;
            }
            I::Load(k) => {
                self.need_room()?;
                let value = self.load(usize::from(k))?;
                self.push(value)?;
            }
            I::Store(k) => {
                let value = self.pop()?;
                self.store(usize::from(k), value)?;
            }
            I::Jmp(d) => return Ok(Flow::Goto(ops::relative(self.code, next, d)?)),
            I::Jz(d) => {
                if self.pop()? == 0 {
                    return Ok(Flow::Goto(ops::relative(self.code, next, d)?));
                }
            }
            I::Jnz(d) => {
                if self.pop()? != 0 {
                    return Ok(Flow::Goto(ops::relative(self.code, next, d)?));
                }
            }
            I::JumpDest => {}
            I::JmpX => {
                let target = self.pop()?;
                return Ok(Flow::Goto(ops::jump_target(self.code, target)?));
            }
            I::Call(d) => {
                self.push_frame(next)?;
                return Ok(Flow::Goto(ops::relative(self.code, next, d)?));
            }
            I::Return => {
                let back = self.calls.pop().ok_or(Fault::CallStackUnderflow)?;
                return Ok(Flow::Goto(back));
            }
            I::CallX => {
                let target = self.pop()?;
                self.push_frame(next)?;
                return Ok(Flow::Goto(ops::jump_target(self.code, target)?));
            }
            I::Sense(k) => {
                // Room first, so a push that would fail never costs a callback.
                self.need_room()?;
                match self.host.sense(k).map_err(Stop::Host)? {
                    SenseAnswer::Value(value) => self.push(value)?,
                    SenseAnswer::Fault(fault) => return Err(fault.into()),
                }
            }
            I::Act(k) => {
                let arg = self.pop()?;
                let action = Action::from_code(k).ok_or(Fault::BadAction)?;
                if self.act.is_some() {
                    return Err(Fault::AlreadyActed.into());
                }
                let ok = self.host.act(action, arg).map_err(Stop::Host)?;
                self.act = Some(ActRecord { action, arg, ok });
                // The argument was just popped, so there is room (E.4).
                self.push(i64::from(ok))?;
            }
            I::Rand => {
                self.need_room()?;
                let value = self.host.rand().map_err(Stop::Host)?;
                self.push(value)?;
            }
            I::Gas => self.push(i64::from(self.gas_left))?,
            I::Trace => {
                let value = self.pop()?;
                // Within capacity: TRACE costs 1 gas, so a turn holds at most `budget`
                // of them, and the trace was reserved for `budget` values.
                self.trace.push(value);
            }
        }
        Ok(Flow::Next)
    }

    fn pop(&mut self) -> Result<i64, Fault> {
        self.stack.pop().ok_or(Fault::StackUnderflow)
    }

    fn push(&mut self, value: i64) -> Result<(), Fault> {
        self.stack.push(value).map_err(|_| Fault::StackOverflow)
    }

    /// Fails with `StackOverflow` if a push would, without pushing anything.
    fn need_room(&self) -> Result<(), Fault> {
        if self.stack.is_full() {
            Err(Fault::StackOverflow)
        } else {
            Ok(())
        }
    }

    fn unary(&mut self, op: impl FnOnce(i64) -> i64) -> Result<(), Fault> {
        let a = self.pop()?;
        self.push(op(a))
    }

    /// `a b -- op(a, b)`: `b` is on top, so it is popped first (B.3).
    fn binary(&mut self, op: impl FnOnce(i64, i64) -> i64) -> Result<(), Fault> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(op(a, b))
    }

    /// `DIV` and `MOD`: like [`Self::binary`], but the operation can fault.
    fn division(&mut self, op: impl FnOnce(i64, i64) -> Result<i64, Fault>) -> Result<(), Fault> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(op(a, b)?)
    }

    fn constant(&self, k: i64) -> Result<i64, Fault> {
        ops::constant(self.constants, k)
    }

    fn load(&self, addr: usize) -> Result<i64, Fault> {
        self.mem.get(addr).copied().ok_or(Fault::OutOfBounds)
    }

    fn store(&mut self, addr: usize, value: i64) -> Result<(), Fault> {
        let slot = self.mem.get_mut(addr).ok_or(Fault::OutOfBounds)?;
        *slot = value;
        Ok(())
    }

    fn push_frame(&mut self, back: usize) -> Result<(), Fault> {
        self.calls.push(back).map_err(|_| Fault::CallStackOverflow)
    }
}

/// A memory address popped by `LOADX` or `STOREX`, if it is in `0..=255`.
fn cell(addr: i64) -> Result<usize, Fault> {
    usize::try_from(addr)
        .ok()
        .filter(|&addr| addr < MEMORY_CELLS)
        .ok_or(Fault::OutOfBounds)
}
