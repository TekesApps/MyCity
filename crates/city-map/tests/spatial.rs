use mycity_core::sha256;
use mycity_map::{geometry::*, terrain::*, *};
fn p(x: i64, y: i64) -> Point {
    Point { x, y }
}
fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
    Rect {
        min: p(x0, y0),
        max: p(x1, y1),
    }
}
fn frame(id: u32, x: i64, y: i64) -> Frame {
    Frame {
        world: 1,
        branch: 1,
        map: id,
        size: MapSize::Small,
        origin: p(x, y),
        metric_frame: "synthetic-local-v1".into(),
        vertical_datum: "synthetic-z0".into(),
    }
}
fn port(point: Point, facing: Facing) -> Port {
    Port {
        point,
        z_cm: 0,
        mode: Mode::Walk,
        facing,
        clear_width_cm: 200,
        can_enter: true,
        can_exit: true,
        authorized: true,
    }
}
fn png_bytes(w: u32, h: u32, depth: png::BitDepth, raw: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, w, h);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(raw).unwrap();
    }
    bytes
}
fn manifest(bytes: &[u8], w: u32, h: u32, depth: u8, size: MapSize) -> Manifest {
    Manifest {
        schema: "mycity-heightmap-v1".into(),
        source_sha256: sha256(bytes),
        width: w,
        height: h,
        bit_depth: depth,
        size,
        sample_semantics: "vertices".into(),
        row0: "north".into(),
        z_min_cm: 0,
        z_max_cm: 65535,
        vertical_datum: "synthetic-z0".into(),
        source_resolution_cm: (size.side_cm() / i64::from(w - 1)) as u32,
        source_accuracy_cm: None,
        synthetic: true,
        georeferencing: "local_metric".into(),
        processing_version: "bilinear-cm-swne-v1".into(),
        license: "CC0 synthetic test".into(),
        validity: Validity::AllValid,
        water: Water::AllDry,
    }
}
#[test]
fn maps_keep_one_grid_format_exact_global_coordinates_and_nonoverlapping_cells() {
    for (size, cells, n) in [
        (MapSize::Small, 64, 257),
        (MapSize::Medium, 128, 513),
        (MapSize::Large, 256, 1025),
    ] {
        assert_eq!(size.cells(), cells);
        assert_eq!(size.vertices(), n);
        let mut f = frame(1, -102400, 400);
        f.size = size;
        assert_eq!(f.cell(p(1599, 1599)).unwrap(), (0, 0));
        assert_eq!(f.cell(p(1600, 1600)).unwrap(), (1, 1));
        assert_eq!(f.cell(p(size.side_cm(), 0)), Err(Error::Bound));
        assert_eq!(
            f.global(p(size.side_cm(), 0)).unwrap(),
            p(size.side_cm() - 102400, 400)
        );
    }
    let mut f = frame(1, i64::MAX, 0);
    assert_eq!(f.bounds(), Err(Error::Bound));
    f.origin = p(XY_LIMIT_CM, 0);
    assert_eq!(f.bounds(), Err(Error::Bound));
    assert_eq!(rect(-100, -100, 100, 100).area_cm2().unwrap(), 40000);
}
#[test]
fn grayscale_8_and_16_north_orientation_and_negative_half_rounding_are_explicit() {
    let bytes = png_bytes(
        2,
        2,
        png::BitDepth::Sixteen,
        &[0, 0, 255, 255, 128, 0, 64, 0],
    );
    let t = import(manifest(&bytes, 2, 2, 16, MapSize::Small), &bytes, 1, 0).unwrap();
    assert_eq!(t.height_cm(p(0, 102400)).unwrap(), 0);
    assert_eq!(t.height_cm(p(102400, 102400)).unwrap(), 65535);
    assert_eq!(t.height_cm(p(0, 0)).unwrap(), 32768);
    assert_eq!(t.height_cm(p(102400, 0)).unwrap(), 16384);
    let bytes = png_bytes(2, 2, png::BitDepth::Eight, &[0, 255, 0, 255]);
    let mut m = manifest(&bytes, 2, 2, 8, MapSize::Small);
    m.z_min_cm = -128;
    m.z_max_cm = 127;
    let t = import(m, &bytes, 1, 0).unwrap();
    assert_eq!(t.height_cm(p(51200, 100)).unwrap(), -1);
    assert_eq!(t.height_cm(p(0, 0)).unwrap(), -128);
    assert_eq!(t.height_cm(p(102400, 0)).unwrap(), 127);
}
#[test]
fn runtime_uses_sw_ne_triangles_and_platform_proof_checks_internal_peaks() {
    let mut raw = vec![0; 257 * 257 * 2];
    let index = ((256 - 1) * 257 + 1) * 2;
    raw[index..index + 2].copy_from_slice(&100u16.to_be_bytes());
    let bytes = png_bytes(257, 257, png::BitDepth::Sixteen, &raw);
    let t = import(manifest(&bytes, 257, 257, 16, MapSize::Small), &bytes, 1, 0).unwrap();
    assert_eq!(t.height_cm(p(200, 200)).unwrap(), 50); // bilinear would be 25
    assert_eq!(t.height_cm(p(100, 200)).unwrap(), 25);
    assert_eq!(
        t.flat_platform(rect(0, 0, 800, 800), 0),
        Err(Error::NeedsDetailedValidation)
    );
    assert!(t.flat_platform(rect(2000, 2000, 3000, 3000), 0).is_ok());
    assert_eq!(t.height_cm(p(-1, 0)), Err(Error::Bound));
}
#[test]
fn black_is_valid_no_data_never_becomes_zero_and_water_is_a_separate_mask() {
    let bytes = png_bytes(2, 2, png::BitDepth::Eight, &[0; 4]);
    let mut m = manifest(&bytes, 2, 2, 8, MapSize::Small);
    m.z_min_cm = 0;
    let old = import(m.clone(), &bytes, 1, 0).unwrap();
    let hash = old.state_hash();
    assert_eq!(old.height_cm(p(0, 0)).unwrap(), 0);
    m.validity = Validity::InvalidRects {
        rects: vec![PixelRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 1,
        }],
    };
    assert!(matches!(
        import(m, &bytes, 2, old.owned_capacity_bytes()),
        Err(Error::NoData)
    ));
    assert_eq!(old.state_hash(), hash);
    let mut m = manifest(&bytes, 2, 2, 8, MapSize::Small);
    m.water = Water::WetRects {
        rects: vec![PixelRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 1,
        }],
    };
    let wet = import(m, &bytes, 1, 0).unwrap();
    assert!(wet.water_at_vertex(0, 256).unwrap());
    assert!(!wet.water_at_vertex(256, 0).unwrap());
    assert_eq!(
        wet.flat_platform(rect(0, 102000, 400, 102400), 0),
        Err(Error::NeedsDetailedValidation)
    );
}
#[test]
fn decode_rejects_mismatches_corruption_unsupported_semantics_and_peak_overcommit() {
    let bytes = png_bytes(2, 2, png::BitDepth::Eight, &[0; 4]);
    let m = manifest(&bytes, 2, 2, 8, MapSize::Small);
    let mut bad = m.clone();
    bad.width = 3;
    bad.height = 3;
    assert!(import(bad, &bytes, 1, 0).is_err());
    let mut bad = m.clone();
    bad.source_sha256 = "0".repeat(64);
    assert!(matches!(import(bad, &bytes, 1, 0), Err(Error::Hash)));
    for s in ["pixel_centers", "unknown"] {
        let mut bad = m.clone();
        bad.sample_semantics = s.into();
        assert!(matches!(import(bad, &bytes, 1, 0), Err(Error::Unsupported)));
    }
    let mut bad = m.clone();
    bad.license.clear();
    assert!(import(bad, &bytes, 1, 0).is_err());
    assert!(matches!(
        import(m.clone(), &bytes, 1, IMPORT_PEAK_BUDGET_BYTES),
        Err(Error::Limit)
    ));
    for cut in [0, 8, 20, bytes.len() - 1] {
        let corrupt = &bytes[..cut];
        assert!(import(m.clone(), corrupt, 1, 0).is_err());
    }
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    let mut bad = m.clone();
    bad.source_sha256 = sha256(&corrupt);
    assert!(matches!(import(bad, &corrupt, 1, 0), Err(Error::Png)));
    let mut huge = bytes.clone();
    huge[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(import(m, &huge, 1, 0).is_err());
}
#[test]
fn transparency_color_and_unbounded_metadata_are_never_reinterpreted_as_height() {
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, 2, 2);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Eight);
        e.set_trns(vec![0, 0]);
        let mut w = e.write_header().unwrap();
        w.write_image_data(&[0; 4]).unwrap();
    }
    assert!(matches!(
        import(manifest(&bytes, 2, 2, 8, MapSize::Small), &bytes, 1, 0),
        Err(Error::Unsupported)
    ));
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, 2, 2);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().unwrap();
        w.write_image_data(&[0; 12]).unwrap();
    }
    assert!(matches!(
        import(manifest(&bytes, 2, 2, 8, MapSize::Small), &bytes, 1, 0),
        Err(Error::Unsupported)
    ));
    let mut v = serde_json::to_value(manifest(&bytes, 2, 2, 8, MapSize::Small)).unwrap();
    v["path"] = serde_json::json!("../../private");
    assert!(serde_json::from_value::<Manifest>(v).is_err());
}
#[test]
fn all_map_sizes_and_maximum_source_are_actually_decoded_with_bounded_owned_storage() {
    for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
        let bytes = png_bytes(2, 2, png::BitDepth::Sixteen, &[0; 8]);
        let m = manifest(&bytes, 2, 2, 16, size);
        let t = import(m, &bytes, 1, 0).unwrap();
        assert_eq!(t.heights().len(), size.vertices() * size.vertices());
        assert!(t.owned_capacity_bytes() <= TERRAIN_RESIDENT_BUDGET_BYTES);
    }
    let bytes = png_bytes(
        4097,
        4097,
        png::BitDepth::Sixteen,
        &vec![0; 4097 * 4097 * 2],
    );
    let m = manifest(&bytes, 4097, 4097, 16, MapSize::Large);
    let b = budget(&m, bytes.len(), 10 * 1024 * 1024).unwrap();
    assert_eq!(b.decoded_bytes, 33570818);
    assert!(b.planned_peak_bytes < IMPORT_PEAK_BUDGET_BYTES);
    let t = import(m, &bytes, 2, 10 * 1024 * 1024).unwrap();
    assert_eq!(t.heights().len(), 1050625);
}
#[test]
fn one_centimeter_invasion_holes_width_and_vertical_overlap_are_not_waived_by_grid_or_layer() {
    let region = Region {
        outer: rect(0, 0, 10000, 10000),
        holes: vec![rect(4000, 4000, 6000, 6000)],
    };
    assert!(region.contains_rect(rect(1500, 1500, 3000, 3000)).is_ok()); // crosses the 16m cell boundary
    assert_eq!(
        region.contains_rect(rect(3000, 4000, 4001, 5000)),
        Err(Error::Permission)
    );
    let road = Corridor::from_polyline(&[p(1000, 3500), p(9000, 3500)], 1002).unwrap();
    assert_eq!(road.validate_region(&region), Err(Error::Permission));
    let a = Prism {
        footprint: rect(1000, 1000, 2000, 2000),
        z_min_cm: -50,
        z_max_cm: 1000,
        layer: Layer::G,
    };
    let b = Prism {
        footprint: rect(1999, 1000, 3000, 2000),
        z_min_cm: -100,
        z_max_cm: 0,
        layer: Layer::U1,
    };
    assert!(a.collision(b, 0).unwrap());
    let mut b = b;
    b.z_max_cm = -50;
    assert!(!a.collision(b, 0).unwrap());
    assert!(a.collision(b, 1).unwrap());
    let edge = Corridor::from_polyline(&[p(500, 1000), p(500, 9000)], 1002).unwrap();
    assert_eq!(edge.validate_region(&region), Err(Error::Permission));
}
#[test]
fn quadratic_has_independently_sampled_error_below_five_cm_and_no_silent_budget_downgrade() {
    let q = Quadratic {
        start: p(1000, 1000),
        control: p(12000, 20000),
        end: p(30000, 1000),
    };
    let road = Corridor::quadratic(q, 700).unwrap();
    assert!(road.points().len() > 2);
    assert_eq!(road.radius_cm(), 355);
    let distance = |p: (f64, f64), a: Point, b: Point| {
        let dx = (b.x - a.x) as f64;
        let dy = (b.y - a.y) as f64;
        let t = (((p.0 - a.x as f64) * dx + (p.1 - a.y as f64) * dy) / (dx * dx + dy * dy))
            .clamp(0.0, 1.0);
        ((p.0 - a.x as f64 - t * dx).powi(2) + (p.1 - a.y as f64 - t * dy).powi(2)).sqrt()
    };
    for i in 0..=10000 {
        let t = i as f64 / 10000.0;
        let u = 1.0 - t;
        let xy = (
            u * u * q.start.x as f64 + 2.0 * u * t * q.control.x as f64 + t * t * q.end.x as f64,
            u * u * q.start.y as f64 + 2.0 * u * t * q.control.y as f64 + t * t * q.end.y as f64,
        );
        let error = road
            .points()
            .windows(2)
            .map(|s| distance(xy, s[0], s[1]))
            .fold(f64::INFINITY, f64::min);
        assert!(error <= 5.0, "{error}");
    }
    assert!(Corridor::from_polyline(&vec![p(0, 0); MAX_VERTICES + 1], 100).is_err());
    let huge = Quadratic {
        start: p(-XY_LIMIT_CM, -XY_LIMIT_CM),
        control: p(0, XY_LIMIT_CM),
        end: p(XY_LIMIT_CM, -XY_LIMIT_CM),
    };
    assert!(matches!(Corridor::quadratic(huge, 700), Err(Error::Limit)));
    assert_eq!(
        Corridor::from_polyline(&[p(0, 0), p(300, 400)], 100)
            .unwrap()
            .length_upper_cm(),
        500
    );
}
#[test]
fn seams_require_real_global_adjacency_same_height_frame_mode_permission_and_direction() {
    let a = frame(1, 0, 0);
    let mut b = frame(2, 102400, 0);
    let ap = port(p(102400, 2000), Facing::East);
    let bp = port(p(0, 2000), Facing::West);
    assert!(seam(&a, ap, &b, bp).is_ok());
    for x in [0, 102399, 102401, 204800] {
        b.origin.x = x;
        assert!(seam(&a, ap, &b, bp).is_err());
    }
    b.origin.x = 102400;
    let mut bad = bp;
    bad.z_cm = 1;
    assert_eq!(seam(&a, ap, &b, bad), Err(Error::Seam));
    let mut bad = bp;
    bad.mode = Mode::Electric;
    assert_eq!(seam(&a, ap, &b, bad), Err(Error::Seam));
    let mut bad = bp;
    bad.authorized = false;
    assert_eq!(seam(&a, ap, &b, bad), Err(Error::Permission));
    let mut bad = bp;
    bad.can_enter = false;
    assert_eq!(seam(&a, ap, &b, bad), Err(Error::Direction));
    b.vertical_datum = "other".into();
    assert_eq!(seam(&a, ap, &b, bp), Err(Error::Seam));
}
#[test]
fn entrance_checks_first_normal_full_route_obstacles_modes_and_exact_anchor() {
    let shell = Prism {
        footprint: rect(2000, 2000, 4000, 4000),
        z_min_cm: -60,
        z_max_cm: 1000,
        layer: Layer::G,
    };
    let door = port(p(3000, 2000), Facing::South);
    let anchor = port(p(5000, 1000), Facing::North);
    let path = Corridor::from_polyline(&[door.point, p(3000, 1000), anchor.point], 200).unwrap();
    let region = Region {
        outer: rect(0, 0, 10000, 10000),
        holes: vec![],
    };
    assert!(entrance(shell, door, anchor, &path, &region, &[]).is_ok()); // later turn is legal
    let mut bad = door;
    bad.facing = Facing::North;
    assert_eq!(
        entrance(shell, bad, anchor, &path, &region, &[]),
        Err(Error::Direction)
    );
    let mut bad = anchor;
    bad.point.x += 1;
    assert_eq!(
        entrance(shell, door, bad, &path, &region, &[]),
        Err(Error::Input)
    );
    let obstacle = Prism {
        footprint: rect(3900, 950, 4100, 1050),
        z_min_cm: 0,
        z_max_cm: 400,
        layer: Layer::U1,
    };
    assert_eq!(
        entrance(shell, door, anchor, &path, &region, &[obstacle]),
        Err(Error::Collision)
    );
    let mut bad = anchor;
    bad.z_cm = 1;
    assert_eq!(
        entrance(shell, door, bad, &path, &region, &[]),
        Err(Error::Layer)
    );
    let returning = Corridor::from_polyline(
        &[
            door.point,
            p(3000, 1000),
            p(5000, 1000),
            p(5000, 3000),
            p(3000, 3000),
            anchor.point,
        ],
        200,
    )
    .unwrap();
    assert_eq!(
        entrance(shell, door, anchor, &returning, &region, &[]),
        Err(Error::Collision)
    );
}

