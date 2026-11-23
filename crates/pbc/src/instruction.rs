//! The 43 instructions of PBC v1 and how they are decoded (SPEC B.2, B.3).
//!
//! Bytes become an [`Instruction`] in one place, [`decode_at`], and an [`Instruction`]
//! becomes bytes in one place, [`Instruction::encode_into`]. The tests check that the
//! two agree for every byte value.

use thiserror::Error;

/// The opcode of `JUMPDEST`. Every jump target must hold this byte (A.3, B.4).
pub const JUMPDEST: u8 = 0x43;

/// One decoded instruction, with its operand if it has one.
///
/// Mnemonics follow the spec's table, but only the byte, operand, gas, stack effect
/// and semantics are normative (B.3). Gas costs and semantics live in the `vm` crate:
/// this crate only knows the shape of the bytes (journal D-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    /// `0x00`: ends the turn normally.
    Halt,
    /// `0x01 k:u16`: push `constants[k]`.
    Const(u16),
    /// `0x02 n:i16`: push `n`, sign-extended.
    PushI16(i16),
    /// `0x03`: `a --`.
    Drop,
    /// `0x04`: `a -- a a`.
    Dup,
    /// `0x05`: `a b -- b a`.
    Swap,
    /// `0x06`: `a b -- a b a`.
    Over,
    /// `0x07`: `a b c -- b c a`.
    Rot,
    /// `0x08`: `k -- constants[k]`.
    ConstX,
    /// `0x10`: `a b -- a+b`.
    Add,
    /// `0x11`: `a b -- a-b`.
    Sub,
    /// `0x12`: `a b -- a*b`.
    Mul,
    /// `0x13`: `a b -- a/b`.
    Div,
    /// `0x14`: `a b -- a%b`.
    Mod,
    /// `0x15`: `a -- -a`.
    Neg,
    /// `0x20`: `a b -- (a == b)`.
    Eq,
    /// `0x21`: `a b -- (a < b)`.
    Lt,
    /// `0x22`: `a b -- (a > b)`.
    Gt,
    /// `0x23`: `a -- (a == 0)`, logical.
    Not,
    /// `0x24`: `a b -- a&b`.
    And,
    /// `0x25`: `a b -- a|b`.
    Or,
    /// `0x26`: `a b -- a^b`.
    Xor,
    /// `0x27`: `a n -- a << n`, logical.
    Shl,
    /// `0x28`: `a n -- a >> n`, logical.
    Shr,
    /// `0x29`: `a -- !a`, bitwise.
    BNot,
    /// `0x2D`: `a n -- a >> n`, arithmetic.
    Sar,
    /// `0x30`: `addr -- mem[addr]`.
    LoadX,
    /// `0x31`: `v addr --`, `mem[addr] = v`.
    StoreX,
    /// `0x32 k:u8`: push `mem[k]`.
    Load(u8),
    /// `0x33 k:u8`: `v --`, `mem[k] = v`.
    Store(u8),
    /// `0x40 d:i16`: relative jump.
    Jmp(i16),
    /// `0x41 d:i16`: `c --`, relative jump if `c == 0`.
    Jz(i16),
    /// `0x42 d:i16`: `c --`, relative jump if `c != 0`.
    Jnz(i16),
    /// `0x43`: marks a valid jump target.
    JumpDest,
    /// `0x44`: `t --`, jump to the absolute target `t`.
    JmpX,
    /// `0x50 d:i16`: push the return address, then relative jump.
    Call(i16),
    /// `0x51`: pop the return address and go there.
    Return,
    /// `0x52`: `t --`, push the return address, then jump to the absolute target `t`.
    CallX,
    /// `0x60 k:u8`: push the value of sensor `k`.
    Sense(u8),
    /// `0x61 k:u8`: `arg -- ok`, perform action `k`.
    Act(u8),
    /// `0x70`: push a random value supplied by the host.
    Rand,
    /// `0x71`: push the gas left, after this instruction's own cost.
    Gas,
    /// `0xF0`: `v --`, append `v` to the turn's trace.
    Trace,
}

