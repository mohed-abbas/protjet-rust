//! The verifier is sound: a program it accepts never raises a fault it excludes.
//!
//! Each accepted random program runs many times, with different memories, budgets and
//! host answers. If any run raises one of `EXCLUDED_FAULTS`, the verifier was wrong.

mod common;

use common::{RandomHost, Rng};
use vm::verify::{EXCLUDED_FAULTS, Reason};
use vm::{run_turn, verify};

fn cases() -> u64 {
    std::env::var("PBC_CASES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(3000)
}

#[test]
fn accepted_programs_never_raise_an_excluded_fault() {
    let mut accepted = 0;
    let mut rejected_for = std::collections::BTreeMap::new();
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let verified = match verify(&program) {
            Ok(verified) => verified,
            Err(rejection) => {
                let name = match rejection.reason {
                    Reason::MayFault(fault) => fault.as_str(),
                    Reason::UnknownTarget => "UnknownTarget",
                    Reason::TooComplex => "TooComplex",
                };
                *rejected_for.entry(name).or_insert(0) += 1;
                continue;
            }
        };
        accepted += 1;
        for run in 0..20 {
            let mut mem = rng.memory();
            let budget = rng.below(3000) as u16;
            let mut host = RandomHost::new(seed * 100 + run);
            let result = run_turn(&program, &mut mem, budget, &mut host).unwrap();
            if let Some(fault) = result.fault {
                assert!(
                    !EXCLUDED_FAULTS.contains(&fault),
                    "seed {seed}, run {run}: verified as {verified:?}, raised {fault}"
                );
            }
        }
    }
    println!(
        "accepted {accepted} of {}; rejected for {rejected_for:#?}",
        cases()
    );
    // The property is only meaningful if many programs get through.
    let n = cases() as usize;
    assert!(
        accepted * 10 >= n,
        "only {accepted} of {n} programs accepted"
    );
}

#[test]
fn rejected_programs_can_really_fault_sometimes() {
    // Not a proof of anything (the verifier may reject safe programs), but if no
    // rejected program ever faulted the way it was rejected for, the verifier would be
    // rejecting blindly.
    let mut confirmed = 0;
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let Err(rejection) = verify(&program) else {
            continue;
        };
        let Reason::MayFault(expected) = rejection.reason else {
            continue;
        };
        let mut mem = rng.memory();
        let result = run_turn(&program, &mut mem, 3000, &mut RandomHost::new(seed)).unwrap();
        if result.fault == Some(expected) {
            confirmed += 1;
        }
    }
    assert!(confirmed > 0, "no rejection was ever confirmed by a run");
}
