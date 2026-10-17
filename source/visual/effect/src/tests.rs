use super::*;
use data::object_effect::read_dpd;

fn put(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// One PDATA with one entry: a Move program (u0 = 0, u1 = 16) whose key 0
/// adds velocity (1, 2, 3); life 3 ticks, instance end 4 ticks.
fn synthetic() -> Arc<Dpd> {
    let mut b = vec![0u8; 0x300];
    put(&mut b, 0, 0x96);
    put(&mut b, 4, 1);
    put(&mut b, 8, 0x40);
    let block = 0x40 + 0x110;
    put(&mut b, block, 0x4000);
    put(&mut b, block + 0xc, 0x120);
    put(&mut b, block + 0x120, 1);
    put(&mut b, block + 0x124, 0x10);
    let e = block + 0x10;
    put(&mut b, e + 0x14, 0x3000);
    put(&mut b, e + 0x18, 0x7000_0000);
    put(&mut b, e + 0x1c, 0x7000_0000);
    put(&mut b, e + 0x20, 0xa0 + 32);
    b[e + 0x26] = 1;
    put(&mut b, e + 0x28, 36);
    b[e + 0x2c] = 16;
    b[e + 0x2e] = 1;
    put(&mut b, e + 0x30, 0x80);
    put(&mut b, e + 0x34, 0x94);
    put(&mut b, block + 0x84, 1f32.to_bits());
    put(&mut b, block + 0x88, 2f32.to_bits());
    put(&mut b, block + 0x8c, 3f32.to_bits());
    put(&mut b, block + 0x90, u32::MAX);
    put(&mut b, block + 0x94, 0);
    put(&mut b, block + 0x98, 16);
    Arc::new(read_dpd(&b).unwrap())
}

#[test]
fn ticks_at_half_rate_integrates_and_expires() {
    let mut inst = Instance::new(synthetic(), 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    let view = math::IDENTITY;
    let position = |inst: &Instance| {
        inst.parts
            .iter()
            .flatten()
            .next()
            .map(|p| [p.f32(0), p.f32(4), p.f32(8)])
    };
    // Frame 1 (odd half): the part spawns and runs its frame-0 update.
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([1., 2., 3.]));
    // Frame 2: updates skip (frame |= 1), then the tick advances.
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([1., 2., 3.]));
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([2., 4., 6.]));
    inst.frame(&view, &mut rng);
    inst.frame(&view, &mut rng);
    assert_eq!(position(&inst), Some([3., 6., 9.]));
    // After the third tick the part's frame reaches its life and it goes.
    inst.frame(&view, &mut rng);
    assert_eq!(inst.live_parts(), 0);
    assert!(!inst.finished());
    inst.frame(&view, &mut rng);
    inst.frame(&view, &mut rng);
    assert!(inst.finished());
}

#[test]
fn rng_matches_the_msvc_sequence() {
    let mut r = Rng::default();
    assert_eq!([r.rand(), r.rand(), r.rand()], [41, 18467, 6334]);
}

/// One program of a synthetic entry: type, keyframe size in bytes, the
/// keyframe words (time word first) and the work-variable offsets.
struct Prog {
    kind: u32,
    size: u16,
    keys: Vec<u32>,
    use_val: Vec<u32>,
}

fn f(v: f32) -> u32 {
    v.to_bits()
}

/// A DPD with one PDATA whose entries run `entries` (start frame, total
/// ticks, programs), the given OMD models (id, vertices) and VSFs
/// (id, model, indices).
fn build(
    entries: &[(i32, u32, Vec<Prog>)],
    models: &[(i16, Vec<[i16; 3]>)],
    vsfs: &[(i32, i32, Vec<u16>)],
) -> Arc<Dpd> {
    build_looped(entries, models, vsfs, &[])
}

