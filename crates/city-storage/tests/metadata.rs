use mycity_core::{
    transaction::{Action, Caller, Envelope, TxError},
    units::Money,
};
use mycity_storage::{Store, StoreWorkspace};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const RULES: &str = "retained-metadata-regression";
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-metadata-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn movement(s: &Store, id: u64) -> Envelope {
    let w = s.engine().unwrap().world();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: id,
        expected_revision: w.view().revision,
        actions: vec![Action::Transfer {
            from: w.account_ref(3).unwrap(),
            to: w.account_ref(2).unwrap(),
            amount: Money::new(1).unwrap(),
        }],
    }
}
#[test]
fn prepared_metadata_is_consumed_and_reused_for_both_formats_save_archive_query_and_cold_open() {
    for v2 in [false, true] {
        let temp = Temp::new();
        let workspace = StoreWorkspace::new().unwrap();
        let id = workspace.identity().unwrap();
        let mut s = Store::create_spatial_in_prepared_workspace(
            &temp.world(),
            include_bytes!("../../../fixtures/m0-e0-opening.json"),
            vec![],
            RULES,
            (
                include_bytes!("../../../fixtures/m0-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            ),
            v2,
            workspace,
        )
        .unwrap();
        assert_eq!(s.metadata_workspace_identity().unwrap(), id.metadata);
        let mut failed = movement(&s, 1);
        failed.expected_revision = 999;
        assert_eq!(
            s.execute(Caller::System, failed.clone()).unwrap().model,
            Err(TxError::StaleRevision)
        );
        let good = movement(&s, 2);
        s.execute(Caller::System, good.clone())
            .unwrap()
            .model
            .unwrap();
        let image = s.engine().unwrap().export_checkpoint().unwrap();
        let records = s.engine().unwrap().journal().to_vec();
        s.checkpoint().unwrap();
        if !v2 {
            let current: serde_json::Value =
                serde_json::from_slice(&fs::read(temp.world().join("CURRENT.json")).unwrap())
                    .unwrap();
            let root: serde_json::Value = serde_json::from_slice(
                &fs::read(temp.world().join(format!("root-{}.json", current["slot"]))).unwrap(),
            )
            .unwrap();
            let cp = fs::read(temp.world().join(format!(
                "checkpoint-{}.json",
                root["checkpoint"]["generation"]
            )))
            .unwrap();
            #[derive(serde::Serialize)]
            struct OldView<'a> {
                schema: &'a str,
                sequence: u64,
                chain: &'a str,
                genesis_sha256: &'a str,
                engine: &'a [u8],
                receipts: &'a [mycity_core::transaction::JournalRecord],
                previous: Option<serde_json::Value>,
            }
            let actual: serde_json::Value = serde_json::from_slice(&cp).unwrap();
            let expected = serde_json::to_vec(&OldView {
                schema: "mycity-m0-checkpoint-v1",
                sequence: 2,
                chain: actual["chain"].as_str().unwrap(),
                genesis_sha256: actual["genesis_sha256"].as_str().unwrap(),
                engine: &image,
                receipts: &records,
                previous: None,
            })
            .unwrap();
            assert_eq!(
                cp, expected,
                "legacy wire must preserve old field order and full values"
            );
        }
        for id in 3..=10 {
            let e = movement(&s, id);
            s.execute(Caller::System, e).unwrap().model.unwrap();
        }
        s.checkpoint().unwrap();
        let digest = s.state_hash().unwrap();
        assert_eq!(s.metadata_workspace_identity().unwrap(), id.metadata);
        assert_eq!(
            s.receipt(1).unwrap().unwrap().outcome,
            Err(TxError::StaleRevision)
        );
        assert_eq!(s.metadata_workspace_identity().unwrap(), id.metadata);
        drop(s);
        let workspace = StoreWorkspace::new().unwrap();
        let next = workspace.identity().unwrap();
        let mut s = Store::open_in_prepared_workspace(&temp.world(), RULES, workspace).unwrap();
        assert_eq!(s.metadata_workspace_identity().unwrap(), next.metadata);
        assert_eq!(s.state_hash().unwrap(), digest);
        assert_eq!(
            s.execute(Caller::System, failed).unwrap().model,
            Err(TxError::StaleRevision)
        );
        s.execute(Caller::System, good).unwrap().model.unwrap();
        assert_eq!(s.state_hash().unwrap(), digest);
        assert_eq!(s.committed_sequence(), 10);
        assert_eq!(s.metadata_workspace_identity().unwrap(), next.metadata);
    }
}

#[test]
fn original_legacy_checkpoint_with_large_ignored_receipt_data_and_positional_headers_reopens() {
    for positional in [false, true] {
        let temp = Temp::new();
        let mut s = Store::create_spatial_in_prepared_workspace(
            &temp.world(),
            include_bytes!("../../../fixtures/m0-e0-opening.json"),
            vec![],
            RULES,
            (
                include_bytes!("../../../fixtures/m0-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            ),
            false,
            StoreWorkspace::new().unwrap(),
        )
        .unwrap();
        let e = movement(&s, 91);
        s.execute(Caller::System, e.clone()).unwrap().model.unwrap();
        let expected = s.engine().unwrap().export_checkpoint().unwrap();
        let expected_record = serde_json::to_value(s.engine().unwrap().journal()[0]).unwrap();
        s.checkpoint().unwrap();
        drop(s);
        let world = temp.world();
        let mut current: serde_json::Value =
            serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
        let slot = current["slot"].as_u64().unwrap();
        let root_path = world.join(format!("root-{slot}.json"));
        let mut root: serde_json::Value =
            serde_json::from_slice(&fs::read(&root_path).unwrap()).unwrap();
        let generation = root["checkpoint"]["generation"].as_u64().unwrap();
        let cp_path = world.join(format!("checkpoint-{generation}.json"));
        let index_path = world.join(format!("checkpoint-index-{generation}.json"));
        let mut cp: serde_json::Value =
            serde_json::from_slice(&fs::read(&cp_path).unwrap()).unwrap();
        // Both values were accepted by the original JournalRecord derive and
        // exceed a WAL frame while the whole file remains within its original limit.
        cp["receipts"][0]["ignored"] = serde_json::Value::Array(vec![0.into(); 65536]);
        let receipt = serde_json::to_string_pretty(&cp["receipts"][0]).unwrap();
        assert!(receipt.len() > mycity_storage::FRAME_LIMIT);
        let old: mycity_core::transaction::JournalRecord = serde_json::from_str(&receipt).unwrap();
        assert_eq!(serde_json::to_value(old).unwrap(), expected_record);
        let bytes = if positional {
            serde_json::to_vec(&[
                cp["schema"].clone(),
                cp["sequence"].clone(),
                cp["chain"].clone(),
                cp["genesis_sha256"].clone(),
                cp["engine"].clone(),
                cp["receipts"].clone(),
                cp["previous"].clone(),
            ])
            .unwrap()
        } else {
            serde_json::to_vec_pretty(&cp).unwrap()
        };
        fs::write(&cp_path, &bytes).unwrap();
        let mut index: serde_json::Value =
            serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
        index["data_sha256"] = mycity_core::sha256(&bytes).into();
        let bytes = if positional {
            serde_json::to_vec(&[
                index["data_sha256"].clone(),
                index["operations"].clone(),
                index["previous"].clone(),
            ])
            .unwrap()
        } else {
            serde_json::to_vec(&index).unwrap()
        };
        fs::write(&index_path, &bytes).unwrap();
        root["checkpoint"]["sha256"] = mycity_core::sha256(&bytes).into();
        let bytes = serde_json::to_vec(&root).unwrap();
        fs::write(&root_path, &bytes).unwrap();
        current["root_sha256"] = mycity_core::sha256(&bytes).into();
        fs::write(
            world.join("CURRENT.json"),
            serde_json::to_vec(&current).unwrap(),
        )
        .unwrap();
        let workspace = StoreWorkspace::new().unwrap();
        let identity = workspace.identity().unwrap();
        let mut s = Store::open_in_prepared_workspace(&world, RULES, workspace).unwrap();
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), expected);
        assert_eq!(
            serde_json::to_value(s.receipt(91).unwrap().unwrap()).unwrap(),
            expected_record
        );
        s.execute(Caller::System, e).unwrap().model.unwrap();
        assert_eq!(s.committed_sequence(), 1);
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), expected);
        assert_eq!(s.metadata_workspace_identity().unwrap(), identity.metadata);
        // Temp owns and removes the entire synthetic world after Store closes.
    }
}