/// Why no instruction could be decoded at an address (SPEC B.2 steps 1–3).
///
/// Each variant becomes the runtime fault of the same name in the `vm` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The address is at or past the end of the code (step 1).
    #[error("pc is past the end of the code")]
    PcOutOfRange,
    /// The byte at the address is not an opcode of B.3 (step 2).
    #[error("unknown opcode")]
    BadOpcode,
    /// The operand runs past the end of the code (step 3).
    #[error("the operand runs past the end of the code")]
    Truncated,
}

/// Decodes the instruction that starts at `pc`.
///
/// Checks happen in the order of B.2: `pc` inside the code, then a known opcode, then
/// an operand that fits. Decoding never looks at the bytes before `pc`, so it works at
/// any address, including the middle of another instruction's operand (A.3).
pub fn decode_at(code: &[u8], pc: usize) -> Result<Instruction, DecodeError> {
    use Instruction as I;

    let opcode = *code.get(pc).ok_or(DecodeError::PcOutOfRange)?;
    // The operand starts right after the opcode byte. `pc < code.len()`, so `pc + 1`
    // cannot overflow.
    let u8_operand = || operand::<1>(code, pc + 1).map(|[b]| b);
    let u16_operand = || operand::<2>(code, pc + 1).map(u16::from_be_bytes);
    let i16_operand = || operand::<2>(code, pc + 1).map(i16::from_be_bytes);

    let instruction = match opcode {
        0x00 => I::Halt,
        0x01 => I::Const(u16_operand()?),
        0x02 => I::PushI16(i16_operand()?),
        0x03 => I::Drop,
        0x04 => I::Dup,
        0x05 => I::Swap,
        0x06 => I::Over,
        0x07 => I::Rot,
        0x08 => I::ConstX,
        0x10 => I::Add,
        0x11 => I::Sub,
        0x12 => I::Mul,
        0x13 => I::Div,
        0x14 => I::Mod,
        0x15 => I::Neg,
        0x20 => I::Eq,
        0x21 => I::Lt,
        0x22 => I::Gt,
        0x23 => I::Not,
        0x24 => I::And,
        0x25 => I::Or,
        0x26 => I::Xor,
        0x27 => I::Shl,
        0x28 => I::Shr,
        0x29 => I::BNot,
        0x2D => I::Sar,
        0x30 => I::LoadX,
        0x31 => I::StoreX,
        0x32 => I::Load(u8_operand()?),
        0x33 => I::Store(u8_operand()?),
        0x40 => I::Jmp(i16_operand()?),
        0x41 => I::Jz(i16_operand()?),
        0x42 => I::Jnz(i16_operand()?),
        JUMPDEST => I::JumpDest,
        0x44 => I::JmpX,
        0x50 => I::Call(i16_operand()?),
        0x51 => I::Return,
        0x52 => I::CallX,
        0x60 => I::Sense(u8_operand()?),
        0x61 => I::Act(u8_operand()?),
        0x70 => I::Rand,
        0x71 => I::Gas,
        0xF0 => I::Trace,
        _ => return Err(DecodeError::BadOpcode),
    };
    Ok(instruction)
}

/// Reads the `N` operand bytes starting at `start`, or `Truncated` if the code ends first.
fn operand<const N: usize>(code: &[u8], start: usize) -> Result<[u8; N], DecodeError> {
    let end = start.checked_add(N).ok_or(DecodeError::Truncated)?;
    let bytes = code.get(start..end).ok_or(DecodeError::Truncated)?;
    <[u8; N]>::try_from(bytes).map_err(|_| DecodeError::Truncated)
}

