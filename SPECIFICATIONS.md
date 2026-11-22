# Palestre: consolidated specifications

Sources:
- `SyllabusDuProjet-1.pdf` (school syllabus, 2 pages)
- https://github.com/haveneer-training/palestre (commit `5154ede`, 2026-10-08):
  `README.md` (the brief), `SPEC-PBC-v1.md` (normative spec v1.9.0, **wins on any conflict**), `corpus/` (public tests)
- Local copy of that repo: `palestre-spec/` (a git clone; run `git -C palestre-spec pull` to get spec updates)

---

## 0. Administrative (syllabus)

| Item | Value |
|---|---|
| Course | Langage Rust, "La palestre", 5IW1 + 5IBC, 2026-2027 |
| Teacher | Pascal HAVE |
| Group size | 3 to 4, free choice |
| Workload | 20 h/student (syllabus), **30 h/student (README)**, they disagree |
| Subject | Imposed: implement the Palestre VM spec with remote execution, plus match capabilities (free spec) |
| Final deliverable | **Wednesday 06/01/2027, 9:00** |
| Code freeze | Last commit counted = day before the defense |
| Defense | 20 min per group, closed session. Slides + demo + **live code changes requested by the examiner** |
| Tools | Rust |

## 1. What to build

A **deterministic stack VM** for the `PBC` bytecode, plus a **remote executor** over TCP. Every group implements the same spec: your bytecode must run on other groups' machines, and your server must give the same result, trace, faults and gas, bit for bit.

Two mandatory components:
1. **`pbc` (format)**: loader, decoder and validator for `.pbc` files. Zero panics, no unbounded allocation.
2. **`exec` (remote executor)**: execution engine plus a TCP server for the Annex K protocol. SENSE/ACT/RAND are delegated to the client through callbacks.

### Mandatory CLI
```
palestre exec --listen <addr>      # e.g. 127.0.0.1:9000, no default port imposed
```
A sequential server (one connection at a time, single thread) is compliant. Concurrency is a bonus.

### Suggested tooling (optional, bonus if done well)
```
palestre asm agent.asm -o agent.pbc
palestre disasm agent.pbc
palestre run agent.pbc
```
Free assembler syntax. Key property test: `asm(disasm(p)) == p`.

## 2. The six invariants (you must be able to *demonstrate* them)

1. **Determinism**: same bytecode + initial state + budget + environment answers ⇒ same trace, final state **and gas**.
2. **Termination**: every run stops. Out of budget ⇒ `OutOfGas`.
3. **Turn atomicity**: a fault rolls back the agent and world state for the turn. Gas used, trace and the attempted action are **not** rolled back (E.2).
4. **Isolation**: no access outside memory `0..=255`. No overflow ever panics.
5. **No panic**: any input, including random or adversarial bytes, gives a typed error. No panic, no native stack overflow, no infinite loop. Tested by fuzzing.
6. **Announced cost**: gas depends only on the executed instruction sequence, never on time, host or memory.

## 3. Technical constraints

- Rust edition 2021 or 2024.
- `#![forbid(unsafe_code)]` on the engine and decoder, no exceptions.
- **No async**: no `async`/`await`, `tokio` or `futures`.
- Concurrency optional; if used, **`std` only** (`thread::spawn`, `thread::scope`, `mpsc`, `Mutex`), with no lock inside the execution loop.
- `unwrap()`/`expect()`/`panic!()`: **strictly forbidden** in the engine and decoder, tolerated only in tests.
- **No allocation inside a turn's execution loop.** Fixed-size stack, memory and call stack, pre-sized trace buffer.
- `cargo clippy -- -D warnings` and `cargo fmt` must be clean.
- **Allowed crates** (anything else needs prior approval): `sha2`, `thiserror`, `clap`, `serde`, `serde_json` (never in the VM inner loop), `rand` (tests and map generation), `proptest`, `criterion`, `arbitrary`, `cargo-fuzz`.
- **Forbidden**: third-party VMs and interpreters (`wasmi`, `wasmtime`, `rhai`, `revm`, `mlua`, …), parser generators (`pest`, `nom`, `lalrpop`, `chumsky`). Parsers are hand-written. No JavaScript or TypeScript, even for a web viewer (use Rust compiled to WASM).
- The spec version string must live in **one single place** in the code: a v2.0.0 spec will be revealed at the defense.

## 4. Normative spec PBC v1 (v1.9.0)

