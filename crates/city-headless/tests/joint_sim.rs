//! Fast process integration only. The >=1800s gate is deliberately not claimed here.
#[path = "support/lifecycle_artifacts.rs"]
mod lifecycle_artifacts;
use mycity_core::{
    boot::{Command as B, Facility},
    schedule::Phase,
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use mycity_headless::{
    joint_reference::{self as joint, Reference},
    sim_client::Client,
    sim_host::Command,
};
use serde_json::Value;
use std::{
    time::{Duration, Instant},
};
struct Run {
    client: Client,
    reference: Reference,
    id: u64,
}
impl Run {
    fn call(&mut self, command: Command) -> Value {
        self.id += 1;
        self.client.submit(self.id, command).unwrap();
        for _ in 0..24 {
            let event = self.client.next(Duration::from_secs(10)).unwrap();
            if event.kind == "result" {
                assert!(event.payload.get("Err").is_none(), "{event:?}");
                return event.payload["Ok"].clone();
            }
        }
        panic!("result absent");
    }
    fn raw(&mut self, actions: Vec<Action>) {
        let s = self.call(Command::Status);
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: s["sequence"].as_u64().unwrap() + 1,
            expected_revision: s["revision"].as_u64().unwrap(),
            actions,
        };
        let result = self.call(Command::Execute { envelope });
        assert!(result["model"].get("Ok").is_some(), "{result}");
    }
    fn execute(&mut self, actions: Vec<Action>) {
        self.raw(actions.clone());
        self.reference.execute(actions).unwrap();
    }
    fn drive(&mut self, target: u64) {
        self.reference.drive(target).unwrap();
        for _ in 0..10000 {
            let state = self.call(Command::Status);
            if state["timeline"]["target"].is_null() && state["timeline"]["last_complete"] == target
            {
                assert_eq!(
                    state["business_sha256"],
                    joint::business_digest(&self.reference.engine).unwrap()
                );
                assert_eq!(
                    state["background"]["state_sha256"],
                    self.reference.summary(&joint::policy().unwrap()).unwrap()["air_sha256"]
                );
                assert_eq!(state["background"]["version"], self.reference.windows);
                return;
            }
            let phase = serde_json::from_value::<Option<Phase>>(state["timeline"]["phase"].clone())
                .unwrap();
            if phase == Some(Phase::P4) && !state["background"]["pending"].is_null() {
                let job = state["background"]["pending"]["id"].as_u64().unwrap();
                self.call(Command::RunRequired { job });
            } else {
                self.raw(vec![Action::Timeline {
                    command: T::Advance {
                        target,
                        limit: 8192,
                    },
                }]);
            }
        }
        panic!("joint drive limit");
    }
}
#[test]
fn seven_day_independent_sim_and_direct_cpu_match_without_endurance_claim() {
    let started = Instant::now();
    let p = joint::policy().unwrap();
    let root = lifecycle_artifacts::root().join(format!("mycity-joint-smoke-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut client = Client::new();
    client.start_joint(
        lifecycle_artifacts::exe(),
        &root.join("world"),
        true,
        false,
    )
    .unwrap();
    assert_eq!(client.next(Duration::from_secs(10)).unwrap().kind, "ready");
    let mut run = Run {
        client,
        reference: Reference::new().unwrap(),
        id: 0,
    };
    for action in joint::setup(&p).unwrap() {
        run.execute(vec![action]);
    }
    run.drive(60);
    run.execute(vec![Action::Boot {
        command: B::Start {
            facility: Facility::Generator,
            reverse_request: false,
        },
    }]);
    run.drive(120);
    let mut tax_shortfall = false;
    for day in 1..=7 {
        run.drive(day * 86400);
        if day == 2 {
            run.execute(vec![Action::RegionActivate { city: 2 }]);
        }
        if day == 3 {
            run.execute(vec![Action::RegionActivate { city: 1 }]);
        }
        if day == 4 {
            let summary = run.reference.summary(&p).unwrap();
            tax_shortfall = summary["tax"]["arrears_cents"].as_i64().unwrap() > 0;
            run.execute(joint::loan(&p).unwrap());
            run.execute(vec![Action::RegionSettleDebt {
                obligation: p.tax.obligation_id,
            }]);
        }
    }
    assert!(tax_shortfall);
    let final_state = run.call(Command::Status);
    let reference = run.reference.summary(&p).unwrap();
    assert_eq!(final_state["runtime"]["payroll"], reference["payroll"]);
    assert_eq!(final_state["runtime"]["claims"], reference["claims"]);
    assert_eq!(final_state["runtime"]["accounts"], reference["accounts"]);
    run.call(Command::Checkpoint);
    run.call(Command::Stop);
    run.client.stop().unwrap();
    lifecycle_artifacts::cleanup(root).unwrap();
    println!("W4_FAST_JOINT wall_seconds={} windows={} all_business_and_air_exact=true endurance_1800_seconds=NOT_RUN whole_group_resources=NOT_RUN",started.elapsed().as_secs_f64(),run.reference.windows);
}