impl Instruction {
    /// The opcode byte of this instruction.
    pub fn opcode(&self) -> u8 {
        match self {
            Self::Halt => 0x00,
            Self::Const(_) => 0x01,
            Self::PushI16(_) => 0x02,
            Self::Drop => 0x03,
            Self::Dup => 0x04,
            Self::Swap => 0x05,
            Self::Over => 0x06,
            Self::Rot => 0x07,
            Self::ConstX => 0x08,
            Self::Add => 0x10,
            Self::Sub => 0x11,
            Self::Mul => 0x12,
            Self::Div => 0x13,
            Self::Mod => 0x14,
            Self::Neg => 0x15,
            Self::Eq => 0x20,
            Self::Lt => 0x21,
            Self::Gt => 0x22,
            Self::Not => 0x23,
            Self::And => 0x24,
            Self::Or => 0x25,
            Self::Xor => 0x26,
            Self::Shl => 0x27,
            Self::Shr => 0x28,
            Self::BNot => 0x29,
            Self::Sar => 0x2D,
            Self::LoadX => 0x30,
            Self::StoreX => 0x31,
            Self::Load(_) => 0x32,
            Self::Store(_) => 0x33,
            Self::Jmp(_) => 0x40,
            Self::Jz(_) => 0x41,
            Self::Jnz(_) => 0x42,
            Self::JumpDest => JUMPDEST,
            Self::JmpX => 0x44,
            Self::Call(_) => 0x50,
            Self::Return => 0x51,
            Self::CallX => 0x52,
            Self::Sense(_) => 0x60,
            Self::Act(_) => 0x61,
            Self::Rand => 0x70,
            Self::Gas => 0x71,
            Self::Trace => 0xF0,
        }
    }

    /// The spec's mnemonic for this instruction, without its operand.
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Self::Halt => "HALT",
            Self::Const(_) => "CONST",
            Self::PushI16(_) => "PUSHI16",
            Self::Drop => "DROP",
            Self::Dup => "DUP",
            Self::Swap => "SWAP",
            Self::Over => "OVER",
            Self::Rot => "ROT",
            Self::ConstX => "CONSTX",
            Self::Add => "ADD",
            Self::Sub => "SUB",
            Self::Mul => "MUL",
            Self::Div => "DIV",
            Self::Mod => "MOD",
            Self::Neg => "NEG",
            Self::Eq => "EQ",
            Self::Lt => "LT",
            Self::Gt => "GT",
            Self::Not => "NOT",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Xor => "XOR",
            Self::Shl => "SHL",
            Self::Shr => "SHR",
            Self::BNot => "BNOT",
            Self::Sar => "SAR",
            Self::LoadX => "LOADX",
            Self::StoreX => "STOREX",
            Self::Load(_) => "LOAD",
            Self::Store(_) => "STORE",
            Self::Jmp(_) => "JMP",
            Self::Jz(_) => "JZ",
            Self::Jnz(_) => "JNZ",
            Self::JumpDest => "JUMPDEST",
            Self::JmpX => "JMPX",
            Self::Call(_) => "CALL",
            Self::Return => "RETURN",
            Self::CallX => "CALLX",
            Self::Sense(_) => "SENSE",
            Self::Act(_) => "ACT",
            Self::Rand => "RAND",
            Self::Gas => "GAS",
            Self::Trace => "TRACE",
        }
    }

    /// How many bytes the instruction takes in the code: the opcode plus its operand.
    pub fn size(&self) -> usize {
        let operand_len = match self {
            Self::Const(_)
            | Self::PushI16(_)
            | Self::Jmp(_)
            | Self::Jz(_)
            | Self::Jnz(_)
            | Self::Call(_) => 2,
            Self::Load(_) | Self::Store(_) | Self::Sense(_) | Self::Act(_) => 1,
            _ => 0,
        };
        1 + operand_len
    }

    /// Appends the bytes of this instruction to `out` (operands are big-endian).
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.push(self.opcode());
        match *self {
            Self::Const(k) => out.extend_from_slice(&k.to_be_bytes()),
            Self::PushI16(n) | Self::Jmp(n) | Self::Jz(n) | Self::Jnz(n) | Self::Call(n) => {
                out.extend_from_slice(&n.to_be_bytes());
            }
            Self::Load(k) | Self::Store(k) | Self::Sense(k) | Self::Act(k) => out.push(k),
            _ => {}
        }
    }
}