/// [`build`] with loop ranges (entry, loop start, loop end in frames).
fn build_looped(
    entries: &[(i32, u32, Vec<Prog>)],
    models: &[(i16, Vec<[i16; 3]>)],
    vsfs: &[(i32, i32, Vec<u16>)],
    loops: &[(usize, i32, i32)],
) -> Arc<Dpd> {
    let mut b = vec![0u8; 0x8000];
    put(&mut b, 0, 0x96);
    // Lists: pdata, textures, shapes, models, vsf (offsets from 0x1000).
    let mut at = 4;
    let mut list = |b: &mut Vec<u8>, items: &[u32]| {
        put(b, at, items.len() as u32);
        for (i, v) in items.iter().enumerate() {
            put(b, at + 4 + 4 * i, *v);
        }
        at += 4 + 4 * items.len();
    };
    let model_at: Vec<u32> = (0..models.len())
        .map(|i| 0x1000 + 0x200 * i as u32)
        .collect();
    let vsf_at: Vec<u32> = (0..vsfs.len()).map(|i| 0x2000 + 0x100 * i as u32).collect();
    list(&mut b, &[0x40]);
    list(&mut b, &[]);
    list(&mut b, &[]);
    list(&mut b, &model_at);
    list(&mut b, &vsf_at);
    for (i, (id, v)) in models.iter().enumerate() {
        let m = model_at[i] as usize;
        b[m..m + 2].copy_from_slice(&id.to_le_bytes());
        let hdr = m + 0x10;
        put(&mut b, hdr + 8, 0x40);
        b[hdr + 0x12..hdr + 0x14].copy_from_slice(&(v.len() as i16).to_le_bytes());
        for (k, p) in v.iter().enumerate() {
            for (c, x) in p.iter().enumerate() {
                let o = hdr + 0x40 + 6 * k + 2 * c;
                b[o..o + 2].copy_from_slice(&x.to_le_bytes());
            }
        }
    }
    for (i, (id, model, idx)) in vsfs.iter().enumerate() {
        let v = vsf_at[i] as usize;
        put(&mut b, v, *id as u32);
        put(&mut b, v + 4, *model as u32);
        put(&mut b, v + 8, idx.len() as u32);
        for (k, x) in idx.iter().enumerate() {
            b[v + 12 + 2 * k..v + 14 + 2 * k].copy_from_slice(&x.to_le_bytes());
        }
    }
    for k in 0..4 {
        put(&mut b, 0x40 + 0xa0 + 4 * k, f(1.));
    }
    let block = 0x40 + 0x110;
    put(&mut b, block, 0x1000 * 64);
    put(&mut b, block + 0xc, 0x100);
    put(&mut b, block + 0x100, entries.len() as u32);
    let mut data = 0x4000;
    for (n, (start, ticks, programs)) in entries.iter().enumerate() {
        let off = 0x200 + 0x200 * n;
        put(&mut b, block + 0x100 + 4 + 4 * n, off as u32);
        let e = block + off;
        put(&mut b, e + 0x10, *start as u32);
        put(&mut b, e + 0x14, ticks * 0x1000);
        let (ls, le) = loops
            .iter()
            .find(|l| l.0 == n)
            .map_or((0x7000_0000, 0x7000_0000), |l| (l.1, l.2));
        put(&mut b, e + 0x18, ls as u32);
        put(&mut b, e + 0x1c, le as u32);
        put(&mut b, e + 0x20, 0xa0 + 0x400);
        b[e + 0x26] = programs.len() as u8;
        for (j, p) in programs.iter().enumerate() {
            let r = e + 0x28 + 0x10 * j;
            put(&mut b, r, p.kind);
            b[r + 4..r + 6].copy_from_slice(&p.size.to_le_bytes());
            b[r + 6] = 1;
            put(&mut b, r + 8, (data - block) as u32);
            for (k, w) in p.keys.iter().enumerate() {
                put(&mut b, data + 4 * k, *w);
            }
            // The word after the keyframes is the next key's time: never.
            put(&mut b, data + 4 * p.keys.len(), u32::MAX);
            data += 4 * p.keys.len() + 8;
            put(&mut b, r + 12, (data - block) as u32);
            for (k, w) in p.use_val.iter().enumerate() {
                put(&mut b, data + 4 * k, *w);
            }
            data += 4 * p.use_val.len() + 8;
        }
    }
    Arc::new(read_dpd(&b).unwrap())
}

fn colour(var: u32) -> Prog {
    // Colour var (RGBA16 at +0, bytes at +8): 0x4000 = 128 after /128.
    Prog {
        kind: 43,
        size: 12,
        keys: vec![0, 0x4000_4000, 0x4000_4000],
        use_val: vec![var],
    }
}

#[test]
fn matrix_programs_place_a_model_draw() {
    // Point (u0 = 0) at (1,2,3), unit Scale (u0 = 32), MatrixZXY with a
    // 90 degree Y angle, DrawMatrix, DrawMdl3 of model 5 (u0 = colour 48).
    let progs = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), f(2.), f(3.)],
            use_val: vec![0],
        },
        Prog {
            kind: 42,
            size: 16,
            keys: vec![0, f(1.), f(1.), f(1.)],
            use_val: vec![32],
        },
        Prog {
            kind: 41,
            size: 16,
            keys: vec![0, 0, 0x4000, 0],
            use_val: vec![16],
        },
        colour(48),
        Prog {
            kind: 91,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 32],
        },
        Prog {
            kind: 112,
            size: 4,
            keys: vec![0],
            use_val: vec![],
        },
        Prog {
            kind: 137,
            size: 16,
            keys: vec![0, 5, 0, 0],
            use_val: vec![48, 0, 64],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[(5, vec![[10, 0, 0], [0, 10, 0]])], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    inst.frame(&math::IDENTITY, &mut rng);
    assert_eq!(inst.model_draws.len(), 1);
    let d = &inst.model_draws[0];
    assert_eq!(d.model, 5);
    assert_eq!(d.rgba, [128; 4]);
    assert_eq!(&d.a[3][..3], &[1., 2., 3.]);
    // The Y quarter turn takes the model's +X axis to -Z.
    assert!(d.a[0][0].abs() < 1e-3 && (d.a[0][2] + 1.).abs() < 1e-3);
}

