# Palestre

A deterministic stack VM for the PBC v1 bytecode, with a remote executor (ESGI 5IW/5IBC Rust project).
The specification is in `palestre-spec/` (see `SPECIFICATIONS.md` for a summary).

## Layout

| Path | Role |
|---|---|
| `crates/pbc` | `.pbc` format: loading, validation, decoding (SPEC annex A, B.3) |
| `crates/vm` | Execution engine: stack, memory, gas, faults (annex B, D) |
| `crates/exec` | Remote execution server (annex K) |
| `crates/arena` | Local world and match loop (annex E) |
| `crates/palestre` | The `palestre` binary |
| `journal/` | Architecture decision records (`D-NNN-*.md`) |

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p palestre -- exec --listen 127.0.0.1:9000
```