### 4.0 General conventions
- All multi-byte integers are **big-endian** (file and network).
- Single value type: `i64`, two's complement. All arithmetic is **wrapping**: overflow is never a fault.
- Division truncates toward zero, remainder has the dividend's sign (Rust semantics). `i64::MIN / -1 = i64::MIN`, `i64::MIN % -1 = 0`.
- Hash = SHA-256, lowercase hex, 64 chars.

### 4.1 `.pbc` file format (Annex A)

| Offset | Size | Field | Constraint |
|---|---|---|---|
| 0 | 4 | `magic` | `50 42 43 31` ("PBC1") |
| 4 | 1 | `version` | `0x01` |
| 5 | 1 | `flags` | `0x00` |
| 6 | 2 | `const_count` | u16, `0..=4095` |
| 8 | 8 × const_count | `constants` | i64 values |
| — | 4 | `code_len` | u32, `1..=65535` |
| — | code_len | `code` | instruction bytes |

- No checksum. The file must be **exactly** the size implied by its fields: a missing or extra byte is an error.
- Any violation ⇒ `Fault::BadHeader` (a load error, never a runtime fault). No panic, no unbounded allocation, no out-of-bounds read.
- Max file size: `8 + 8×4095 + 4 + 65535 = 98307` bytes.

### 4.2 Machine (B.1)

| Resource | Size | Overflow |
|---|---|---|
| Data stack | 64 × i64 | `StackOverflow` |
| Call stack | 16 return addresses | `CallStackOverflow` |
| Memory per agent | 256 × i64, indices `0..=255` | `OutOfBounds` |
| Gas per turn | `gas_budget`, usually 1000, range `1..=65535` | `OutOfGas` |

Memory **persists** between turns. Data stack and call stack are **cleared** each turn, and `pc` restarts at 0.

### 4.3 Execution cycle (B.2), order is normative
1. `pc >= code_len` ⇒ `PcOutOfRange`
2. Decode the opcode. Unknown ⇒ `BadOpcode`
3. Operand runs past `code_len` ⇒ `Truncated`
4. **Charge gas.** Not enough ⇒ `OutOfGas`, instruction not executed and **not charged**
5. Execute. A fault here has **already** paid its gas
6. `pc += instruction size`, unless a jump was taken

Example: a PUSH on a full stack with no gas left ⇒ `OutOfGas`, not `StackOverflow`.

### 4.4 Instruction set (B.3)

Stack notation `a b -- r`: **`b` is the top** before the operation, `r` is the top after. Mnemonics are indicative; only byte, operand, gas, stack effect and semantics are normative.