#[test]
fn vt_mime_morphs_between_models_and_ts3_scrolls() {
    let progs = vec![
        colour(48),
        // VtMime u0 = 0: model 0 -> 1, t += 0.5 at the key.
        Prog {
            kind: 49,
            size: 24,
            keys: vec![0, 0, 1, f(0.5), 0xff7f_ffff, 0],
            use_val: vec![0],
        },
        // Ts3: scroll block at 64, U += 2 at the key, Vvel = FLT_MAX overflows.
        Prog {
            kind: 139,
            size: 40,
            keys: vec![0, 0, f(2.), 0, 0, 0, 0, 0, 0x7f7f_ffff, 0],
            use_val: vec![48, 0, 64, 96],
        },
    ];
    let dpd = build(
        &[(0, 8, progs)],
        &[
            (0, vec![[0, 0, 0], [100, 0, 0]]),
            (1, vec![[10, 0, 0], [0, -50, 0]]),
        ],
        &[],
    );
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    inst.frame(&math::IDENTITY, &mut Rng::default());
    let d = &inst.model_draws[0];
    let v = d.vertices.as_ref().unwrap();
    assert_eq!(v[0], [5, 0, 0]);
    assert_eq!(v[1], [50, -25, 0]);
    assert_eq!(d.scroll[0], 2 * 65536);
}

#[test]
fn vertex_ap_births_children_at_model_vertices() {
    // Entry 0: VertexAp (VSF 0 = vertices 1, 0 of model 0, 1 birth every
    // tick, sequential, child entry 1, var 0). Entry 1 (spawned only by
    // births): Point at var 0.
    let emitter = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(0.), f(0.), f(0.)],
            use_val: vec![0],
        },
        Prog {
            kind: 42,
            size: 16,
            keys: vec![0, f(1.), f(1.), f(1.)],
            use_val: vec![32],
        },
        Prog {
            kind: 87,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 32],
        },
        Prog {
            kind: 174,
            size: 20,
            keys: vec![0, 0x0101_0000, 0, 1, 0],
            use_val: vec![64, 64],
        },
    ];
    let child = vec![Prog {
        kind: 40,
        size: 16,
        keys: vec![0, f(0.), f(0.), f(0.)],
        use_val: vec![0],
    }];
    let dpd = build(
        &[(0, 8, emitter), (i32::MAX, 8, child)],
        &[(0, vec![[1, 2, 3], [4, 5, 6]])],
        &[(0, 0, vec![1, 0])],
    );
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    for _ in 0..4 {
        inst.frame(&math::IDENTITY, &mut rng);
    }
    // The emitter plus at least two children (vertices 1 then 0).
    let positions: Vec<[f32; 3]> = inst
        .parts
        .iter()
        .flatten()
        .skip(1)
        .map(|p| [p.f32(0), p.f32(4), p.f32(8)])
        .collect();
    assert!(positions.contains(&[4., 5., 6.]), "{positions:?}");
    assert!(positions.contains(&[1., 2., 3.]), "{positions:?}");
}

fn point(var: u32) -> Prog {
    Prog {
        kind: 40,
        size: 16,
        keys: vec![0, f(0.), f(0.), f(0.)],
        use_val: vec![var],
    }
}

fn unit_scale(var: u32) -> Prog {
    Prog {
        kind: 42,
        size: 16,
        keys: vec![0, f(1.), f(1.), f(1.)],
        use_val: vec![var],
    }
}

#[test]
fn par_matrix_places_a_child_in_its_parents_space() {
    // The emitter sits at (10, 0, 0) and births a child at raw vertex
    // (1, 2, 3) (VertexApLc); the child's ParMatrix moves its position
    // through the parent's matrix.
    let emitter = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(10.), f(0.), f(0.)],
            use_val: vec![0],
        },
        unit_scale(32),
        Prog {
            kind: 87,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 32],
        },
        Prog {
            kind: 177,
            size: 20,
            keys: vec![0, 0x0101_0000, 0, 1, 0],
            use_val: vec![64, 64],
        },
    ];
    let child = vec![
        point(0),
        unit_scale(32),
        Prog {
            kind: 87,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 32],
        },
        Prog {
            kind: 105,
            size: 12,
            keys: vec![0, 0, 0],
            use_val: vec![],
        },
    ];
    let dpd = build(
        &[(0, 8, emitter), (i32::MAX, 8, child)],
        &[(0, vec![[1, 2, 3], [4, 5, 6]])],
        &[(0, 0, vec![0])],
    );
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    for _ in 0..4 {
        inst.frame(&math::IDENTITY, &mut rng);
    }
    // Children born on the last frame have not run their matrices yet.
    let children: Vec<_> = inst.parts.iter().flatten().skip(1).collect();
    assert!(children.iter().all(|c| c.parent.is_some()));
    assert!(children.iter().any(|c| c.cur[3][..3] == [11., 2., 3.]));
}

