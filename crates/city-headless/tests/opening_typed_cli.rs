//! New typed input through unchanged actual local-file provider and CLI.
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn invoke(p: &Path, name: &str, world: &Path, opening: &Path, resources: &Path, ok: bool) -> Value {
    let exe = std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")));
    let mut child = Command::new(exe)
        .arg("init-opening")
        .args([world, opening, resources])
        .stdin(Stdio::null())
        .stdout(fs::File::create(p.join(format!("{name}.stdout"))).unwrap())
        .stderr(fs::File::create(p.join(format!("{name}.stderr"))).unwrap())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded own CLI child timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    fs::write(
        p.join(format!("{name}.result.json")),
        serde_json::to_vec(&json!({"exit":status.code(),"pid":child.id()})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        status.success(),
        ok,
        "{}",
        fs::read_to_string(p.join(format!("{name}.stderr"))).unwrap()
    );
    if ok {
        serde_json::from_slice(&fs::read(p.join(format!("{name}.stdout"))).unwrap()).unwrap()
    } else {
        Value::Null
    }
}
#[test]
fn actual_local_selected_typed_files_publish_with_exact_original_input_and_refuse_resource_mismatch(
) {
    let root = PathBuf::from(
        std::env::var_os("MYCITY_TYPED_OPENING_CLI_ROOT")
            .expect("existing retained HP P900 root required"),
    )
    .canonicalize()
    .unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p = root.join(format!("actual-typed-cli-{}", std::process::id()));
    fs::create_dir(&p).unwrap();
    for (name, raw, resources) in [
        (
            "base",
            include_bytes!("../../../tests/fixtures/opening-typed/base.json").as_slice(),
            json!([]),
        ),
        (
            "boot",
            include_bytes!("../../../tests/fixtures/opening-typed/boot.json").as_slice(),
            json!([{"id":101,"capacity":2},{"id":102,"capacity":2}]),
        ),
    ] {
        let opening = p.join(format!("{name}-opening.json"));
        let manifest = p.join(format!("{name}-resources.json"));
        let world = p.join(format!("{name}-world"));
        fs::write(&opening, raw).unwrap();
        fs::write(
            &manifest,
            serde_json::to_vec(&json!({"schema":"mycity-resources-v1","resources":resources}))
                .unwrap(),
        )
        .unwrap();
        let report = invoke(&p, name, &world, &opening, &manifest, true);
        assert_eq!(report["opening_source"]["sha256"], mycity_core::sha256(raw));
        assert_eq!(
            report["opening_source"]["source_label"],
            "local_file_os_read"
        );
        let s = Store::open(&world, env!("MYCITY_SOURCE_SHA256")).unwrap();
        let view = serde_json::to_value(s.engine().unwrap().world().view()).unwrap();
        if name == "base" {
            assert_eq!(view["accounts"][0]["cash"], 5000);
            assert_eq!(view["accounts"][1]["cash"], 1000);
            assert_eq!(view["stocks"][0]["basis"], 700);
        } else {
            assert_eq!(s.engine().unwrap().resources().len(), 2);
            assert_eq!(view["boot"]["time"], 0);
        }
        let genesis: Value =
            serde_json::from_slice(&fs::read(world.join("genesis.json")).unwrap()).unwrap();
        assert_eq!(
            serde_json::from_value::<Vec<u8>>(genesis["opening"].clone()).unwrap(),
            raw
        );
        let hash = s.state_hash().unwrap();
        drop(s);
        fs::write(&opening, b"changed external selection; not a new genesis").unwrap();
        let s = Store::open(&world, env!("MYCITY_SOURCE_SHA256")).unwrap();
        assert_eq!(s.state_hash().unwrap(), hash);
        drop(s);
        invoke(
            &p,
            &format!("{name}-refuse-recreate"),
            &world,
            &opening,
            &manifest,
            false,
        );
    }
    let bad = p.join("mismatched-world");
    let opening = p.join("bad-resource-opening.json");
    let manifest = p.join("empty-resource.json");
    fs::write(
        &opening,
        include_bytes!("../../../tests/fixtures/opening-typed/boot.json"),
    )
    .unwrap();
    fs::write(
        &manifest,
        b"{\"schema\":\"mycity-resources-v1\",\"resources\":[]}",
    )
    .unwrap();
    invoke(&p, "resource-mismatch", &bad, &opening, &manifest, false);
    assert!(!bad.join("CURRENT.json").exists());
}
