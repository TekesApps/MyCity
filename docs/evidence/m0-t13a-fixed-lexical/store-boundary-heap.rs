//! Actual prepared Store legacy open; requested heap, not physical/role acceptance.
#![forbid(unsafe_code)]
#[global_allocator]
static ALLOCATOR: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    let expected=std::fs::read(&args[2]).unwrap();
    let workspace=mycity_storage::StoreWorkspace::new().unwrap();
    let identity=workspace.identity().unwrap();
    let before_current=ALLOCATOR.current_usage();
    let before_peak=ALLOCATOR.peak_usage();
    let store=mycity_storage::Store::open_in_prepared_workspace(
        std::path::Path::new(&args[1]),"metadata-compat-proof-v1",workspace).unwrap();
    let after_current=ALLOCATOR.current_usage();
    let after_peak=ALLOCATOR.peak_usage();
    assert_eq!(identity.metadata,store.metadata_workspace_identity().unwrap());
    let actual=store.engine().unwrap().export_checkpoint().unwrap();
    assert_eq!(actual,expected);
    assert_eq!(store.committed_sequence(),2);
    println!("{{\"before_current\":{},\"before_lifetime_peak\":{},\"after_current\":{},\"after_lifetime_peak\":{},\"metadata_owned_bytes\":{},\"lexical_capacity\":{},\"backings_unchanged\":true,\"full_engine_equal\":true,\"engine_bytes\":{},\"engine_sha256\":\"{}\",\"sequence\":{},\"peak_reset\":false,\"whole_role_accepted\":false}}",before_current,before_peak,after_current,after_peak,identity.metadata.owned_bytes,identity.metadata.lexical_capacity_bytes,actual.len(),mycity_core::sha256(&actual),store.committed_sequence());
}
