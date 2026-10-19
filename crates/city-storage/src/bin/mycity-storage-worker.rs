//! Explicit crash-test helper for laboratory worlds; never used by the simulator implicitly.
use mycity_core::transaction::{Action, Caller, Envelope, Resource};
use mycity_core::units::Money;
use mycity_storage::{Store, WritePoint};
use std::{
    env,
    io::{self, Write},
    path::Path,
};

const RULES: &str = "m0-storage-process-fixture-v1";
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err(
            "usage: mycity-storage-worker <init|commit|inspect> <world_dir> [crash_point]".into(),
        );
    }
    let dir = Path::new(&args[2]);
    if args[1] == "init" || args[1] == "init-regional" {
        let _ = Store::create(
            dir,
            if args[1] == "init-regional" {
                &include_bytes!("../../../../fixtures/m0-region-opening.json")[..]
            } else {
                &include_bytes!("../../../../fixtures/m0-e0-opening.json")[..]
            },
            vec![Resource::new(1, 10).map_err(err)?],
            RULES,
        )
        .map_err(err)?;
        return Ok(());
    }
    let mut store = Store::open(dir, RULES).map_err(err)?;
    if args[1] == "inspect" {
        let engine = store.engine().map_err(err)?;
        println!(
            "{}",
            serde_json::json!({"sequence":store.committed_sequence(),"state_sha256":engine.state_hash(),"world":engine.world().view(),"resources":engine.resources(),"journal":engine.journal()})
        );
        return Ok(());
    }
    if args[1] != "commit" {
        return Err("unknown_worker_command".into());
    }
    let world = store.engine().map_err(err)?.world();
    let cmd = Envelope {
        world: 1,
        branch: 1,
        operation_id: 1,
        expected_revision: 0,
        actions: if world.view().region.is_some() {
            vec![
                Action::RegionMigrate {
                    from: 1,
                    to: 2,
                    requested: 2000,
                    tolerance: 2,
                },
                Action::RegionAdvance { seconds: 30 },
                Action::RegionActivate { city: 1 },
            ]
        } else {
            vec![
                Action::Protect {
                    account: world.account_ref(2)?,
                    amount: Money::new(10000).map_err(err)?,
                },
                Action::Transfer {
                    from: world.account_ref(3)?,
                    to: world.account_ref(4)?,
                    amount: Money::new(10000).map_err(err)?,
                },
                Action::ReserveCapacity {
                    resource: 1,
                    units: 8,
                },
            ]
        },
    };
    let crash = args.get(3).map(String::as_str).unwrap_or("none");
    let outcome = store
        .execute_with_observer(Caller::System, cmd, |point: WritePoint| -> io::Result<()> {
            if format!("{point:?}") == crash {
                std::process::exit(97)
            }
            Ok(())
        })
        .map_err(err)?;
    println!("{}", serde_json::to_string(&outcome).map_err(err)?);
    io::stdout().flush().map_err(err)?;
    if crash == "Responded" {
        std::process::exit(97)
    }
    Ok(())
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(2)
    }
}