#[test]
fn joint_scene_uses_support_whole_paths_revisions_and_real_neighbor_frames() {
    let bytes = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
    let input: probe::Input =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-spatial.json")).unwrap();
    let t = import(input.terrain, bytes, 1, 0).unwrap();
    let scene = scene::Scene::assemble(input.scene.clone(), &t, &t).unwrap();
    assert!(scene.owned_capacity_bytes() < GEOMETRY_BUDGET_BYTES);
    let before = scene.state_hash();
    let mut bad = input.scene.clone();
    bad.terrain_revision = 2;
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::Stale)
    ));
    let mut bad = input.scene.clone();
    bad.neighbor.origin.x += 1;
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::Seam)
    ));
    let mut bad = input.scene.clone();
    bad.door.point.x = 2500;
    assert!(scene::Scene::assemble(bad, &t, &t).is_err());
    let mut bad = input.scene.clone();
    bad.region.holes.push(rect(2999, 999, 3001, 1001));
    assert!(scene::Scene::assemble(bad, &t, &t).is_err());
    let mut bad = input.scene.clone();
    bad.road_mode = Mode::Vehicle;
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::Unsupported)
    ));
    let mut bad = input.scene;
    bad.ground_z_cm = 1;
    bad.door.z_cm = 1;
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::NeedsDetailedValidation)
    ));
    assert_eq!(scene.state_hash(), before);
}
#[test]
fn metadata_and_overexpanded_streams_reject_and_known_narrow_water_is_not_lost() {
    let bytes = include_bytes!("../../../fixtures/terrain/m0-overexpanded8.png");
    assert!(import(manifest(bytes, 2, 2, 8, MapSize::Small), bytes, 1, 0).is_err());
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, 2, 2);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Eight);
        e.add_text_chunk("Author".into(), "untrusted metadata".into())
            .unwrap();
        let mut w = e.write_header().unwrap();
        w.write_image_data(&[0; 4]).unwrap();
    }
    assert!(matches!(
        import(manifest(&bytes, 2, 2, 8, MapSize::Small), &bytes, 1, 0),
        Err(Error::Unsupported)
    ));
    let bytes = png_bytes(1025, 1025, png::BitDepth::Eight, &vec![0; 1025 * 1025]);
    let mut m = manifest(&bytes, 1025, 1025, 8, MapSize::Small);
    m.water = Water::WetRects {
        rects: vec![PixelRect {
            x0: 6,
            y0: 1020,
            x1: 7,
            y1: 1021,
        }],
    };
    let t = import(m, &bytes, 1, 0).unwrap();
    // 1m source water at x6m sits between the target x4m and x8m vertices.
    assert!(!t.water_at_vertex(1, 1).unwrap());
    assert!(!t.water_at_vertex(2, 1).unwrap());
    assert_eq!(
        t.flat_platform(rect(500, 300, 700, 500), 0),
        Err(Error::NeedsDetailedValidation)
    );
}