#[test]
fn ke_born_rnd6_gives_the_child_the_vertex_direction() {
    // One birth per tick at VSF vertex (10, 0, 0): position var 0, angle
    // var 16 gets (a0 = 0, 0, a8 = 90 degrees = 0x4000).
    let emitter = vec![
        point(0),
        unit_scale(32),
        Prog {
            kind: 184,
            size: 28,
            keys: vec![0, 0x0101_0000, 0, 1, 0, 1, 16],
            use_val: vec![64],
        },
    ];
    let child = vec![point(0), unit_scale(32)];
    let dpd = build(
        &[(0, 8, emitter), (i32::MAX, 8, child)],
        &[(0, vec![[10, 0, 0]])],
        &[(0, 0, vec![0])],
    );
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    for _ in 0..4 {
        inst.frame(&math::IDENTITY, &mut rng);
    }
    let born = inst.parts.iter().flatten().nth(1).expect("a child");
    assert_eq!([born.f32(0), born.f32(4), born.f32(8)], [10., 0., 0.]);
    assert_eq!(born.f32(12), 1.);
    assert_eq!((born.i32(16), born.i32(20), born.i32(24)), (0, 0, 0x4000));
    assert!(born.parent.is_some());
}

#[test]
fn eidrawshipoly_ribbon_follows_the_part() {
    // A part moving +1 x per tick; the ribbon keeps 8 samples, fits a spline
    // through the newest ones and emits 4 steps x 1 cross cell per frame.
    let mut ribbon = vec![0u32; 41];
    ribbon[1] = 1;
    ribbon[2] = 0x0800_0100; // +9 single, +0xb ring size 8
    ribbon[3] = f(4.); // width
    ribbon[4] = 0x0104_0000; // +0x12 4 steps, +0x13 cross 2
    ribbon[36] = f(100.); // +0x90 uv length
    let progs = vec![
        Prog {
            kind: 36,
            size: 16,
            keys: vec![0, f(1.), f(0.), f(0.)],
            use_val: vec![0, 96],
        },
        point(100),
        unit_scale(48),
        colour(60),
        Prog {
            kind: 87,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 48],
        },
        Prog {
            kind: 241,
            size: 164,
            keys: ribbon,
            use_val: vec![0, 48, 60, 120],
        },
    ];
    let dpd = build(&[(0, 40, progs)], &[(1, vec![[0, 0, 0]])], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    let mut last = Vec::new();
    for _ in 0..24 {
        last = inst.frame(&math::IDENTITY, &mut rng);
    }
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    assert_eq!(last.len(), 4);
    // The head is the newest (largest x); the path runs back along -x.
    let xs: Vec<f32> = last.iter().map(|q| q.corners[0][0]).collect();
    assert!(xs.windows(2).all(|w| w[0] >= w[1]), "{xs:?}");
    // Width: the two edges of a row are 2 x 4 apart in y.
    let q = &last[0];
    assert!((q.corners[0][1] - q.corners[1][1]).abs() > 1.);
}

fn ticks(inst: &mut Instance, rng: &mut Rng, n: usize) {
    for _ in 0..2 * n {
        inst.frame(&math::IDENTITY, rng);
    }
}

