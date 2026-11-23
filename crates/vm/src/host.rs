//! What the machine asks of the outside world: sensors, actions and randomness
//! (SPEC annex E, K.6).

use crate::Fault;

/// The four actions of E.4, by their `ACT k` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// `ACT 0`: move one cell; the argument is the direction.
    Move,
    /// `ACT 1`: pick up resources; the argument is ignored.
    Take,
    /// `ACT 2`: put down resources; the argument is the amount.
    Drop,
    /// `ACT 3`: turn carried resources into energy; the argument is the amount.
    Eat,
}

impl Action {
    /// Every action, in code order.
    pub const ALL: [Action; 4] = [Self::Move, Self::Take, Self::Drop, Self::Eat];

    /// The action of `ACT k`, or `None` for a `k` outside `0..=3` (which is `BadAction`).
    pub fn from_code(k: u8) -> Option<Self> {
        match k {
            0 => Some(Self::Move),
            1 => Some(Self::Take),
            2 => Some(Self::Drop),
            3 => Some(Self::Eat),
            _ => None,
        }
    }

    /// The `k` of `ACT k` for this action.
    pub fn code(self) -> u8 {
        match self {
            Self::Move => 0,
            Self::Take => 1,
            Self::Drop => 2,
            Self::Eat => 3,
        }
    }

    /// The name used in reports and protocol frames: `"MOVE"`, `"TAKE"`, `"DROP"`, `"EAT"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Move => "MOVE",
            Self::Take => "TAKE",
            Self::Drop => "DROP",
            Self::Eat => "EAT",
        }
    }

    /// The action called `name`, if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.as_str() == name)
    }
}

/// The action a turn attempted, kept even if it failed or the turn faulted afterwards
/// (E.2, K.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActRecord {
    /// Which action.
    pub action: Action,
    /// The argument popped by `ACT`.
    pub arg: i64,
    /// Whether the host reported a success.
    pub ok: bool,
}

/// The host's answer to `SENSE k`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseAnswer {
    /// The sensor's value, pushed onto the stack.
    Value(i64),
    /// A fault the machine raises instead (in practice `BadSensor`, K.6).
    Fault(Fault),
}

/// The world as the machine sees it during a turn.
///
/// The arena implements it locally; the remote server implements it by sending a
/// callback frame and waiting for the answer (K.6). Each method is called at most as
/// often as the program runs the matching instruction, in program order, and never for
/// an instruction refused before it runs (gas, stack room, `BadAction`, `AlreadyActed`).
///
/// `Error` is what the host itself can fail with — a closed connection, a bad frame. It
/// is not a fault of the program: [`run_turn`](crate::run_turn) stops and hands it back
/// unchanged (journal D-003). A host that cannot fail uses
/// [`std::convert::Infallible`].
pub trait Host {
    /// The host's own failure.
    type Error;

    /// Reads sensor `k`. Deciding which sensors exist is the host's job (E.3).
    fn sense(&mut self, k: u8) -> Result<SenseAnswer, Self::Error>;

    /// Attempts `action` with `arg`, and says whether it succeeded. Called at most once
    /// per turn.
    fn act(&mut self, action: Action, arg: i64) -> Result<bool, Self::Error>;

    /// Draws a random value.
    fn rand(&mut self) -> Result<i64, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names_round_trip() {
        for action in Action::ALL {
            assert_eq!(Action::from_code(action.code()), Some(action));
            assert_eq!(Action::from_name(action.as_str()), Some(action));
        }
        assert_eq!(Action::from_code(4), None);
        assert_eq!(Action::from_code(255), None);
        assert_eq!(Action::from_name("move"), None);
    }
}