#[test]
fn joint_seam_rejects_water_off_center_cliff_neighbor_obstacle_and_unsupported_modes() {
    let bytes = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
    let input: probe::Input =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-spatial.json")).unwrap();
    let t = import(input.terrain.clone(), bytes, 1, 0).unwrap();
    let mut bad = input.scene.clone();
    bad.neighbor_obstacles.push(Prism {
        footprint: rect(0, 4999, 100, 5001),
        z_min_cm: 0,
        z_max_cm: 400,
        layer: Layer::U1,
    });
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::Collision)
    ));
    let mut bad = input.scene.clone();
    bad.seam_from.mode = Mode::Electric;
    bad.seam_to.mode = Mode::Electric;
    assert!(matches!(
        scene::Scene::assemble(bad, &t, &t),
        Err(Error::Unsupported)
    ));
    let mut m = input.terrain.clone();
    m.water = Water::WetRects {
        rects: vec![PixelRect {
            x0: 256,
            y0: 244,
            x1: 257,
            y1: 245,
        }],
    };
    let wet = import(m, bytes, 1, 0).unwrap();
    assert!(matches!(
        scene::Scene::assemble(input.scene.clone(), &wet, &t),
        Err(Error::NeedsDetailedValidation)
    ));
    let mut raw = vec![0; 257 * 257 * 2];
    let index = (243 * 257 + 256) * 2;
    raw[index..index + 2].copy_from_slice(&100u16.to_be_bytes());
    let bytes = png_bytes(257, 257, png::BitDepth::Sixteen, &raw);
    let cliff = import(manifest(&bytes, 257, 257, 16, MapSize::Small), &bytes, 1, 0).unwrap();
    let mut bad = input.scene;
    bad.seam_from.point.y = 4800;
    bad.seam_to.point.y = 4800;
    assert_eq!(cliff.height_cm(bad.seam_from.point).unwrap(), 0); // center alone would pass
    assert!(matches!(
        scene::Scene::assemble(bad, &cliff, &t),
        Err(Error::NeedsDetailedValidation)
    ));
}

