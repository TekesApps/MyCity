use mycity_core::{
    construction::{Gate, GateCategory, GateDecision, Kind, Material, Phase, Project},
    transaction::Resource,
    units::{GameTime, Money, Quantity},
};
use mycity_storage::Store;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "construction-engine-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("mycity-road-withdrawal-{}-{n}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "road-withdrawal Store artifact retained at {}",
                self.0.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn opening() -> Vec<u8> {
    let mut project = Project::new(
        7,
        Kind::Build,
        4,
        3,
        1,
        80,
        1,
        1,
        Money::new(1_000_000).unwrap(),
        Money::new(3_600).unwrap(),
        28_800,
        &[Gate {
            id: 1,
            category: GateCategory::Permit,
            evidence_id: 1,
            party: 1,
            decided: GateDecision::Accepted,
        }],
        &[Material {
            stock_id: 1,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    project.kind = Kind::Demolish;
    project.phase = Phase::Closed;
    project.operating_resource = 81;
    project.operating_base_capacity = 40;
    project.operating_units = 160;
    project.operating_edge_id = 9;
    project.operating_published_at = Some(GameTime::new(2 * 86_400).unwrap());
    project.operating_withdrawn_at = Some(GameTime::new(4 * 86_400).unwrap());
    project.last_work_at = project.operating_withdrawn_at;

    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path) -> Store {
    Store::create(
        path,
        &opening(),
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 40).unwrap(),
        ],
        RULES,
    )
    .unwrap()
}

#[test]
fn cold_open_preserves_irreversible_road_withdrawal_timestamps() {
    let temp = Temp::new();
    drop(create(&temp.world()));
    let store = Store::open(&temp.world(), RULES).unwrap();
    let project = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.kind, Kind::Demolish);
    assert_eq!(project.phase, Phase::Closed);
    assert_eq!(project.operating_published, false);
    assert_eq!(project.operating_published_at.unwrap().value(), 2 * 86_400);
    assert_eq!(project.operating_withdrawn_at.unwrap().value(), 4 * 86_400);
    assert_eq!(store.engine().unwrap().resources()[1].capacity, 40);
}
