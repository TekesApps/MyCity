use sha2::{Digest, Sha256};
use std::{env, fs, path::Path, process::Command};

fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) {
    for e in fs::read_dir(dir).expect("source directory") {
        let p = e.expect("source entry").path();
        if p.is_dir() {
            collect(root, &p, files);
        } else {
            files.push(p.strip_prefix(root).unwrap().to_string_lossy().into_owned());
        }
    }
}

fn output(command: &str, args: &[&str], dir: &Path) -> String {
    let out = Command::new(command)
        .args(args)
        .current_dir(dir)
        .output()
        .expect(command);
    assert!(out.status.success(), "{command} failed");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).parent().unwrap().parent().unwrap();
    let mut files = vec![
        ".cargo/config.toml".into(),
        "Cargo.toml".into(),
        "Cargo.lock".into(),
        "rust-toolchain.toml".into(),
    ];
    println!("cargo:rerun-if-changed={}", root.join("crates").display());
    println!("cargo:rerun-if-changed={}", root.join("fixtures").display());
    collect(root, &root.join("crates"), &mut files);
    collect(root, &root.join("fixtures"), &mut files);
    files.sort();
    let mut hash = Sha256::new();
    for name in files {
        println!("cargo:rerun-if-changed={}", root.join(&name).display());
        let bytes = fs::read(root.join(&name)).unwrap();
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!("cargo:rustc-env=MYCITY_SOURCE_SHA256={:x}", hash.finalize());
    println!(
        "cargo:rustc-env=MYCITY_LOCK_SHA256={:x}",
        Sha256::digest(fs::read(root.join("Cargo.lock")).unwrap())
    );
    println!(
        "cargo:rustc-env=MYCITY_RUSTC={}",
        output(&env::var("RUSTC").unwrap(), &["--version"], root)
    );
    println!(
        "cargo:rustc-env=MYCITY_TARGET={}",
        env::var("TARGET").unwrap()
    );
    println!(
        "cargo:rustc-env=MYCITY_PROFILE={}",
        env::var("PROFILE").unwrap()
    );
}