| Op | Mnemonic | Operand | Gas | Stack | Semantics |
|---|---|---|---|---|---|
| `0x00` | HALT | — | 0 | `--` | normal end of turn |
| `0x01` | CONST k | u16 | 1 | `-- v` | `constants[k]`; `k >= const_count` ⇒ BadConst |
| `0x02` | PUSHI16 n | i16 | 1 | `-- v` | sign-extended |
| `0x03` | DROP | — | 1 | `a --` | |
| `0x04` | DUP | — | 1 | `a -- a a` | |
| `0x05` | SWAP | — | 1 | `a b -- b a` | |
| `0x06` | OVER | — | 1 | `a b -- a b a` | |
| `0x07` | ROT | — | 1 | `a b c -- b c a` | |
| `0x08` | CONSTX | — | 2 | `k -- v` | k popped; `k<0` or `>= const_count` ⇒ BadConst |
| `0x10` | ADD | — | 2 | `a b -- a+b` | wrapping |
| `0x11` | SUB | — | 2 | `a b -- a-b` | wrapping |
| `0x12` | MUL | — | 2 | `a b -- a*b` | wrapping |
| `0x13` | DIV | — | 7 | `a b -- a/b` | `b==0` ⇒ DivByZero |
| `0x14` | MOD | — | 7 | `a b -- a%b` | `b==0` ⇒ DivByZero |
| `0x15` | NEG | — | 2 | `a -- -a` | wrapping |
| `0x20` | EQ | — | 2 | `a b -- 0/1` | |
| `0x21` | LT | — | 2 | `a b -- 0/1` | signed `a < b` |
| `0x22` | GT | — | 2 | `a b -- 0/1` | signed `a > b` |
| `0x23` | NOT | — | 2 | `a -- 0/1` | **logical**: 1 if a==0 |
| `0x24` | AND | — | 2 | `a b -- a&b` | bitwise |
| `0x25` | OR | — | 2 | `a b -- a\|b` | bitwise |
| `0x26` | XOR | — | 4 | `a b -- a^b` | bitwise |
| `0x27` | SHL | — | 3 | `a n -- v` | logical left (B.6) |
| `0x28` | SHR | — | 6 | `a n -- v` | **logical** right (B.6) |
| `0x29` | BNOT | — | 2 | `a -- !a` | one's complement (bitwise) |
| `0x2D` | SAR | — | 6 | `a n -- v` | **arithmetic** right (B.6) |
| `0x30` | LOADX | — | 4 | `addr -- v` | `mem[addr]` |
| `0x31` | STOREX | — | 5 | `v addr --` | `mem[addr] = v` |
| `0x32` | LOAD k | u8 | 3 | `-- v` | `mem[k]` (OutOfBounds unreachable) |
| `0x33` | STORE k | u8 | 4 | `v --` | `mem[k] = v` |
| `0x40` | JMP d | i16 | 2 | `--` | relative jump |
| `0x41` | JZ d | i16 | 2 | `c --` | jump if `c == 0` |
| `0x42` | JNZ d | i16 | 2 | `c --` | jump if `c != 0` |
| `0x43` | JUMPDEST | — | 1 | `--` | valid jump target marker |
| `0x44` | JMPX | — | 3 | `t --` | jump to **absolute** popped target |
| `0x50` | CALL d | i16 | 6 | `--` | push return addr, relative jump |
| `0x51` | RETURN | — | 4 | `--` | pop return addr; empty ⇒ CallStackUnderflow |
| `0x52` | CALLX | — | 7 | `t --` | push return addr, jump to **absolute** popped target |
| `0x60` | SENSE k | u8 | 8 | `-- v` | sensor k (E.3); unknown ⇒ BadSensor |
| `0x61` | ACT k | u8 | 12 | `arg -- ok` | action k (E.4); unknown ⇒ BadAction; 2nd ACT in turn ⇒ AlreadyActed |
| `0x70` | RAND | — | 5 | `-- v` | random value supplied by host/client |
| `0x71` | GAS | — | 2 | `-- g` | gas left **after** its own cost (budget 1000, GAS at pc=0 ⇒ 998) |
| `0xF0` | TRACE | — | 1 | `v --` | append v to the turn's trace |

Any other byte ⇒ `BadOpcode`. Since TRACE costs 1, a turn's trace holds at most `gas_budget` entries, so size the buffer once.

### 4.5 Jumps (B.4)
- `target = pc + instruction_size + d` (relative to the **next** instruction).
- Checked in order: (1) target in `[0, code_len)`, (2) `code[target] == 0x43`. Either fails ⇒ `BadJump`.
- Only the **single target byte** is checked: landing mid-operand on a `0x43` byte is legal, and decoding resumes there.
- Applies to JMP, JZ/JNZ (only when the jump is taken), CALL, JMPX, CALLX. **Not** to RETURN.
- JMPX/CALLX: absolute popped target, same two checks; negative or too large ⇒ `BadJump`.

### 4.6 The 16 faults (B.5), exact strings, closed list
`BadHeader`, `BadConst`, `BadOpcode`, `Truncated`, `PcOutOfRange`, `BadJump`, `StackUnderflow`, `StackOverflow`, `CallStackOverflow`, `CallStackUnderflow`, `OutOfGas`, `DivByZero`, `OutOfBounds`, `BadSensor`, `BadAction`, `AlreadyActed`.
`BadHeader` is load-only and never appears in a running turn.

### 4.7 Bitwise and shifts (B.6)
- AND/OR/XOR/BNOT work on the 64-bit two's complement pattern.
- `SHL`/`SHR` are **logical** (as u64): `-1 SHR 1 = 9223372036854775807`.
- `SAR` is **arithmetic**: `-1 SAR 1 = -1`, `-7 SAR 1 = -4`.
- `n` is on top (`a n --`).
- Count outside `0..=63` (including negative) is **never a fault**: SHL/SHR ⇒ `0`; SAR ⇒ `0` if `a >= 0`, `-1` if `a < 0`.
- `SAR != DIV` by a power of two: `-7 DIV 2 = -3`, `-7 SAR 1 = -4`.
- `NOT 0 = 1`, `BNOT 0 = -1`, `BNOT 5 = -6`.

