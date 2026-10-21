use mycity_core::air::{AirWorkspace, Input, Source, State, STRATEGY, WORKSPACE_BYTES};
use serde_json::{json, Value};
fn input(side: u16, sources: u32) -> Input {
    Input {
        schema: STRATEGY.into(),
        side,
        target: 120,
        initial_micro: 2_000_000,
        boundary_micro: [Some(2_000_000); 4],
        boundary_until: 120,
        sources: (0..sources)
            .map(|n| Source {
                id: n + 1,
                cell: n % u32::from(side).pow(2),
                start: 0,
                end: 120,
                rate_micro: 1,
            })
            .collect(),
    }
}
#[test]
fn fixed_workspace_preserves_every_source_and_cell_page_and_original_encoding() {
    let mut workspace = AirWorkspace::new().unwrap();
    let id = workspace.report().unwrap().workspace_instance;
    for side in [32, 64, 128] {
        for sources in [0, 1, 20000] {
            let input = input(side, sources);
            assert_eq!(
                input.identity(),
                mycity_core::sha256(&serde_json::to_vec(&input).unwrap())
            );
            let mut plain = State::new(input.clone()).unwrap();
            let initial = plain.checkpoint().unwrap();
            workspace.restore(&initial).unwrap();
            assert!(workspace.matches_input(0, 120, &input.identity()));
            // Value serialization reorders keys, including payload before checkpoint SHA.
            let reordered =
                serde_json::to_vec(&serde_json::from_slice::<Value>(&initial).unwrap()).unwrap();
            workspace.restore(&reordered).unwrap();
            while !plain.complete() {
                plain.page(8192, 1024).unwrap();
                workspace.page(8192, 1024).unwrap();
                let mut actual = Vec::new();
                workspace.checkpoint_to(&mut actual).unwrap();
                assert_eq!(
                    actual,
                    plain.checkpoint().unwrap(),
                    "side={side} sources={sources}"
                );
                workspace.restore(&actual).unwrap(); // Resume both Sources and Cells in the same backings.
            }
            assert!(workspace.complete());
            let report = workspace.report().unwrap();
            assert_eq!(report.workspace_instance, id);
            assert_eq!(report.payload_capacity_bytes, WORKSPACE_BYTES);
            assert_eq!(
                report.typed_backing_bytes + report.padding_bytes,
                WORKSPACE_BYTES
            );
            assert!(report.full_fill_scan_passed && report.allocation_set_stable);
            assert_eq!(report.active_cells, usize::from(side).pow(2));
            assert_eq!(report.active_sources, sources as usize);
            workspace.reset().unwrap();
            let report = workspace.report().unwrap();
            assert!(!report.loaded);
            assert_eq!(report.active_cells, 0);
            assert_eq!(report.active_sources, 0);
        }
    }
}
#[test]
fn excess_rows_are_rejected_before_decoding_the_extra_value_and_failure_refills() {
    let original = State::new(input(32, 1)).unwrap().checkpoint().unwrap();
    let mut workspace = AirWorkspace::new().unwrap();
    workspace.restore(&original).unwrap();
    let before = workspace.report().unwrap();
    for field in ["field", "scratch", "integral"] {
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        let mut rows = vec![json!(0); 16384];
        rows.push(json!({"not_a_number":0}));
        value["payload"][field] = json!(rows);
        let error = workspace
            .restore(&serde_json::to_vec(&value).unwrap())
            .unwrap_err();
        assert!(
            error.contains("air_workspace_extra_row"),
            "{field}: {error}"
        );
        let after = workspace.report().unwrap();
        assert!(!after.loaded && after.full_fill_scan_passed);
        assert_eq!(after.workspace_instance, before.workspace_instance);
        assert_eq!(after.active_cells, 0);
        assert_eq!(after.active_sources, 0);
        workspace.restore(&original).unwrap();
    }
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    let mut rows = vec![json!({"id":1,"cell":0,"start":0,"end":120,"rate_micro":1}); 20000];
    rows.push(json!({"not_a_source":0}));
    value["payload"]["input"]["sources"] = json!(rows);
    assert!(workspace
        .restore(&serde_json::to_vec(&value).unwrap())
        .unwrap_err()
        .contains("air_workspace_extra_row"));
    workspace.restore(&original).unwrap();
    assert!(workspace.report().unwrap().loaded);
}
#[test]
fn parser_limits_invalid_pages_and_output_failure_never_leave_loaded_partial_data() {
    struct Fail;
    impl std::io::Write for Fail {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("cut"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(AirWorkspace::with_budget(WORKSPACE_BYTES - 1).is_err());
    let original = State::new(input(32, 1)).unwrap().checkpoint().unwrap();
    let mut workspace = AirWorkspace::new().unwrap();
    for bad in [
        format!("{{\"schema\":\"{}\"}}", "x".repeat(129)),
        format!("{}0{}", "[".repeat(129), "]".repeat(129)),
    ] {
        assert!(workspace.restore(bad.as_bytes()).is_err());
        assert!(!workspace.report().unwrap().loaded);
    }
    workspace.restore(&original).unwrap();
    assert!(workspace.page(0, 1024).is_err());
    assert!(!workspace.report().unwrap().loaded);
    workspace.restore(&original).unwrap();
    assert!(workspace
        .checkpoint_to(&mut Fail)
        .unwrap_err()
        .contains("cut"));
    assert!(!workspace.report().unwrap().loaded);
    workspace.restore(&original).unwrap();
    let mut actual = Vec::new();
    workspace.checkpoint_to(&mut actual).unwrap();
    assert_eq!(actual, original);
}
