use mycity_core::air::{Input, Phase, Source, State, OWNED_BYTES, STRATEGY};
use serde_json::{json, Value};
fn input(target: u32) -> Input {
    Input {
        schema: STRATEGY.into(),
        side: 32,
        target,
        initial_micro: 2_000_000,
        boundary_micro: [Some(2_000_000); 4],
        boundary_until: target,
        sources: vec![],
    }
}
fn finish(s: &mut State, source: u32, cells: u32) {
    while !s.complete() {
        s.page(source, cells).unwrap();
    }
}
// Independent unpaged f64 stencil on a padded grid, with precomputed event anchors.
fn reference(i: &Input) -> Vec<f64> {
    let n = usize::from(i.side);
    let pitch = n + 2;
    let mut field = vec![i.initial_micro as f64 / 1e6; pitch * pitch];
    for x in 1..=n {
        field[x] = i.boundary_micro[0].unwrap() as f64 / 1e6;
        field[(n + 1) * pitch + x] = i.boundary_micro[1].unwrap() as f64 / 1e6;
    }
    for y in 1..=n {
        field[y * pitch] = i.boundary_micro[3].unwrap() as f64 / 1e6;
        field[y * pitch + n + 1] = i.boundary_micro[2].unwrap() as f64 / 1e6;
    }
    let mut anchors = std::collections::BTreeSet::from([0, i.target]);
    for t in (60..i.target).step_by(60) {
        anchors.insert(t);
    }
    for s in &i.sources {
        anchors.insert(s.start);
        anchors.insert(s.end);
    }
    let mut at = 0;
    for end in anchors.into_iter().filter(|t| *t > 0) {
        let h = f64::from(end - at) / 60.0;
        let r = 0.02 * h;
        let d = 0.01 * h;
        let mut next = field.clone();
        for y in 1..=n {
            for x in 1..=n {
                let p = y * pitch + x;
                next[p] = (1.0 - d)
                    * ((1.0 - 4.0 * r) * field[p]
                        + r * (field[p - pitch] + field[p + pitch] + field[p + 1] + field[p - 1]))
                    + 2.0 * d;
            }
        }
        for s in &i.sources {
            let seconds = end.min(s.end).saturating_sub(at.max(s.start));
            let cell = s.cell as usize;
            let p = (cell / n + 1) * pitch + cell % n + 1;
            next[p] += f64::from(s.rate_micro) * f64::from(seconds) / 1e6;
        }
        field = next;
        at = end;
    }
    (0..n * n)
        .map(|p| field[(p / n + 1) * pitch + p % n + 1])
        .collect()
}
#[test]
fn constant_background_is_exact_for_all_map_sizes_and_full_day_integral() {
    for side in [32, 64, 128] {
        let mut i = input(86400);
        i.side = side;
        let mut s = State::new(i).unwrap();
        finish(&mut s, 8192, 1024);
        assert!(s.field_bits().iter().all(|b| *b == 2f32.to_bits()));
        assert!(s.integral_bits().iter().all(|b| *b == 172800f64.to_bits()));
        let r = s.report();
        assert_eq!(r.steps, 1440);
        assert_eq!(r.boundary_exchange, 0.0);
        assert_eq!(r.emitted_micro, 0);
    }
}
#[test]
fn independent_f64_single_step_day_and_fixed_thirty_day_window() {
    for target in [60, 86400, 30 * 86400] {
        let mut i = input(target);
        i.initial_micro = 10_000_000;
        i.boundary_micro = [
            Some(1_000_000),
            Some(3_000_000),
            Some(4_000_000),
            Some(2_000_000),
        ];
        i.sources = vec![
            Source {
                id: 1,
                cell: 528,
                start: 0,
                end: target / 2,
                rate_micro: 10000,
            },
            Source {
                id: 2,
                cell: 529,
                start: target / 2,
                end: target,
                rate_micro: 2500,
            },
        ];
        let expected = reference(&i);
        let mut s = State::new(i).unwrap();
        finish(&mut s, 7, 113);
        let mut worst = 0.0f64;
        for (b, r) in s.field_bits().iter().zip(expected) {
            let error = (f64::from(f32::from_bits(*b)) - r).abs();
            worst = worst.max(error);
            assert!(
                error <= 0.001 + 0.0001 * r.abs(),
                "{target}: {error} vs {r}"
            );
        }
        let r = s.report();
        let closure = r.sum
            - (10240.0 + r.emitted_micro as f64 / 1e6 - r.decay
                + r.background
                + r.boundary_exchange
                + r.rounding_residual);
        assert!(closure.abs() < 1e-5, "closure={closure}");
        println!(
            "AIR_F64 {}",
            json!({"game_seconds":target,"max_absolute_error":worst,"source_micro":r.emitted_micro,"numeric_ledger_residual":closure})
        );
    }
}
fn dense(side: u16) -> Input {
    let mut i = input(120);
    i.side = side;
    i.sources = (1..=20000)
        .map(|id| Source {
            id,
            cell: id % u32::from(side).pow(2),
            start: if id % 2 == 0 { 37 } else { 0 },
            end: 83,
            rate_micro: id % 37 + 1,
        })
        .collect();
    i
}
#[test]
fn dense_source_pages_and_mid_source_mid_tile_checkpoints_are_bit_exact() {
    let i = dense(128);
    let mut baseline = State::new(i.clone()).unwrap();
    finish(&mut baseline, 8192, 1024);
    let mut paged = State::new(i.clone()).unwrap();
    paged.page(13, 17).unwrap();
    assert_eq!(paged.progress().unwrap().phase, Phase::Sources);
    let cp: Value = serde_json::from_slice(&paged.checkpoint().unwrap()).unwrap();
    assert!(cp["payload"]["scratch"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_u64().unwrap() != 0));
    paged = State::restore_for(&paged.checkpoint().unwrap(), &i).unwrap();
    while paged.progress().is_some_and(|p| p.phase == Phase::Sources) {
        paged.page(8192, 17).unwrap();
    }
    paged.page(8192, 17).unwrap();
    assert_eq!(paged.progress().unwrap().cursor, 17);
    paged = State::restore_for(&paged.checkpoint().unwrap(), &i).unwrap();
    finish(&mut paged, 611, 127);
    assert_eq!(paged, baseline);
    let expected: i64 = i
        .sources
        .iter()
        .map(|s| i64::from(s.rate_micro) * i64::from(s.end - s.start))
        .sum();
    assert_eq!(paged.report().emitted_micro, expected as u64);
    assert!(paged.owned_bytes() <= OWNED_BYTES);
    println!(
        "AIR_DENSE {}",
        json!({"sources":i.sources.len(),"owned_bytes":paged.owned_bytes(),"source_struct_bytes":std::mem::size_of::<Source>(),"checkpoint_bytes":paged.checkpoint().unwrap().len(),"state":paged.report()})
    );
}
#[test]
fn partial_candidate_never_publishes_field_integral_time_or_diagnostics() {
    let mut i = input(60);
    i.sources = vec![Source {
        id: 1,
        cell: 528,
        start: 0,
        end: 60,
        rate_micro: 10000,
    }];
    let mut s = State::new(i).unwrap();
    let before = s.report();
    s.page(1, 1).unwrap();
    s.page(1, 1).unwrap();
    let r = s.report();
    assert_eq!(r.time, 0);
    assert_eq!(r.field_sha256, before.field_sha256);
    assert_eq!(r.integral_sha256, before.integral_sha256);
    assert_eq!(r.emitted_micro, 0);
    finish(&mut s, 1, 1);
    assert!(s.report().maximum > 2.0);
    assert!(s.field_bits().iter().all(|b| f32::from_bits(*b) >= 0.0));
}
#[test]
fn checksum_stale_identity_invalid_cursor_and_nonfinite_restore_are_rejected() {
    let i = input(60);
    let mut s = State::new(i.clone()).unwrap();
    s.page(1, 1).unwrap();
    let cp = s.checkpoint().unwrap();
    let mut damaged = cp.clone();
    damaged[10] ^= 1;
    assert!(State::restore(&damaged).is_err());
    let mut stale = i;
    stale.initial_micro += 1;
    assert!(State::restore_for(&cp, &stale).is_err());
    for mutation in ["cursor", "nan", "counter"] {
        let mut v: Value = serde_json::from_slice(&cp).unwrap();
        if mutation == "cursor" {
            v["payload"]["progress"]["cursor"] = json!(2000000);
        } else if mutation == "nan" {
            v["payload"]["field"][0] = json!(f32::NAN.to_bits());
        } else {
            v["payload"]["totals"]["emitted_micro"] = json!(u64::MAX);
        }
        // Even direct public deserialization cannot construct an invalid task state.
        assert!(serde_json::from_value::<State>(v["payload"].clone()).is_err());
        assert!(State::restore(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn unknown_boundary_bad_sources_and_limits_do_not_silently_substitute_zero() {
    let base = input(60);
    for mode in 0..7 {
        let mut i = base.clone();
        match mode {
            0 => i.boundary_micro[0] = None,
            1 => i.boundary_until = 59,
            2 => i.side = 256,
            3 => i.target = 30 * 86400 + 1,
            4 => {
                i.sources = vec![Source {
                    id: 1,
                    cell: 1024,
                    start: 0,
                    end: 60,
                    rate_micro: 1,
                }]
            }
            5 => {
                i.sources = vec![Source {
                    id: 1,
                    cell: 0,
                    start: 0,
                    end: 60,
                    rate_micro: 0,
                }]
            }
            _ => i.initial_micro = u64::MAX,
        };
        assert!(State::new(i).is_err());
    }
    let mut s = State::new(base).unwrap();
    let old = s.clone();
    assert!(s.page(8193, 1).is_err());
    assert!(s.page(1, 1025).is_err());
    assert_eq!(s, old);
    assert!(Input::parse(&vec![b' '; 4 * 1024 * 1024 + 1]).is_err());
}
#[test]
fn pulse_spreads_from_a_single_cell_on_later_steps() {
    let mut i = input(120);
    i.sources = vec![Source {
        id: 1,
        cell: 528,
        start: 0,
        end: 60,
        rate_micro: 1_000_000,
    }];
    let mut s = State::new(i).unwrap();
    while s.report().time < 60 {
        s.page(8192, 1024).unwrap();
    }
    assert_eq!(f32::from_bits(s.field_bits()[529]), 2.0);
    finish(&mut s, 8192, 1024);
    assert!(f32::from_bits(s.field_bits()[529]) > 2.0);
    assert!(f32::from_bits(s.field_bits()[528]) < 62.0);
}

#[test]
fn decoder_rejects_excess_rows_before_accepting_source_or_checkpoint_vectors() {
    let mut i = input(60);
    i.sources = (1..=20001)
        .map(|id| Source {
            id,
            cell: 0,
            start: 0,
            end: 60,
            rate_micro: 1,
        })
        .collect();
    let err = Input::parse(&serde_json::to_vec(&i).unwrap()).unwrap_err();
    assert!(err.contains("air_rows"), "{err}");
    let s = State::new(input(60)).unwrap();
    for field in ["field", "scratch", "integral"] {
        let mut v: Value = serde_json::from_slice(&s.checkpoint().unwrap()).unwrap();
        v["payload"][field] = json!(vec![0u32; 16385]);
        let err = State::restore(&serde_json::to_vec(&v).unwrap()).unwrap_err();
        assert!(err.contains("air_rows"), "{field}: {err}");
    }
}

#[test]
fn continuous_windows_preserve_field_integral_and_source_ledger() {
    let mut first = input(60);
    first.sources.push(Source {
        id: 1,
        cell: 528,
        start: 0,
        end: 60,
        rate_micro: 10000,
    });
    let mut full = first.clone();
    full.target = 180;
    full.boundary_until = 180;
    let mut direct = State::new(full).unwrap();
    finish(&mut direct, 8192, 1024);
    let mut state = State::new(first).unwrap();
    finish(&mut state, 1, 17);
    let field = state.field_bits().to_vec();
    let integral = state.integral_bits().to_vec();
    let mut next = input(180);
    next.sources.clear();
    let mut continued = state.continue_from_boundary(next.clone()).unwrap();
    assert_eq!(continued.field_bits(), field);
    assert_eq!(continued.integral_bits(), integral);
    assert_eq!(continued.report().emitted_micro, 600000);
    continued = State::restore(&continued.checkpoint().unwrap()).unwrap();
    finish(&mut continued, 7, 111);
    assert_eq!(continued.field_bits(), direct.field_bits());
    assert_eq!(continued.integral_bits(), direct.integral_bits());
    assert_eq!(
        continued.report().emitted_micro,
        direct.report().emitted_micro
    );
    assert!(continued.field_bits()[528] > 2f32.to_bits());
    let before = state.checkpoint().unwrap();
    let mut bad = next.clone();
    bad.side = 64;
    assert!(state.continue_from_boundary(bad).is_err());
    let mut bad = next.clone();
    bad.boundary_micro[0] = Some(0);
    assert!(state.continue_from_boundary(bad).is_err());
    let mut bad = next.clone();
    bad.sources.push(Source {
        id: 1,
        cell: 1,
        start: 59,
        end: 61,
        rate_micro: 1,
    });
    assert!(state.continue_from_boundary(bad).is_err());
    let unfinished = State::new(next).unwrap();
    assert!(unfinished.continue_from_boundary(input(200)).is_err());
    assert!(state.continue_from_boundary(input(60)).is_err());
    assert_eq!(before, state.checkpoint().unwrap());
}
