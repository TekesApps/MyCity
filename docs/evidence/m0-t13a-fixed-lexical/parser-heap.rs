//! Isolated parser pressure diagnostic, not Store/broker/physical-memory acceptance.
#![forbid(unsafe_code)]
#![allow(dead_code)]
#[path = "../../../crates/city-storage/src/metadata/lexical.rs"]
mod lexical;
#[path = "../../../crates/city-storage/src/metadata/reader.rs"]
mod reader;
#[global_allocator]
static ALLOCATOR: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;
#[derive(serde::Deserialize)]
struct Profile {
    flag: bool,
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    let bytes = std::fs::read(&args[3]).unwrap();
    let mut scratch = (args[1] == "fixed").then(|| lexical::Scratch::new().unwrap());
    let identity = scratch.as_ref().map(lexical::Scratch::identity);
    let before_current = ALLOCATOR.current_usage();
    let before_peak = ALLOCATOR.peak_usage();
    let started = std::time::Instant::now();
    let accepted = match (args[1].as_str(), args[2].as_str()) {
        ("old", "profile") => serde_json::from_slice::<Profile>(&bytes).map(|p| p.flag).is_ok(),
        ("fixed", "profile") => reader::decode::<Profile>(lexical::Raw::root(&bytes), scratch.as_mut().unwrap()).map(|p| p.flag).is_ok(),
        ("old", "ignore") => serde_json::from_slice::<serde::de::IgnoredAny>(&bytes).is_ok(),
        ("fixed", "ignore") => {
            let mut c = lexical::Cursor::new(lexical::Raw::root(&bytes)).unwrap();
            c.skip(scratch.as_mut().unwrap(), lexical::Mode::Ignore).and_then(|()| c.end()).is_ok()
        },
        ("old", "content") => serde_json::from_slice::<serde_json::Value>(&bytes).is_ok(),
        ("fixed", "content") => lexical::validate_content(lexical::Raw::root(&bytes), scratch.as_mut().unwrap()).is_ok(),
        _ => panic!("unknown parser probe"),
    };
    let nanos = started.elapsed().as_nanos();
    let after_current = ALLOCATOR.current_usage();
    let after_peak = ALLOCATOR.peak_usage();
    assert_eq!(identity, scratch.as_ref().map(lexical::Scratch::identity));
    println!("{{\"mode\":\"{}\",\"kind\":\"{}\",\"input_bytes\":{},\"accepted\":{},\"before_current\":{},\"before_lifetime_peak\":{},\"after_current\":{},\"after_lifetime_peak\":{},\"parser_capacity\":{},\"backing_unchanged\":true,\"nanoseconds\":{},\"peak_reset\":false,\"whole_store_or_role_accepted\":false}}",
        args[1], args[2], bytes.len(), accepted, before_current, before_peak, after_current, after_peak, identity.map_or(0, |i| i.1), nanos);
}
