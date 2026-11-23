//! Properties of the engine, checked on thousands of random programs.
//!
//! The unit tests pin each rule on a chosen example. These check what must hold for
//! every program, memory, budget and host: the turn ends, never panics, never spends
//! more gas than its budget, and is deterministic (the property everything else in the
//! project relies on). Run with `PBC_CASES=100000 cargo test -p vm --test properties`
//! to search longer.

mod common;

use common::{RandomHost, Rng};
use vm::{Fault, Host, run_turn};

/// How many random cases each property checks.
fn cases() -> u64 {
    std::env::var("PBC_CASES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(3000)
}

#[test]
fn gas_never_exceeds_the_budget() {
    let mut reached_halt = 0;
    let mut long_turns = 0;
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let mut mem = rng.memory();
        let budget = rng.below(1500) as u16;
        let mut host = RandomHost::new(seed);
        let result =
            run_turn(&program, &mut mem, budget, &mut host).expect("this host never fails");

        assert!(result.gas_used <= budget, "seed {seed}: {result:?}");
        // TRACE costs 1 gas, so the trace never outgrows the gas used.
        assert!(
            result.trace.len() <= usize::from(result.gas_used),
            "seed {seed}"
        );
        // A program that runs out of gas has used its budget, give or take the cost of
        // the refused instruction (at most 12, for ACT).
        if result.fault == Some(Fault::OutOfGas) {
            assert!(budget - result.gas_used < 12, "seed {seed}: {result:?}");
        }
        reached_halt += usize::from(result.fault.is_none());
        long_turns += usize::from(result.gas_used >= 20);
    }
    // The generator must reach past the first few instructions, or this proves little.
    let n = cases() as usize;
    assert!(
        reached_halt * 20 >= n,
        "only {reached_halt} of {n} turns reached HALT"
    );
    assert!(
        long_turns * 5 >= n,
        "only {long_turns} of {n} turns used 20 gas"
    );
}

#[test]
fn a_turn_is_deterministic() {
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let start = rng.memory();
        let budget = rng.below(1500) as u16;

        let (mut mem_a, mut mem_b) = (start, start);
        let a = run_turn(&program, &mut mem_a, budget, &mut RandomHost::new(seed));
        let b = run_turn(&program, &mut mem_b, budget, &mut RandomHost::new(seed));
        assert_eq!(a, b, "seed {seed}");
        assert_eq!(mem_a, mem_b, "seed {seed}");
    }
}

#[test]
fn at_most_one_action_per_turn() {
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let mut mem = rng.memory();
        let mut host = RandomHost::new(seed);
        let result = run_turn(&program, &mut mem, 1000, &mut host).expect("this host never fails");

        assert!(host.acts <= 1, "seed {seed}: {} actions", host.acts);
        assert_eq!(
            result.act.is_some(),
            host.acts == 1,
            "seed {seed}: {result:?}"
        );
    }
}

#[test]
fn a_failing_host_stops_the_turn_cleanly() {
    let mut failures = 0;
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let mut mem = rng.memory();
        let mut host = RandomHost::new(seed);
        host.can_fail = true;
        // No panic, whatever the host does; an `Err` is the host's own error.
        if let Err(error) = run_turn(&program, &mut mem, 1000, &mut host) {
            assert_eq!(error, "client gone");
            failures += 1;
        }
    }
    assert!(
        failures > 0,
        "the host never failed: the property was not exercised"
    );
}

/// A program that never reaches a callback behaves the same with any host.
#[test]
fn the_host_only_matters_through_callbacks() {
    struct Refuse;
    impl Host for Refuse {
        type Error = ();
        fn sense(&mut self, _: u8) -> Result<vm::SenseAnswer, ()> {
            Err(())
        }
        fn act(&mut self, _: vm::Action, _: i64) -> Result<bool, ()> {
            Err(())
        }
        fn rand(&mut self) -> Result<i64, ()> {
            Err(())
        }
    }

    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let start = rng.memory();
        let (mut mem_a, mut mem_b) = (start, start);
        // If the refusing host was never asked, the turn must match a run with a real host.
        if let Ok(a) = run_turn(&program, &mut mem_a, 1000, &mut Refuse) {
            let b = run_turn(&program, &mut mem_b, 1000, &mut RandomHost::new(seed));
            assert_eq!(Ok(a), b, "seed {seed}");
            assert_eq!(mem_a, mem_b, "seed {seed}");
        }
    }
}

/// The properties above are only as good as the programs they see: check that random
/// programs reach `HALT` and every runtime fault. Run with `--nocapture` to see the spread.
#[test]
fn the_generator_reaches_every_outcome() {
    let mut seen = std::collections::BTreeMap::new();
    for seed in 0..cases() {
        let mut rng = Rng::new(seed);
        let program = common::program(&mut rng);
        let mut mem = rng.memory();
        let result = run_turn(&program, &mut mem, 1000, &mut RandomHost::new(seed)).unwrap();
        let name = result.fault.map_or("HALT", Fault::as_str);
        *seen.entry(name).or_insert(0) += 1;
    }
    println!("{seen:#?}");
    for fault in Fault::ALL {
        assert!(
            seen.contains_key(fault.as_str()),
            "no random turn ended in {fault}"
        );
    }
    assert!(seen.contains_key("HALT"), "no random turn reached HALT");
}
