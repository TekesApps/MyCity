use mycity_map::{
    assets::{SpatialAssets, SpatialWorkspace, NORMAL_INPUT_BYTES},
    probe::Input,
    MapSize,
};
const SOURCE: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-exchange-spatial.json");
fn input() -> Input {
    serde_json::from_slice(INPUT).unwrap()
}
#[test]
fn frozen_source_and_all_output_maps_keep_full_scene_hash_and_exact_prepared_backings() {
    for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
        let mut i = input();
        i.terrain.size = size;
        i.scene.map.size = size;
        i.scene.neighbor.size = size;
        i.scene.neighbor.origin.x = size.side_cm();
        i.scene.seam_from.point.x = size.side_cm();
        let encoded = serde_json::to_vec(&serde_json::json!({
            "schema":i.schema,"bootstrap":i.bootstrap,"terrain":i.terrain,"scene":i.scene
        }))
        .unwrap();
        let old = SpatialAssets::from_bytes(&encoded, SOURCE, &i.bootstrap).unwrap();
        let slot = SpatialWorkspace::new().unwrap();
        let id = slot.identity();
        let new = slot.from_bytes(&encoded, SOURCE, &i.bootstrap).unwrap();
        assert_eq!(new.terrain().state_hash(), old.terrain().state_hash());
        assert_eq!(new.scene().state_hash(), old.scene().state_hash());
        assert_eq!(new.terrain().heights(), old.terrain().heights());
        assert_eq!(new.scene().road().points(), old.scene().road().points());
        assert_eq!(
            new.scene().connector().points(),
            old.scene().connector().points()
        );
        let (scene, decoder) = new.normal_backing_identity().unwrap();
        assert_eq!(scene, id.scene);
        assert_eq!(
            decoder,
            (
                id.terrain.decoder_backing,
                id.terrain.decoder_capacity_bytes
            )
        );
        assert_eq!(
            new.terrain().grid_backing_identity(),
            (id.terrain.grid_backing, id.terrain.grid_capacity_bytes)
        );
        assert!(id.prepared_owned_bytes < 11 * 1024 * 1024);
    }
}
#[test]
fn oversize_or_invalid_inputs_never_publish_a_normal_scene() {
    let expected = input().bootstrap;
    assert!(SpatialWorkspace::new()
        .unwrap()
        .from_bytes(&vec![b' '; NORMAL_INPUT_BYTES + 1], SOURCE, &expected)
        .is_err());
    let mut wrong = input();
    wrong.bootstrap.world += 1;
    assert!(SpatialWorkspace::new()
        .unwrap()
        .assemble(wrong, SOURCE, &expected)
        .is_err());
    let mut wrong = input();
    wrong.scene.connector = vec![wrong.scene.connector[0]; 33];
    assert!(SpatialWorkspace::new()
        .unwrap()
        .assemble(wrong, SOURCE, &expected)
        .is_err());
    let mut wrong = input();
    wrong.scene.map.metric_frame = "x".repeat(65);
    assert!(SpatialWorkspace::new()
        .unwrap()
        .assemble(wrong, SOURCE, &expected)
        .is_err());
    let mut corrupt = SOURCE.to_vec();
    corrupt[29] ^= 1;
    let mut wrong = input();
    wrong.terrain.source_sha256 = mycity_core::sha256(&corrupt);
    assert!(SpatialWorkspace::new()
        .unwrap()
        .assemble(wrong, &corrupt, &expected)
        .is_err());
}
#[test]
fn json_limits_are_enforced_before_excess_rows_or_oversize_strings_are_owned() {
    let original: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
    for (name, n) in [
        ("connector", 33),
        ("obstacles", 65),
        ("neighbor_obstacles", 65),
    ] {
        let mut v = original.clone();
        let item = if name == "connector" {
            v["scene"][name][0].clone()
        } else {
            v["scene"]["building"].clone()
        };
        v["scene"][name] = serde_json::json!(vec![item; n]);
        assert!(serde_json::from_slice::<Input>(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    let mut v = original.clone();
    v["scene"]["map"]["metric_frame"] = serde_json::json!("\\\"中".repeat(65));
    assert!(serde_json::from_slice::<Input>(&serde_json::to_vec(&v).unwrap()).is_err());
    let mut v = original;
    v["terrain"]["water"] = serde_json::json!({"kind":"wet_rects","rects":vec![serde_json::json!({"x0":0,"x1":1,"y0":0,"y1":1});33]});
    assert!(serde_json::from_slice::<Input>(&serde_json::to_vec(&v).unwrap()).is_err());
}
#[test]
fn rects_before_kind_is_stream_bounded_and_original_mask_wire_stays_compatible() {
    let rect = r#"{"x0":0,"x1":1,"y0":0,"y1":1}"#;
    let rows = std::iter::repeat(rect)
        .take(33)
        .collect::<Vec<_>>()
        .join(",");
    let bad_water = format!(r#"{{"rects":[{rows}],"kind":"wet_rects"}}"#);
    let bad_validity = format!(r#"{{"rects":[{rows}],"kind":"invalid_rects"}}"#);
    assert!(serde_json::from_str::<mycity_map::terrain::Water>(&bad_water).is_err());
    assert!(serde_json::from_str::<mycity_map::terrain::Validity>(&bad_validity).is_err());
    let good = format!(r#"{{"rects":[{rect}],"kind":"wet_rects"}}"#);
    let water: mycity_map::terrain::Water = serde_json::from_str(&good).unwrap();
    assert_eq!(
        serde_json::to_value(water).unwrap(),
        serde_json::from_str::<serde_json::Value>(&good).unwrap()
    );
    for bad in [
        r#"{"kind":"all_dry","rects":null}"#,
        r#"{"kind":"all_dry","rects":[]}"#,
        r#"{"kind":"wet_rects"}"#,
        r#"{"kind":"all_dry","kind":"all_wet"}"#,
    ] {
        assert!(serde_json::from_str::<mycity_map::terrain::Water>(bad).is_err());
    }
}
#[test]
fn normal_lexical_guard_preserves_escaped_license_but_bounds_scratch_before_decode() {
    let original = std::str::from_utf8(INPUT).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(original).unwrap();
    v["terrain"]["license"] = serde_json::json!("a".repeat(1024));
    let plain = serde_json::to_string(&v).unwrap();
    let escaped = plain.replace(&"a".repeat(1024), &"\\u0061".repeat(1024));
    let expected = input().bootstrap;
    assert!(SpatialWorkspace::new()
        .unwrap()
        .from_bytes(escaped.as_bytes(), SOURCE, &expected)
        .is_ok());
    let too_long = plain.replace(&"a".repeat(1024), &"\\u0061".repeat(1100));
    assert!(SpatialWorkspace::new()
        .unwrap()
        .from_bytes(too_long.as_bytes(), SOURCE, &expected)
        .is_err());
}