### 4.8 Gas (Annex D)
The table above, applied as-is (deliberately non-monotonic). Budget `1..=65535`, usually 1000. Unused gas does **not** carry over. HALT costs 0.

## 5. World / match rules (Annex E)

### E.2 One turn (turn `t` from 1; living agents in **ascending id** order)
1. Snapshot agent and world.
2. Clear stacks, `pc = 0`, gas = `gas_budget`.
3. Run until HALT or fault.
4. **On fault**: restore agent and world from the snapshot. **Not** restored: gas used, trace, the attempted action. Log the fault.
5. `energy -= 1` (even on fault).
6. `energy <= 0` ⇒ `alive = 0`, carried resources dropped on the current cell.

### E.3 Sensors (`SENSE k`)

| k | Value |
|---|---|
| 0 | agent x |
| 1 | agent y |
| 2 | energy |
| 3 | carried |
| 4 | resource amount on current cell |
| 5 | current turn number |
| 6 | agent id |
| 7 | Manhattan distance to the other agent (ignores walls); `-1` if it's dead |
| 8 | blocked-neighbours mask (out of grid or wall) |
| 9 | neighbours-with-resource mask (presence, not quantity) |
| 10 | `x_max` (max index, not width) |
| 11 | `y_max` |
| 12 | `turns_max` |
| 13 | neighbours occupied by a **living** agent mask |
| other | `BadSensor` |

Masks: bit0 = north (y-1, value 1), bit1 = east (x+1, 2), bit2 = south (y+1, 4), bit3 = west (x-1, 8). The NW corner of a wall-free board ⇒ `SENSE 8 = 9`. `(SENSE 8 | SENSE 13)` bit at 0 ⇒ MOVE in that direction succeeds.

### E.4 Actions (`ACT k`, pops arg, pushes `ok` = 1/0)
One action per turn, successful **or failed**. A second ACT ⇒ `AlreadyActed`.

| k | Action | Arg | Effect |
|---|---|---|---|
| 0 | MOVE | 0=N, 1=E, 2=S, 3=W; other ⇒ fail | one cell. Fails if out of grid, wall, or living agent. Success ⇒ extra `energy -= 1` |
| 1 | TAKE | ignored | take `min(cell_resource, 5)` into carried |
| 2 | DROP | qty | drop `min(arg, carried)`; `arg < 0` ⇒ fail |
| 3 | EAT | qty | `k = min(arg, carried)`: `carried -= k`, `energy += eat_rate × k` (default `eat_rate = 10`); `arg < 0` ⇒ fail |
| other | — | — | `BadAction` (no action recorded) |

A failed action is **not** a fault: the turn continues. ACT can never cause StackOverflow (it pops before pushing).

### E.5 End of game
- Stops at `turns_max` (default 500) or when at most one agent is alive. Checked **after** all agents have played the turn.
- Winner: highest `carried`, then highest `energy`, otherwise **draw** (`winner: null`). Id never breaks ties.

Not specified (free design for your arena): grid size, wall/resource placement, initial energy and positions (there is no E.1 in the spec).

## 6. Versioning (Annex H)
- `MAJOR.MINOR.PATCH`. Artifacts compare only if MAJOR and MINOR match. Tools **must** refuse cross-minor comparisons and name both versions.
- **v2.0.0 will be revealed at the defense** (a few changed or added elements). Keep changes local; the version number lives in one place.

## 7. Remote execution protocol (Annex K)

### Model
The server runs the program but has **no world**. SENSE/ACT/RAND become callbacks to the client. Result = f(program, initial mem, budget, callback answers) only. No clock, no server RNG, no server state. The **callback transcript is normative** (same SENSE/ACT/RAND sequence, same order).

### Framing (K.2)

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | `type` (`0x11..=0x1D`) |
| 1 | 4 | `len` u32 **big-endian** |
| 5 | len | payload (JSON, or raw bytes for SUBMIT) |

- Max `len`: client→server **98307**, server→client **4194304**. Oversized ⇒ reject **before allocating**.
- Unknown `type` ⇒ `ERROR bad_frame` then close.

### Messages (K.3)