#[test]
fn loop_range_restarts_the_part_and_point_loop_does_not_drift() {
    // PointLoop u0 = 0 adds x + 1 at frame 0; the entry loops back to
    // frame 0 at frame 2 and would otherwise live 100 ticks. Each restart
    // runs the frame-0 update twice (catch-up, then the pending restart).
    let progs = vec![Prog {
        kind: 213,
        size: 16,
        keys: vec![0, f(1.), 0, 0],
        use_val: vec![0],
    }];
    let dpd = build_looped(&[(0, 100, progs)], &[], &[], &[(0, 0, 0x2000)]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    let mut seen = Vec::new();
    for _ in 0..12 {
        ticks(&mut inst, &mut rng, 1);
        let p = inst.parts.iter().flatten().next().expect("part alive");
        seen.push(p.f32(0));
    }
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    assert!(p.restarted);
    // The first key's add is taken back at each restart: x stays 1.
    assert!(seen.iter().all(|&x| x == 1.), "{seen:?}");
}

#[test]
fn loop_start_catches_the_part_up_to_the_loop_start() {
    // Move: x += 1 per tick. Loop range frames 2..4: at frame 4 the part
    // restarts and runs its updates (cursor keys included) up to frame 2.
    let progs = vec![Prog {
        kind: 36,
        size: 16,
        keys: vec![0, f(1.), 0, 0],
        use_val: vec![0, 16],
    }];
    let dpd = build_looped(&[(0, 100, progs)], &[], &[], &[(0, 0x2000, 0x4000)]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    // Frame 0 update, then ticks at frames 1, 2, 3; the fourth advance
    // reaches frame 4 and restarts.
    ticks(&mut inst, &mut rng, 4);
    let p = inst.parts.iter().flatten().next().unwrap();
    assert_eq!(p.frame, 0x2000);
    // Moves at frames 0..3 (4), the restart clears the rate (create hook),
    // the catch-up re-keys it at frame 0 and moves at frames 0 and 1.
    assert_eq!(p.f32(0), 6.);
}

#[test]
fn matrix_yxz_turns_about_y_first() {
    let m = math::rot_yxz([0x4000, 0x4000, 0]);
    let v = math::apply(&m, [1., 0., 0., 1.]);
    assert!(
        v[0].abs() < 1e-5 && (v[1] - 1.).abs() < 1e-5 && v[2].abs() < 1e-5,
        "{v:?}"
    );
}

#[test]
fn wood_matrix_keeps_the_view_up_axis() {
    assert_eq!(programs::wood(&math::IDENTITY), math::IDENTITY);
    // The effect rolled 90 degrees about the view axis: Y now points along
    // -X; the billboard keeps that up and stays orthonormal.
    let w = programs::wood(&math::rot_z(0x4000));
    let up = w[1];
    assert!((up[0] + 1.).abs() < 1e-5 && up[1].abs() < 1e-5);
    for col in &w[..3] {
        let l: f32 = col[..3].iter().map(|x| x * x).sum();
        assert!((l - 1.).abs() < 1e-5);
    }
    let dot: f32 = (0..3).map(|i| w[0][i] * w[1][i]).sum();
    assert!(dot.abs() < 1e-5);
}

#[test]
fn s_rand_down_fv_only_subtracts() {
    // Target var 16 gains k·r with r in [-1, 0] for k = (10, 20, 30).
    let progs = vec![Prog {
        kind: 77,
        size: 24,
        keys: vec![0, 16, f(10.), f(20.), f(30.), 0],
        use_val: vec![0],
    }];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 1);
    let p = inst.parts.iter().flatten().next().unwrap();
    for (i, k) in [10., 20., 30.].into_iter().enumerate() {
        let r = p.f32(4 * i);
        assert!((-1. ..=0.).contains(&r));
        assert_eq!(p.f32(16 + 4 * i), k * r);
    }
}

#[test]
fn ke_born_rnd3_births_with_the_outward_direction() {
    // Emitter at (5, 0, 0) (MatrixLoc), VSF vertex (10, 0, 0): the child
    // starts at (15, 0, 0) with direction (1, 0, 0) in var 16.
    let emitter = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(5.), f(0.), f(0.)],
            use_val: vec![0],
        },
        Prog {
            kind: 93,
            size: 4,
            keys: vec![0],
            use_val: vec![0],
        },
        Prog {
            kind: 181,
            size: 28,
            // VSF 0, 1 birth, mask 0 (always), sequential; child 1,
            // position var 0, +0x14 set, direction var 16.
            keys: vec![0, 0x0001_0000, 0, 1, 0, 0, 16],
            use_val: vec![64],
        },
    ];
    let child = vec![point(0), point(16)];
    let dpd = build(
        &[(0, 8, emitter), (i32::MAX, 8, child)],
        &[(0, vec![[10, 0, 0]])],
        &[(0, 0, vec![0])],
    );
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 2);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let born = inst.parts.iter().flatten().nth(1).expect("a child");
    assert_eq!([born.f32(0), born.f32(4), born.f32(8)], [15., 0., 0.]);
    assert_eq!([born.f32(16), born.f32(20), born.f32(24)], [1., 0., 0.]);
}

#[test]
fn point_ap_births_copy_the_position() {
    let emitter = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), f(2.), f(3.)],
            use_val: vec![0],
        },
        // Child entry 1, var 32, every 2 ticks 3 births.
        Prog {
            kind: 172,
            size: 16,
            keys: vec![0, 1, 32, 0x0000_0302],
            use_val: vec![0, 64],
        },
    ];
    let child = vec![point(0)];
    let dpd = build(&[(0, 8, emitter), (i32::MAX, 8, child)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 3);
    let children: Vec<_> = inst.parts.iter().flatten().skip(1).collect();
    assert_eq!(children.len(), 6);
    assert!(
        children
            .iter()
            .all(|c| [c.f32(32), c.f32(36), c.f32(40)] == [1., 2., 3.])
    );
}