#[test]
fn gpu_bytes_have_wgsl_offsets_soa_strides_versions_and_no_global_float_coordinates() {
    use mycity_map::gpu_layout::*;
    let bytes = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
    let mut input: probe::Input =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-spatial.json")).unwrap();
    let t = import(input.terrain, bytes, 1, 0).unwrap();
    input.scene.map.origin = p(900000000, -900000000);
    let upload = TileUpload::pack(
        &t,
        &input.scene.map,
        0x100000002,
        0x300000004,
        64,
        64,
        -1000000,
    )
    .unwrap();
    let word = |i: usize| u32::from_le_bytes(upload.header()[i * 4..i * 4 + 4].try_into().unwrap());
    assert_eq!(upload.header().len(), 64);
    assert_eq!((word(0), word(1), word(2), word(3)), (65, 65, 64, 64));
    assert_eq!((word(8), word(9), word(10), word(11)), (1, 0, 2, 1));
    assert_eq!((word(12), word(13)), (4, 3));
    assert_eq!(word(14) as i32, -1000000);
    assert_eq!(upload.heights().len(), 65 * 65 * 4);
    assert_eq!(upload.flags().len(), 65 * 65 * 4);
    assert!(upload.owned_capacity_bytes() < TILE_UPLOAD_BUDGET_BYTES);
    assert_eq!(upload.world_origin_cm, p(900025600, -899974400));
    let v = f32::from_le_bytes(upload.heights()[..4].try_into().unwrap());
    assert_eq!(v, 10000.0);
    let mut stale = upload.stamp().clone();
    assert!(upload.matches_current(&stale));
    stale.generation += 1;
    assert!(!upload.matches_current(&stale));
    stale = upload.stamp().clone();
    stale.terrain_sha256 = "0".repeat(64);
    assert!(!upload.matches_current(&stale));
    let data = vertices(
        &[p(900000001, -899999999)],
        123,
        p(900000000, -900000000),
        0,
    )
    .unwrap();
    assert_eq!(data.len(), 16);
    let coords = data
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(coords, vec![0.01, 0.01, 1.23, 1.0]);
    assert!(vertices(&[p(900025601, 0)], 0, p(900000000, 0), 0).is_err());
}
#[test]
fn gpu_height_projection_stays_within_declared_half_cm_on_nonflat_signed_data() {
    let bytes = png_bytes(
        2,
        2,
        png::BitDepth::Sixteen,
        &[0, 0, 255, 255, 128, 0, 64, 0],
    );
    let mut m = manifest(&bytes, 2, 2, 16, MapSize::Small);
    m.z_min_cm = -1000000;
    m.z_max_cm = 1000000;
    let t = import(m, &bytes, 1, 0).unwrap();
    let f = frame(1, 0, 0);
    for y in [0, 64, 128, 192] {
        for x in [0, 64, 128, 192] {
            let upload = gpu_layout::TileUpload::pack(&t, &f, 1, 1, x, y, -1000000).unwrap();
            assert!(upload.max_conversion_error_cm <= 0.5);
            for (i, b) in upload.heights().chunks_exact(4).enumerate() {
                let meters = f64::from(f32::from_le_bytes(b.try_into().unwrap()));
                let exact = t.heights()[(y + i / 65) * 257 + x + i % 65];
                assert!((meters * 100.0 - 1000000.0 - exact as f64).abs() <= 0.5);
            }
        }
    }
}