| Type | Name | Direction | Payload |
|---|---|---|---|
| `0x11` | OPEN | C→S | `{"proto":1,"client":"name"}` (client optional) |
| `0x12` | READY | S→C | `{"proto":1,"spec":"PBC1","spec_version":"1.9.0","sessions_max":16}` |
| `0x13` | SUBMIT | C→S | raw `.pbc` bytes |
| `0x14` | SESSION | S→C | `{"session":"1","pbc_sha256":"…","code_len":42}` |
| `0x15` | EXEC | C→S | `{"session":"1","mem":[256 ints],"budget":1000}` |
| `0x16` | SENSE | S→C | `{"k":7}` |
| `0x17` | SENSED | C→S | `{"value":-1}` **or** `{"fault":"BadSensor"}` |
| `0x18` | ACT | S→C | `{"kind":"MOVE","arg":0}` |
| `0x19` | ACTED | C→S | `{"ok":true}` |
| `0x1A` | RAND | S→C | `{}` |
| `0x1B` | DREW | C→S | `{"value":42}` |
| `0x1C` | RESULT | S→C | `{"mem":[256],"gas_used":137,"fault":null,"act":{"kind","arg","ok"}\|null,"trace":[…]}` |
| `0x1D` | ERROR | S→C | `{"code":"bad_frame","message":"…"}` |

- `fault` is a B.5 string or `null`. A SENSED naming a load fault (e.g. `BadHeader`) ⇒ `bad_frame`.
- `act` is `null` if no action was attempted; an ACT raising `BadAction` records none.
- The server **must not** refuse a connection because of a different spec version. It announces one version.

### Sessions and state machine (K.4)
- `session`: string, 1–64 chars from `[A-Za-z0-9_-]`, compared byte for byte; otherwise `bad_frame`. Opaque: the client sends it back verbatim. The server may use a counter, a hash, a token…
- `pbc_sha256` / `code_len` are conformance checksums, not the id.
- `Greeting --OPEN--> Idle <-> Executing`
  - Greeting accepts only OPEN (→ READY).
  - Idle: SUBMIT (→ SESSION or `ERROR bad_program`), EXEC (→ Executing).
  - Executing accepts only the exact reply to the pending callback; anything else ⇒ `ERROR unexpected_message` + close.
- Sessions live at least until the connection closes; EXEC doesn't destroy them (re-runnable). No pipelining. No server-side memory persistence between EXECs.
- RESULT is only sent if all callbacks succeeded; a failed callback ⇒ the connection closes.

### Error codes (K.8)

| code | Cause | Closes connection? |
|---|---|---|
| `bad_frame` | bad len, unknown type, invalid JSON, malformed session | yes |
| `bad_proto` | unsupported protocol version | yes |
| `bad_program` | SUBMIT bytes fail the A.1 loader | yes |
| `too_many_sessions` | `sessions_max` reached | **no** |
| `no_such_session` | EXEC on an unknown session | **no** |
| `bad_memory` | `mem` isn't exactly 256 values | yes |
| `bad_budget` | budget 0 or outside `1..=65535` | yes |
| `unexpected_message` | valid frame, wrong state | yes |

A frame truncated by a disconnect ⇒ just close, no ERROR. No auth, no encryption, no explicit session release, no step-by-step observation (no stack in RESULT).

## 8. Test corpus (`corpus/`)
- **65 public cases**: 54 `edge/` + 11 `nominal/`. **Hidden cases are added on defense day.**
- Each case: `case.toml` (description, category, clause, one or more `[[exec]]` with `budget`, sparse `mem`, scripted `answers`; or `load_error`), `program.pbc`, `expected.json` (spec_version, per-exec `calls` transcript + `result`).
- Answers keys: `sense = v`, `sense_fault = "BadSensor"`, `act = true/false`, `rand = v`.
- `result.mem` is the memory as the **machine** left it: on a fault, earlier writes stay (the E.2 rollback is the arena's job, not the machine's).
- Compare in this order, stopping at the first difference: callbacks → `gas_used` → `fault` → `act` → `trace` → `mem`.
- Categories: header, decode, stack, arith, gas, jump, call, fault, act, bitwise, degenerate.

## 9. Grading

### MVP = 10/20 (required to pass)
1. Robust PBC loader/validator: rejects any truncated or corrupt binary with a typed error, zero panics.
2. `cargo test` covering the key instructions: CONST, PUSHI16, DROP, DUP, SWAP, ADD, SUB, MUL, DIV, EQ, LT, GT, LOADX/STOREX or LOAD/STORE, JMP/JZ/JNZ, deterministic gas.
3. A **match demo between two simple `.pbc` agents** (e.g. `idler` vs a moving or harvesting agent): turns, board update, gas count, actions, termination.
4. A clear CLI command or short script so the jury can replay the match and the tests.

