//! The runtime faults of a turn (SPEC B.5).

use pbc::DecodeError;
use std::fmt;

/// Why a turn stopped early.
///
/// These are the fifteen runtime entries of B.5. The sixteenth, `BadHeader`, is a load
/// error and lives in [`pbc::LoadError`]: a program that does not load never runs, so a
/// turn can never end on it (journal D-002).
///
/// [`Fault::as_str`] gives the exact, normative string of each fault, the one that goes
/// into reports and protocol frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    /// `CONST k` or `CONSTX` with an index outside the constant table.
    BadConst,
    /// The byte at `pc` is not an opcode.
    BadOpcode,
    /// The operand of the instruction at `pc` runs past the end of the code.
    Truncated,
    /// `pc` is at or past the end of the code.
    PcOutOfRange,
    /// A jump target outside the code, or one whose byte is not `JUMPDEST`.
    BadJump,
    /// An instruction needed more values than the data stack holds.
    StackUnderflow,
    /// A push onto a full data stack (64 values).
    StackOverflow,
    /// A call with a full call stack (16 return addresses).
    CallStackOverflow,
    /// `RETURN` with an empty call stack.
    CallStackUnderflow,
    /// Not enough gas left for the next instruction. That instruction was not run.
    OutOfGas,
    /// `DIV` or `MOD` by zero.
    DivByZero,
    /// `LOADX` or `STOREX` with an address outside `0..=255`.
    OutOfBounds,
    /// `SENSE k` with a sensor the host does not know.
    BadSensor,
    /// `ACT k` with `k` outside `0..=3`.
    BadAction,
    /// A second `ACT` in the same turn.
    AlreadyActed,
}

impl Fault {
    /// Every fault, in the order of B.5.
    pub const ALL: [Fault; 15] = [
        Self::BadConst,
        Self::BadOpcode,
        Self::Truncated,
        Self::PcOutOfRange,
        Self::BadJump,
        Self::StackUnderflow,
        Self::StackOverflow,
        Self::CallStackOverflow,
        Self::CallStackUnderflow,
        Self::OutOfGas,
        Self::DivByZero,
        Self::OutOfBounds,
        Self::BadSensor,
        Self::BadAction,
        Self::AlreadyActed,
    ];

    /// The exact string of B.5 for this fault.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadConst => "BadConst",
            Self::BadOpcode => "BadOpcode",
            Self::Truncated => "Truncated",
            Self::PcOutOfRange => "PcOutOfRange",
            Self::BadJump => "BadJump",
            Self::StackUnderflow => "StackUnderflow",
            Self::StackOverflow => "StackOverflow",
            Self::CallStackOverflow => "CallStackOverflow",
            Self::CallStackUnderflow => "CallStackUnderflow",
            Self::OutOfGas => "OutOfGas",
            Self::DivByZero => "DivByZero",
            Self::OutOfBounds => "OutOfBounds",
            Self::BadSensor => "BadSensor",
            Self::BadAction => "BadAction",
            Self::AlreadyActed => "AlreadyActed",
        }
    }

    /// The fault whose B.5 string is `name`, if there is one. `"BadHeader"` gives `None`:
    /// it is not a runtime fault.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|fault| fault.as_str() == name)
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for Fault {}

impl From<DecodeError> for Fault {
    fn from(error: DecodeError) -> Self {
        match error {
            DecodeError::PcOutOfRange => Self::PcOutOfRange,
            DecodeError::BadOpcode => Self::BadOpcode,
            DecodeError::Truncated => Self::Truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for fault in Fault::ALL {
            assert_eq!(Fault::from_name(fault.as_str()), Some(fault));
            assert_eq!(fault.to_string(), fault.as_str());
        }
    }

    #[test]
    fn bad_header_is_not_a_runtime_fault() {
        assert_eq!(Fault::from_name(pbc::LoadError::FAULT_NAME), None);
        assert_eq!(Fault::from_name("badconst"), None);
    }

    #[test]
    fn decode_errors_keep_their_name() {
        assert_eq!(Fault::from(DecodeError::PcOutOfRange), Fault::PcOutOfRange);
        assert_eq!(Fault::from(DecodeError::BadOpcode), Fault::BadOpcode);
        assert_eq!(Fault::from(DecodeError::Truncated), Fault::Truncated);
    }
}
