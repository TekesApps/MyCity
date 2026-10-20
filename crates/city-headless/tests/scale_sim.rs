use mycity_core::transaction::Envelope;
use mycity_headless::{
    scale_reference::{self, Step},
    sim_client::Client,
    sim_host::Command,
};
use serde_json::Value;
use std::{path::Path, time::Duration};
fn call(c: &mut Client, id: &mut u64, command: Command) -> Value {
    *id += 1;
    c.submit(*id, command).unwrap();
    let e = c.next(Duration::from_secs(15)).unwrap();
    assert_eq!(e.kind, "result");
    assert!(e.payload.get("Ok").is_some(), "{e:?}");
    e.payload["Ok"].clone()
}
#[test]
fn frozen_input_walk_preserves_every_hour_special_target_and_startup_count() {
    let mut targets = Vec::new();
    let mut actions = 0;
    let mut hours = 0;
    let mut initialized_at = 0;
    scale_reference::walk(|step| {
        match step {
            Step::Action(_) => actions += 1,
            Step::Drive(t) => targets.push(t),
            Step::HourComplete => hours += 1,
            Step::Initialized => initialized_at = actions,
            Step::PeakShipments => (),
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(initialized_at, 88);
    assert_eq!(actions, 115);
    assert_eq!(hours, 720);
    assert_eq!(targets.len(), 723);
    assert_eq!(&targets[..3], &[60, 180, 3600]);
    assert!(targets.contains(&(15 * 86400 + 120)));
    assert_eq!(targets.last(), Some(&(30 * 86400)));
    for hour in 1..=720 {
        assert_eq!(targets.iter().filter(|&&t| t == hour * 3600).count(), 1);
    }
}
#[test]
fn named_profiles_have_frozen_counts_v2_identity_and_read_only_bounded_progress() {
    use scale_reference::Tier;
    for tier in [Tier::Small, Tier::Medium, Tier::Full] {
        let (accounts, _, payroll, claims) = tier.counts();
        let dir = std::env::temp_dir().join(format!(
            "mycity-scale-profile-{}-{}",
            std::process::id(),
            tier.name()
        ));
        std::fs::create_dir(&dir).unwrap();
        let world = dir.join("world");
        let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
        let mut c = Client::spawn_scale(exe, &world, true, false, tier).unwrap();
        let ready = c.next(Duration::from_secs(15)).unwrap();
        assert_eq!(ready.kind, "ready");
        assert_eq!(
            ready.payload["runtime"]["accounts"]
                .as_array()
                .unwrap()
                .len(),
            accounts
        );
        assert_eq!(
            ready.payload["runtime"]["resources"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        let mut id = 0;
        let policy = serde_json::from_slice(tier.policy()).unwrap();
        let mut first = None;
        for action in scale_reference::startup_actions(&policy).unwrap() {
            let s = call(&mut c, &mut id, Command::Status);
            let env = Envelope {
                world: 1,
                branch: 1,
                operation_id: s["sequence"].as_u64().unwrap() + 1,
                expected_revision: s["revision"].as_u64().unwrap(),
                actions: vec![action],
            };
            let result = call(
                &mut c,
                &mut id,
                Command::Execute {
                    envelope: env.clone(),
                },
            );
            assert!(result["model"].get("Ok").is_some(), "{result}");
            if first.is_none() {
                first = Some(env);
            }
        }
        let s = call(&mut c, &mut id, Command::Status);
        assert_eq!(s["runtime"]["claim_count"], claims);
        assert_eq!(s["runtime"]["payroll_count"], payroll);
        assert_eq!(s["runtime"]["claims"].as_array().unwrap().len(), 8);
        for _ in 0..12 {
            let lite = call(&mut c, &mut id, Command::Progress);
            assert_eq!(lite["sequence"], s["sequence"]);
            assert_eq!(lite["revision"], s["revision"]);
            assert_eq!(lite["timeline"], s["timeline"]);
            assert!(lite.get("runtime").is_none() && lite.get("business_sha256").is_none());
            assert!(serde_json::to_vec(&lite).unwrap().len() < 2048);
        }
        let unchanged = call(&mut c, &mut id, Command::Status);
        assert_eq!(s["state_sha256"], unchanged["state_sha256"]);
        let first = first.unwrap();
        let proof = call(
            &mut c,
            &mut id,
            Command::ReceiptProof {
                operation: first.operation_id,
            },
        );
        assert_eq!(proof["present"], true);
        assert_eq!(
            call(&mut c, &mut id, Command::ReceiptProof { operation: 999999 })["present"],
            false
        );
        call(&mut c, &mut id, Command::Checkpoint);
        c.stop().unwrap();
        let current = std::fs::read(world.join("CURRENT.json")).unwrap();
        let mut wrong = Client::spawn_joint(exe, &world, false, false).unwrap();
        assert!(wrong.next(Duration::from_secs(15)).is_err());
        wrong.stop().unwrap();
        assert_eq!(std::fs::read(world.join("CURRENT.json")).unwrap(), current);
        let mut c = Client::spawn_scale(exe, &world, false, false, tier).unwrap();
        assert_eq!(c.next(Duration::from_secs(15)).unwrap().kind, "ready");
        assert_eq!(
            call(
                &mut c,
                &mut id,
                Command::ReceiptProof {
                    operation: first.operation_id
                }
            ),
            proof
        );
        let result = call(&mut c, &mut id, Command::Execute { envelope: first });
        assert_eq!(result["has_persistent_receipt"], true);
        assert_eq!(
            call(&mut c, &mut id, Command::Status)["sequence"],
            payroll + 24
        );
        c.stop().unwrap();
        let current: Value =
            serde_json::from_slice(&std::fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
        let root: Value = serde_json::from_slice(
            &std::fs::read(world.join(format!("root-{}.json", current["slot"]))).unwrap(),
        )
        .unwrap();
        assert_eq!(root["checkpoint_layout"], 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