### Bands
- **16–20**: exceptional design, robustness proven by fuzzing, ambitious bonuses working and documented in ADRs.
- **13–15**: complete and conformant (loader, VM, Annex K server), solid tests, complete journal, smooth defense.
- **10–12**: MVP reached.
- **5–9**: MVP incomplete or unstable, poor errors or tests, shallow ADRs.
- **0–4**: doesn't compile, `unsafe`/`async`/`panic!`, invariant violations, or AI-produced without understanding.

### Five assessment dimensions
1. Architecture (clear `pbc` / engine / `exec` split, narrow public API, invalid states unrepresentable, traits and generics).
2. Conformance and determinism (B.3, B.2, B.5, Annex K, cross-group matrix, six invariants).
3. Rust quality and tests (Result/Option, no panic/unwrap/unsafe/async, unit + proptest + fuzzing, clippy + fmt).
4. Traceability (ADR journal, critical distance from AI, balanced git history).
5. Defense and live coding.

## 10. ADR decision journal (`journal/`), heavily weighted
- One file per decision: `journal/D-001-structure-vm.md` …
- Header: id, date, authors, status (`proposée` / `adoptée` / `révisée` / `abandonnée`), related commits.
- **Five required sections**: Context · Options considered (≥ 2) · What the AI assistant proposed (faithfully) · What we decided and why · What would make us change our mind.
- **At least 10 entries**, spread over components and time, committed **before or with** the code (retroactive bulk journals aren't credible).
- Revise entries rather than rewriting them; failures and abandoned options belong there.
- **Every claimed bonus needs its own ADR** (need, options, AI proposal, trade-offs, tests), or it earns nothing.
- AI assistance is allowed without restriction, but you answer for every line.

## 11. Defense format
- The jury draws an ADR at random: summarize it without notes, show the code, explain (or start implementing) the rejected alternative, prove the decision predates the code in `git log`.
- Each member tells **what didn't work** (bugs, abandoned options, wrong AI suggestions, J4 divergences), backed by the repo.
- Question hat: code navigation without AI (**failure is eliminatory**), new test case or malformed frame, **spec v2** change (new instruction or gas rule), interoperability divergence, network/concurrency.
- **MOB programming**: one keyboard, `cargo test` green at the end. Graded on method and on reading borrow-checker errors (not random `clone()`).
- The oral is a **malus regulator**. AI clause: down to **0/20** if the team can't defend or modify its code.
- Git: named commits, balanced real contribution from every member.

## 12. Milestones (indicative)

| | Expected | Check |
|---|---|---|
| J1 | Repo, CI, PBC loader, binary validation, instruction decoder | Loader/decoder unit tests |
| J2 | Full engine: stack, arithmetic, gas, jumps, calls, typed faults | Instruction, gas and 16-fault tests |
| J3 | Remote executor: TCP server for Annex K | Sessions, executions, SENSE/ACT/RAND callbacks |
| J4 | **Interop day in class (December)**: cross-testing clients and servers between groups (not graded) | Cross exchanges |
| J5 | Consolidated tests, proptest, fuzzing, architecture docs | Final test suite and docs |
| Freeze | Last commit = the day before the defense | `git log` |

## 13. Bonuses (16–20 band, each needs a dedicated ADR)
- Text assembler + disassembler with the `asm(disasm(p)) == p` property test.
- Full arena (all of Annex E) + tournament simulation.
- Concurrent server (std only, no shared state between connections; a slow or killed client blocks no one). Proof: callback transcripts and RESULTs byte-identical to the sequential server.
- **Static bytecode verifier**: before execution, no jump lands out of bounds or on an invalid target, and max stack height is bounded. Described as the richest bonus.
- Differential fuzzing against another group's executor.
- High-level source language + hand-written compiler to PBC.
- Compiler optimizations (constant propagation, peephole, dead code), proven equivalent by differential tests.
- VM optimizations (threaded dispatch, super-instructions) with `criterion` benchmarks, reported even if the gain is negative.
- Visual replay (TUI / native / SVG / WASM web, no JS/TS, library needs approval). Described as a modest bonus.
- `no_std` (alloc only) target or a WebAssembly port.