#[test]
fn draw_mdl_loop_z_scrolls_and_wraps() {
    // U starts at 1.0 with rate 0.5 per tick; V starts at 32767.5 with rate
    // 1 and wraps at 32768.
    let progs = vec![
        colour(48),
        Prog {
            kind: 219,
            size: 40,
            keys: vec![0, 5, f(1.), f(0.5), 0, f(32767.5), f(1.), 0, 0xffff, 0],
            use_val: vec![48, 0, 64, 96],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[(5, vec![[0, 0, 0]])], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    inst.frame(&math::IDENTITY, &mut rng);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let d = &inst.model_draws[0];
    assert_eq!(d.model, 5);
    assert_eq!(d.mode, None);
    assert_eq!(d.scroll, [(1.5 * 65536.) as i32, (0.5 * 65536.) as i32]);
}

#[test]
fn shape_tail_ring_follows_the_part() {
    // Move +1 x per tick; KeShpTail3X (no shape: only the ring runs) keeps
    // the newest translation one slot back each tick.
    let mut tail = vec![0u32; 0x58 / 4];
    tail[1] = 0xffff;
    let progs = vec![
        Prog {
            kind: 36,
            size: 16,
            keys: vec![0, f(1.), 0, 0],
            use_val: vec![0, 16],
        },
        unit_scale(32),
        Prog {
            kind: 93,
            size: 4,
            keys: vec![0],
            use_val: vec![0],
        },
        Prog {
            kind: 157,
            size: 0x58,
            keys: tail,
            use_val: vec![64, 48, 0, 0],
        },
    ];
    let dpd = build(&[(0, 40, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 3);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    let ring = |i: usize| p.f32(64 + 0x30 + 16 * i);
    let head = p.u8(64 + 0x1fa) as usize;
    assert_eq!(head, 28 - 3);
    // Newest first: x = 3, 2, 1; the slots behind keep the frame-0 fill.
    assert_eq!([ring(head), ring(head + 1), ring(head + 2)], [3., 2., 1.]);
    assert_eq!(ring(0), 1.);
}

#[test]
fn ribbon_drift_moves_older_samples_and_edge_fade_shades_the_width() {
    // As eidrawshipoly_ribbon_follows_the_part, with +0x1d drift (y + 3
    // per tick), +0x16 edge fade and U/V swapped (+0xa0 = 2).
    let mut ribbon = vec![0u32; 41];
    ribbon[1] = 1;
    ribbon[2] = 0x0800_0100;
    ribbon[3] = f(4.);
    ribbon[4] = 0x0204_0000; // +0x12 4 steps, +0x13 cross 3
    ribbon[5] = 0x0001_0000; // +0x16 edge fade
    ribbon[7] = 0x0000_0100; // +0x1d drift
    ribbon[15] = f(3.); // +0x3c drift velocity y
    ribbon[40] = 2; // +0xa0 swap
    let progs = vec![
        Prog {
            kind: 36,
            size: 16,
            keys: vec![0, f(1.), f(0.), f(0.)],
            use_val: vec![0, 96],
        },
        point(100),
        unit_scale(48),
        colour(60),
        Prog {
            kind: 87,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 48],
        },
        Prog {
            kind: 241,
            size: 164,
            keys: ribbon,
            use_val: vec![0, 48, 60, 120],
        },
    ];
    let dpd = build(&[(0, 40, progs)], &[(1, vec![[0, 0, 0]])], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    let mut last = Vec::new();
    for _ in 0..12 {
        last = inst.frame(&math::IDENTITY, &mut rng);
    }
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    // 4 steps × 2 cells across.
    assert_eq!(last.len(), 8);
    // Older samples drifted up (+y): the tail end sits above the head.
    let head_y = last[0].corners[0][1];
    let tail_y = last[6].corners[2][1];
    assert!(tail_y > head_y + 5., "{head_y} {tail_y}");
    // Edge fade: both cells of a row average one edge (0) and the centre
    // (1) sample: half alpha.
    assert!((last[0].colour[3] - last[1].colour[3]).abs() < 1e-6);
    assert!(last[0].colour[3] < 0.6);
    // Swapped UVs: the across-width coordinate is now the first one.
    assert_ne!(last[0].uv[0][0], last[0].uv[1][0]);
}

#[test]
fn ke_th_bolt_grows_to_its_target_and_draws_a_strip() {
    // Entry 0: emitter at the origin with a 16-point bolt pool (var 0),
    // KeDrct +x (var 0x380) and KeTh (block 0x300) aiming at entry 1's
    // KeThTp block (var 0), which sits at (100, 0, 0) with radius 5.
    let mut th = vec![0u32; 23];
    th[1] = 1; // target entry 1
    th[2] = 0; // its KeThTp variable
    for w in &mut th[3..7] {
        *w = 0x4000_4000; // colours A and B: 128
    }
    th[11] = f(100.); // length
    th[12] = f(500.); // half width (screen units)
    th[13] = f(1.); // no taper
    th[14] = f(0.5); // inner fraction
    th[17] = f(0.5); // follow
    th[18] = f(20.); // growth
    th[19] = f(0.5); // pull
    th[20] = 0xff00_0000; // no turn, no forks (mask 0xff)
    let emitter = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(0.), f(0.), f(0.)],
            use_val: vec![0x3c0],
        },
        Prog {
            kind: 93,
            size: 4,
            keys: vec![0],
            use_val: vec![0x3c0],
        },
        Prog {
            kind: 6,
            size: 4,
            keys: vec![0],
            use_val: vec![0],
        },
        Prog {
            kind: 48,
            size: 16,
            keys: vec![0, f(1.), f(0.), f(0.)],
            use_val: vec![0x380],
        },
        Prog {
            kind: 129,
            size: 92,
            keys: th,
            use_val: vec![0x300, 0, 0x380, 0],
        },
    ];
    let target = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(100.), f(0.), f(0.)],
            use_val: vec![64],
        },
        Prog {
            kind: 93,
            size: 4,
            keys: vec![0],
            use_val: vec![64],
        },
        Prog {
            kind: 126,
            size: 24,
            keys: vec![0, f(5.), f(30.), f(0.5), u32::MAX, 0],
            use_val: vec![0],
        },
    ];
    let dpd = build(&[(0, 40, emitter), (0, 40, target)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut view = math::IDENTITY;
    view[3] = [0., 0., 600., 1.];
    let mut rng = Rng::default();
    let mut tris = Vec::new();
    for _ in 0..24 {
        inst.frame(&view, &mut rng);
        if !inst.triangles.is_empty() {
            tris = inst.triangles.clone();
        }
    }
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    assert!(!tris.is_empty());
    let corners: Vec<[f32; 3]> = tris.iter().flat_map(|t| t.corners).collect();
    assert!(corners.iter().flatten().all(|c| c.is_finite()));
    // The bolt runs from the emitter toward the target along +x.
    let far = corners.iter().map(|c| c[0]).fold(f32::MIN, f32::max);
    assert!(far > 50. && far < 110., "{far}");
    // Width: 500 screen units = 500 / scale view units each side.
    let wide = corners.iter().map(|c| c[1].abs()).fold(0., f32::max);
    assert!(
        (wide - 500. / DEFAULT_SCREEN_SCALE[1]).abs() < 0.05,
        "{wide}"
    );
    // Additive blend, transparent edges, opaque axis.
    assert!(tris.iter().all(|t| t.blend == 0x48));
    assert!(tris.iter().flat_map(|t| t.colours).any(|c| c[3] == 0));
    assert!(tris.iter().flat_map(|t| t.colours).any(|c| c == [128; 4]));
}

#[test]
fn matrix_yzx_applies_y_then_z_then_x() {
    // Angles (X, Y, Z) = (90°, 90°, 0) in u1: Y first leaves +Y alone, then
    // X takes it to +Z (pppMatrixXYZ would give +X). Scales are 1 (u2).
    let progs = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), f(1.), f(1.)],
            use_val: vec![32],
        },
        Prog {
            kind: 41,
            size: 16,
            keys: vec![0, 0x4000, 0x4000, 0],
            use_val: vec![16],
        },
        Prog {
            kind: 89,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16, 32],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 1);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    let y = p.cur[1];
    assert!(
        y[0].abs() < 1e-5 && y[1].abs() < 1e-5 && (y[2] - 1.).abs() < 1e-5,
        "{y:?}"
    );
    let v = math::apply(&math::rot_yzx([0x4000, 0x4000, 0]), [0., 1., 0., 1.]);
    assert!(v[2] > 0.99, "{v:?}");
}

