//! Explicit observer fixtures, never economic or performance acceptance workloads.
use super::{err, save};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn run(root: &Path, mode: &str) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let origin = Instant::now();
    if mode == "leaf" {
        std::thread::sleep(Duration::from_millis(1000));
        return Ok(());
    }
    if mode == "long-leaf" {
        std::thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    if mode == "fast-leaf" {
        return Ok(());
    }
    if mode == "no-clock" {
        std::thread::sleep(Duration::from_millis(400));
        return Ok(());
    }
    crate::observer_clock::synchronize(root, || super::now(origin))?;
    save(
        &root.join("manifest.json"),
        &serde_json::json!({"schema":"mycity-native-observer-fixture-v1","external_owned_charge_bytes":0,"not_model_acceptance":true}),
    )?;
    match mode {
        "errno-leaf" => {
            let own = crate::native_observer::capture(std::process::id(), None, origin)
                .map_err(|e| format!("fixture_identity:{e:?}"))?;
            errno::set_errno(errno::Errno(2));
            let children = super::tracker::children(own.identity, origin).map_err(err)?;
            if !children.is_empty() {
                return Err("native_fixture_leaf_not_empty".into());
            }
            save(&root.join("enumeration.json"), &children)?;
        }
        "errno-parent" => {
            let mut child = Command::new(std::env::current_exe().map_err(err)?)
                .arg("native-watch-fixture")
                .arg(root.join("leaf"))
                .arg("leaf")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(err)?;
            let own = crate::native_observer::capture(std::process::id(), None, origin)
                .map_err(|e| format!("fixture_identity:{e:?}"))?;
            errno::set_errno(errno::Errno(2));
            let children = super::tracker::children(own.identity, origin).map_err(err)?;
            if !children.contains(&child.id()) {
                return Err("native_fixture_child_missing".into());
            }
            save(&root.join("enumeration.json"), &children)?;
            if !child.wait().map_err(err)?.success() {
                return Err("native_fixture_child_failed".into());
            }
        }
        "periodic" => std::thread::sleep(Duration::from_secs(61)),
        "hang" => std::thread::sleep(Duration::from_secs(30)),
        "hang-tree" => {
            let mut child = Command::new(std::env::current_exe().map_err(err)?)
                .arg("native-watch-fixture")
                .arg(root.join("long-leaf"))
                .arg("long-leaf")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(err)?;
            save(&root.join("leaf-pid.json"), &child.id())?;
            child.wait().map_err(err)?;
        }
        "ok" | "delayed-owner" => std::thread::sleep(Duration::from_millis(700)),
        "fail" => {
            std::thread::sleep(Duration::from_millis(400));
            return Err("native_observer_fixture_expected_failure".into());
        }
        "orphan" => {
            Command::new(std::env::current_exe().map_err(err)?)
                .arg("native-watch-fixture")
                .arg(root.join("orphan"))
                .arg("leaf")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(err)?;
        }
        "short-child" => {
            let status = Command::new(std::env::current_exe().map_err(err)?)
                .arg("native-watch-fixture")
                .arg(root.join("short-child"))
                .arg("fast-leaf")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(err)?;
            if !status.success() {
                return Err("native_fixture_child".into());
            }
            std::thread::sleep(Duration::from_millis(700));
        }
        _ => return Err("native_observer_fixture_mode".into()),
    }
    save(
        &root.join("fixture-result.json"),
        &serde_json::json!({"mode":mode,"parent":std::process::id(),"scope":"observer fixture only"}),
    )
}