/// Encodes a list of instructions into code bytes. Handy for tests and hand-written agents.
pub fn encode_all(instructions: &[Instruction]) -> Vec<u8> {
    let mut code = Vec::new();
    for instruction in instructions {
        instruction.encode_into(&mut code);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use Instruction as I;

    /// One of each instruction, with operands that exercise sign and byte order.
    const SAMPLES: [Instruction; 43] = [
        I::Halt,
        I::Const(0x0102),
        I::PushI16(-11),
        I::Drop,
        I::Dup,
        I::Swap,
        I::Over,
        I::Rot,
        I::ConstX,
        I::Add,
        I::Sub,
        I::Mul,
        I::Div,
        I::Mod,
        I::Neg,
        I::Eq,
        I::Lt,
        I::Gt,
        I::Not,
        I::And,
        I::Or,
        I::Xor,
        I::Shl,
        I::Shr,
        I::BNot,
        I::Sar,
        I::LoadX,
        I::StoreX,
        I::Load(200),
        I::Store(255),
        I::Jmp(-32768),
        I::Jz(32767),
        I::Jnz(-1),
        I::JumpDest,
        I::JmpX,
        I::Call(9),
        I::Return,
        I::CallX,
        I::Sense(13),
        I::Act(3),
        I::Rand,
        I::Gas,
        I::Trace,
    ];

    #[test]
    fn exactly_43_bytes_are_opcodes() {
        let known = (0..=255u8)
            .filter(|&b| decode_at(&[b, 0, 0], 0).is_ok())
            .count();
        assert_eq!(known, 43);
    }

    #[test]
    fn decoding_and_encoding_agree_for_every_byte() {
        for byte in 0..=255u8 {
            if let Ok(instruction) = decode_at(&[byte, 0x12, 0x34], 0) {
                let mut bytes = Vec::new();
                instruction.encode_into(&mut bytes);
                assert_eq!(bytes[0], byte, "{instruction:?}");
                assert_eq!(bytes.len(), instruction.size(), "{instruction:?}");
            }
        }
    }

    #[test]
    fn every_instruction_survives_a_round_trip() {
        for instruction in SAMPLES {
            let bytes = encode_all(&[instruction]);
            assert_eq!(bytes.len(), instruction.size());
            assert_eq!(decode_at(&bytes, 0), Ok(instruction));
        }
    }

    #[test]
    fn operands_are_big_endian() {
        assert_eq!(decode_at(&[0x01, 0x01, 0x02], 0), Ok(I::Const(258)));
        assert_eq!(decode_at(&[0x02, 0xFF, 0xF5], 0), Ok(I::PushI16(-11)));
        assert_eq!(decode_at(&[0x32, 0xC8], 0), Ok(I::Load(200)));
    }

    #[test]
    fn pc_past_the_end() {
        assert_eq!(decode_at(&[0x00], 1), Err(DecodeError::PcOutOfRange));
        assert_eq!(
            decode_at(&[0x00], usize::MAX),
            Err(DecodeError::PcOutOfRange)
        );
    }

    #[test]
    fn unknown_opcode() {
        assert_eq!(decode_at(&[0x99], 0), Err(DecodeError::BadOpcode));
        assert_eq!(decode_at(&[0x09, 0x00], 0), Err(DecodeError::BadOpcode));
    }

    #[test]
    fn truncated_operand() {
        assert_eq!(decode_at(&[0x02, 0x00], 0), Err(DecodeError::Truncated));
        assert_eq!(decode_at(&[0x40], 0), Err(DecodeError::Truncated));
        assert_eq!(decode_at(&[0x60], 0), Err(DecodeError::Truncated));
    }

    #[test]
    fn decodes_from_the_middle_of_an_operand() {
        // PUSHI16 67 is 02 00 43: its last byte is a JUMPDEST when read on its own (A.3).
        assert_eq!(decode_at(&[0x02, 0x00, JUMPDEST], 2), Ok(I::JumpDest));
    }
}
