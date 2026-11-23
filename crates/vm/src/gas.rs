//! The gas table of SPEC annex B (B.3, D.1).

use pbc::Instruction;

/// What `instruction` costs, charged before it runs (B.2 step 4).
///
/// The match is exhaustive on purpose: adding an instruction to `pbc` without giving it
/// a price here does not compile (journal D-007).
pub fn cost(instruction: &Instruction) -> u16 {
    use Instruction as I;
    match instruction {
        I::Halt => 0,
        I::Const(_)
        | I::PushI16(_)
        | I::Drop
        | I::Dup
        | I::Swap
        | I::Over
        | I::Rot
        | I::JumpDest
        | I::Trace => 1,
        I::ConstX
        | I::Add
        | I::Sub
        | I::Mul
        | I::Neg
        | I::Eq
        | I::Lt
        | I::Gt
        | I::Not
        | I::And
        | I::Or
        | I::BNot
        | I::Jmp(_)
        | I::Jz(_)
        | I::Jnz(_)
        | I::Gas => 2,
        I::Shl | I::Load(_) | I::JmpX => 3,
        I::Xor | I::LoadX | I::Store(_) | I::Return => 4,
        I::StoreX | I::Rand => 5,
        I::Shr | I::Sar | I::Call(_) => 6,
        I::Div | I::Mod | I::CallX => 7,
        I::Sense(_) => 8,
        I::Act(_) => 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gas column of B.3, opcode by opcode.
    const TABLE: [(u8, u16); 43] = [
        (0x00, 0),
        (0x01, 1),
        (0x02, 1),
        (0x03, 1),
        (0x04, 1),
        (0x05, 1),
        (0x06, 1),
        (0x07, 1),
        (0x08, 2),
        (0x10, 2),
        (0x11, 2),
        (0x12, 2),
        (0x13, 7),
        (0x14, 7),
        (0x15, 2),
        (0x20, 2),
        (0x21, 2),
        (0x22, 2),
        (0x23, 2),
        (0x24, 2),
        (0x25, 2),
        (0x26, 4),
        (0x27, 3),
        (0x28, 6),
        (0x29, 2),
        (0x2D, 6),
        (0x30, 4),
        (0x31, 5),
        (0x32, 3),
        (0x33, 4),
        (0x40, 2),
        (0x41, 2),
        (0x42, 2),
        (0x43, 1),
        (0x44, 3),
        (0x50, 6),
        (0x51, 4),
        (0x52, 7),
        (0x60, 8),
        (0x61, 12),
        (0x70, 5),
        (0x71, 2),
        (0xF0, 1),
    ];

    #[test]
    fn matches_the_spec_table() {
        for (opcode, gas) in TABLE {
            let instruction = pbc::decode_at(&[opcode, 0, 0], 0).unwrap();
            assert_eq!(cost(&instruction), gas, "{}", instruction.mnemonic());
        }
    }
}