#[test]
fn rigid_inverse_undoes_a_rotated_translation() {
    let mut m = math::rot_z(0x2000);
    m[3] = [1., 2., 3., 1.];
    let p = math::mul(&math::rigid_inverse(&m), &m);
    for (c, col) in p.iter().enumerate() {
        for (r, x) in col.iter().enumerate() {
            let e = if c == r { 1. } else { 0. };
            assert!((x - e).abs() < 1e-5, "{p:?}");
        }
    }
}

#[test]
fn ke_angle_is_atan2_with_the_exe_branches() {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
    let a = |p0: f32, p1: f32| programs::ke_angle(p0, p1).unwrap();
    assert!(a(1., 0.).abs() < 1e-6);
    assert!((a(0., 1.) - FRAC_PI_2).abs() < 1e-6);
    assert!((a(0., -1.) + FRAC_PI_2).abs() < 1e-6);
    assert!((a(-1., 0.) - PI).abs() < 1e-6);
    assert!((a(-1., 1.) - 3. * FRAC_PI_4).abs() < 1e-5);
    assert!((a(-1., -1.) + 3. * FRAC_PI_4).abs() < 1e-5);
    assert!(programs::ke_angle(0., 0.).is_none());
}

#[test]
fn rand_float_and_rand_down_fv_scale_their_draws() {
    // RandFloat (u0 = 0): var 16 gains k·r − k, r in [0, 2), k = 10.
    // RandDownFV (u0 = 4): vars 32.. gain k_i·r, r in [−1, 0].
    let progs = vec![
        Prog {
            kind: 51,
            size: 16,
            keys: vec![0, 16, f(10.), 0],
            use_val: vec![0],
        },
        Prog {
            kind: 65,
            size: 24,
            keys: vec![0, 32, f(10.), f(20.), f(30.), 0],
            use_val: vec![4],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 1);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    let r = p.f32(0);
    assert!((0. ..2.).contains(&r), "{r}");
    assert_eq!(p.f32(16), 10. * r - 10.);
    let d = p.f32(4);
    assert!((-1. ..=0.).contains(&d), "{d}");
    for (i, k) in [10., 20., 30.].into_iter().enumerate() {
        assert_eq!(p.f32(32 + 4 * i), k * d);
    }
}

#[test]
fn ke_mv_yp_eff_turns_a_velocity_into_pitch_and_yaw() {
    // Velocity (1, 0, 1): pitch atan2(−1, 1) = −45° (−8192 in 0x10000 turns),
    // yaw atan2(1, −0) = 90° (16384).
    let progs = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), 0, f(1.)],
            use_val: vec![16],
        },
        Prog {
            kind: 47,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 1);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    assert!((p.i32(0) + 8192).abs() <= 2, "{}", p.i32(0));
    assert!((p.i32(8) - 16384).abs() <= 2, "{}", p.i32(8));
}

