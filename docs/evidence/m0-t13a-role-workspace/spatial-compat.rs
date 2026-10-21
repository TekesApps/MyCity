//! Complete old world normal-profile open, full canonical bytes and moved backing proof.
#![forbid(unsafe_code)]
use std::{env, fs, path::Path};
fn main() {
    let args: Vec<_> = env::args().collect(); assert_eq!(args.len(), 4);
    let expected = fs::read(&args[3]).unwrap();
    let core = mycity_core::transaction::EngineWorkspace::new().unwrap();
    let prepared_core = core.identity().unwrap();
    let spatial = mycity_map::assets::SpatialWorkspace::new().unwrap();
    let prepared_spatial = spatial.identity();
    let store = mycity_storage::Store::open_in_role_workspace(Path::new(&args[1]), &args[2], (core, spatial)).unwrap();
    let actual = store.engine().unwrap().export_checkpoint().unwrap();
    assert_eq!(actual, expected, "old complete Engine canonical bytes changed");
    let restored_core = store.model_workspace_identity().unwrap();
    assert_eq!(restored_core.initial, prepared_core.restore);
    assert_eq!(restored_core.restore, prepared_core.initial);
    assert_eq!(restored_core.candidate, prepared_core.candidate);
    assert_eq!(restored_core.codec, prepared_core.codec);
    let assets = store.spatial().unwrap().unwrap();
    let (scene, decoder) = assets.normal_backing_identity().unwrap();
    assert_eq!(scene, prepared_spatial.scene);
    assert_eq!(decoder.0, prepared_spatial.terrain.decoder_backing);
    assert_eq!(assets.terrain().grid_backing_identity().0, prepared_spatial.terrain.grid_backing);
    println!("{{\"schema\":\"mycity-role-spatial-old-store-proof-v1\",\"complete_engine_equal\":true,\"bytes\":{},\"sha256\":\"{}\",\"sequence\":{},\"core_owned_bytes\":{},\"spatial_prepared_owned_bytes\":{},\"spatial_live_owned_bytes\":{},\"prepared_backings_preserved\":true,\"runtime_grant_accepted\":false}}",
        actual.len(), mycity_core::sha256(&actual), store.committed_sequence(),
        prepared_core.owned_bytes, prepared_spatial.prepared_owned_bytes, assets.owned_capacity_bytes());
}