#[test]
fn ke_mat_sn_turns_by_the_change_of_its_angles() {
    // Angle (u1 = 16) adds 0x100 about X at frame 0; pppKeMatSN then turns
    // the matrix at u0+0x40 by old − new = −0x100 about X (u0 = 0).
    let progs = vec![
        Prog {
            kind: 41,
            size: 16,
            keys: vec![0, 0x100, 0, 0],
            use_val: vec![16],
        },
        Prog {
            kind: 85,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 1);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    assert_eq!(p.i32(0x80), 0x100);
    let m = programs::read_matrix(p, 0x40);
    let want = math::rot_xyz([-0x100, 0, 0]);
    for c in 0..4 {
        for r in 0..4 {
            assert!((m[c][r] - want[c][r]).abs() < 1e-5, "{m:?}");
        }
    }
}

#[test]
fn ang_move_loop_moves_no_value_during_a_loop_catch_up() {
    // The loop-start test's Move with pppAngMoveLoop (ints): the restart's
    // catch-up re-keys the rate but does not move the value, so 4 normal
    // ticks give 4 where a plain Move gives 6.
    let progs = vec![Prog {
        kind: 211,
        size: 16,
        keys: vec![0, 1, 0, 0],
        use_val: vec![0, 16],
    }];
    let dpd = build_looped(&[(0, 100, progs)], &[], &[], &[(0, 0x2000, 0x4000)]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    ticks(&mut inst, &mut rng, 4);
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    assert_eq!(p.frame, 0x2000);
    assert_eq!(p.i32(0), 4);
}

#[test]
fn matrix_no_rot_is_the_view_inverse_with_its_own_axes() {
    // View = translation (5, 0, 0): its inverse is (−5, 0, 0), but the
    // translation u0 = (1, 2, 3) replaces it; axes scaled by u1 = (1, 1, 1).
    let progs = vec![
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), f(2.), f(3.)],
            use_val: vec![0],
        },
        Prog {
            kind: 40,
            size: 16,
            keys: vec![0, f(1.), f(1.), f(1.)],
            use_val: vec![16],
        },
        Prog {
            kind: 96,
            size: 4,
            keys: vec![0],
            use_val: vec![0, 16],
        },
    ];
    let dpd = build(&[(0, 8, progs)], &[], &[]);
    let mut inst = Instance::new(dpd, 0, math::IDENTITY, 0.).unwrap();
    let mut rng = Rng::default();
    for _ in 0..2 {
        inst.frame(&VIEW_TRANSLATE, &mut rng);
    }
    assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
    let p = inst.parts.iter().flatten().next().unwrap();
    let want = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [1., 2., 3., 1.],
    ];
    assert_eq!(p.cur, want);
}

/// A view that is a translation by (5, 0, 0).
const VIEW_TRANSLATE: math::Mat = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [5., 0., 0., 1.],
];

#[test]
fn world_draw_matrices_put_the_part_at_its_origin_plus_its_offset() {
    // Instance at (10, 20, 30) (identity axes); MatrixLoc puts the cur
    // translation at (1, 2, 3). Both draw matrices give the part (11, 22, 33).
    for kind in [121, 123] {
        let progs = vec![
            Prog {
                kind: 40,
                size: 16,
                keys: vec![0, f(1.), f(2.), f(3.)],
                use_val: vec![48],
            },
            Prog {
                kind: 93,
                size: 4,
                keys: vec![0],
                use_val: vec![48],
            },
            Prog {
                kind,
                size: 4,
                keys: vec![0],
                use_val: vec![0, 16],
            },
        ];
        let dpd = build(&[(0, 8, progs)], &[], &[]);
        let world = instance_matrix([10., 20., 30.], [0.; 3], [1.; 3]);
        let mut inst = Instance::new(dpd, 0, world, 0.).unwrap();
        let mut rng = Rng::default();
        ticks(&mut inst, &mut rng, 1);
        assert!(inst.unsupported.is_empty(), "{:?}", inst.unsupported);
        let p = inst.parts.iter().flatten().next().unwrap();
        for c in 0..3 {
            for r in 0..4 {
                let e = if c == r { 1. } else { 0. };
                assert!((p.draw[c][r] - e).abs() < 1e-5, "{kind}: {:?}", p.draw);
            }
        }
        assert_eq!(p.draw[3], [11., 22., 33., 1.], "{kind}");
    }
}

#[test]
fn a_full_particle_heap_makes_no_parts() {
    let dpd = synthetic();
    let mut open = Instance::new(dpd.clone(), 0, IDENTITY, 0.).unwrap();
    let mut full = Instance::new(dpd, 0, IDENTITY, 0.).unwrap();
    full.heap_limit = Some(0);
    let mut rng = Rng::default();
    open.frame(&IDENTITY, &mut rng);
    full.frame(&IDENTITY, &mut rng);
    // One part: its work block and cursor words rounded to 16, plus the
    // block header.
    assert_eq!(open.live_parts(), 1);
    assert_eq!(open.heap_used() % 16, 0);
    assert_eq!(full.live_parts(), 0);
}
