//! Private research comparison: evaluates a motion with `motion_eval` and
//! compares every model-bone world matrix against a reference dump (row-major
//! 4x4 f32 matrices, `cnt1 + cnt2` per sample, as written by the local
//! reference adapter). Inputs are private payload files; nothing is
//! distributed.
//!
//! Pass rule (owner's decision, 2026-11-02): the reference is the PS2 routine,
//! whose trig and square roots are approximations, so differences at the
//! level of that rounding count as matched. A bone matches at a sample when
//! its translation error is at most `max(0.1, 2e-3 * size)`, where `size` is
//! the largest reference joint distance from the origin at that sample
//! (coordinates of 1e6 or more, hidden bones parked far away, are left out
//! of `size`), and every rotation row differs from the reference by at most
//! 0.002 of that row's length (rows carry the joint's scale; a row shorter
//! than 1 is held to 0.002 absolute). Non-finite
//! values never match. `STRICT=1` restores the plain rule: translation error
//! at most 0.1, rotation not checked.
use data::{bar, model_pose::Skinning, motion, motion_eval};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        return Err("usage: motion_eval_compare <reference.bin> <model-original.bin> <motion.bin> <ticks-per-key-unit> <report-bones:comma|all> <tick>...".into());
    }
    let reference = std::fs::read(PathBuf::from(&args[0]))?;
    let model = std::fs::read(PathBuf::from(&args[1]))?;
    let motion_bytes = std::fs::read(PathBuf::from(&args[2]))?;
    let scale: f32 = args[3].parse()?;
    let table = bar::read_bar(&model)?;
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 4 && e.offset > 0 && e.size > 0)
        .ok_or("model entry absent")?;
    let skinning =
        Skinning::read(&model[entry.offset as usize..entry.offset as usize + entry.size as usize])?;
    let m = motion::read_interpolated(&motion_bytes)?;
    let (cnt1, cnt2) = (m.bone_count, m.total_bone_count);
    let per = 64 * (cnt1 + cnt2);
    let ticks: Vec<f32> = args[5..]
        .iter()
        .map(|t| t.parse())
        .collect::<Result<_, _>>()?;
    if reference.len() != per * ticks.len() {
        return Err(format!(
            "reference has {} bytes, expected {}",
            reference.len(),
            per * ticks.len()
        )
        .into());
    }
    let report: Vec<usize> = if args[4] == "all" {
        Vec::new()
    } else {
        args[4]
            .split(',')
            .map(|b| b.parse())
            .collect::<Result<_, _>>()?
    };
    // HELPERS=1 also compares the IK helper joints (the reference holds all
    // `total` joints in its first block).
    let compared = if std::env::var_os("HELPERS").is_some() {
        m.total_bone_count
    } else {
        cnt1
    };
    let mut worst = vec![(0f32, 0f32); compared];
    let strict = std::env::var_os("STRICT").is_some();
    let mut outside = 0usize;
    for (sample, &tick) in ticks.iter().enumerate() {
        // EXPR_CLOCK=<v> sets the expression time node's clock (the local
        // reference adapter leaves it at 0); default: the sampled frame.
        let clock = match std::env::var("EXPR_CLOCK") {
            Ok(v) => v.parse()?,
            Err(_) => tick * scale,
        };
        let e = motion_eval::evaluate_with_clock(&skinning.bones, &m, tick * scale, clock)?;
        if let Ok(list) = std::env::var("SHOW_JOINTS") {
            for j in list.split(',').filter_map(|v| v.parse::<usize>().ok()) {
                let w = e.joints[j];
                println!(
                    "tick {tick} joint {j}: rot {:?} t {:?}",
                    w.rotation, w.translation
                );
            }
        }
        // The model's size at this sample for the pass rule (module doc).
        let size = (0..compared)
            .map(|b| {
                let at = sample * per + b * 64 + 48;
                let t: Vec<f32> = reference[at..at + 12]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes(*c))
                    .collect();
                (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt()
            })
            .filter(|d| d.is_finite() && *d < 1e6)
            .fold(0f32, f32::max);
        let limit = if strict { 0.1 } else { (2e-3 * size).max(0.1) };
        let mut sample_outside = 0usize;
        let mut sample_worst = (0f32, 0usize);
        let mut sample_rotation = (0f32, 0usize);
        for (b, w) in worst.iter_mut().enumerate() {
            let at = sample * per + b * 64;
            let r: Vec<f32> = reference[at..at + 64]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
                .collect();
            let ours = e.joints[b].matrix();
            let mut dt = 0f32;
            let mut dr = 0f32;
            for row in 0..4 {
                for col in 0..4 {
                    let d = (ours[row][col] - r[row * 4 + col]).abs();
                    if row == 3 {
                        dt = dt.max(d);
                    } else {
                        dr = dr.max(d);
                    }
                }
            }
            // Rotation rows relative to the reference row's length (rows
            // shorter than 1, small or vanishing scales, absolutely).
            let row_error = (0..3)
                .map(|row| {
                    let len = (0..3)
                        .map(|col| r[row * 4 + col] * r[row * 4 + col])
                        .sum::<f32>()
                        .sqrt();
                    let d = (0..3)
                        .map(|col| (ours[row][col] - r[row * 4 + col]).abs())
                        .fold(0f32, f32::max);
                    d / len.max(1.)
                })
                .fold(0f32, |a, d| if d.is_nan() { f32::NAN } else { a.max(d) });
            if row_error.is_nan() || row_error > sample_rotation.0 {
                sample_rotation = (row_error, b);
            }
            let matched = dt <= limit && (strict || row_error <= 2e-3);
            if !matched {
                sample_outside += 1;
            }
            w.0 = w.0.max(dt);
            w.1 = w.1.max(dr);
            if dt > sample_worst.0 {
                sample_worst = (dt, b);
            }
            if report.contains(&b) {
                println!(
                    "tick {tick} bone {b}: ours t={:?} x={:?} | ref t={:?} x={:?}",
                    ours[3][..3]
                        .iter()
                        .map(|v| (v * 100.).round() / 100.)
                        .collect::<Vec<_>>(),
                    ours[0][..3]
                        .iter()
                        .map(|v| (v * 1000.).round() / 1000.)
                        .collect::<Vec<_>>(),
                    r[12..15]
                        .iter()
                        .map(|v| (v * 100.).round() / 100.)
                        .collect::<Vec<_>>(),
                    r[0..3]
                        .iter()
                        .map(|v| (v * 1000.).round() / 1000.)
                        .collect::<Vec<_>>(),
                );
            }
        }
        println!(
            "tick {tick}: worst translation error {:.4} at bone {}; worst rotation row error {:.4} at bone {}; {} bones outside the pass rule (translation limit {limit:.4})",
            sample_worst.0, sample_worst.1, sample_rotation.0, sample_rotation.1, sample_outside
        );
        if sample_outside > 0 {
            outside += 1;
        }
        if std::env::var("FIRST_DIVERGENCE").is_ok_and(|t| t.parse::<f32>().ok() == Some(tick)) {
            // Model bones in the motion's processing order, with their errors.
            for record in m.joints.bytes.as_chunks::<4>().0 {
                let j = i16::from_le_bytes([record[0], record[1]]);
                let Ok(b) = usize::try_from(j) else { continue };
                if b >= compared {
                    println!("  order helper {b} flags {}", record[2]);
                    continue;
                }
                let at = sample * per + b * 64;
                let r: Vec<f32> = reference[at..at + 64]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes(*c))
                    .collect();
                let ours = e.joints[b].matrix();
                let dt = (0..3)
                    .map(|c| (ours[3][c] - r[12 + c]).abs())
                    .fold(0., f32::max);
                let dr = (0..3)
                    .flat_map(|row| (0..3).map(move |col| (row, col)))
                    .map(|(row, col)| (ours[row][col] - r[row * 4 + col]).abs())
                    .fold(0., f32::max);
                println!("  order bone {b} flags {} dt {dt:.4} dr {dr:.4}", record[2]);
            }
        }
    }
    let mut ranked: Vec<_> = worst.iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
    println!("worst bones (translation, rotation):");
    for (b, (dt, dr)) in ranked.iter().take(25) {
        println!("  bone {b}: {dt:.4} {dr:.4}");
    }
    let total: f32 = worst.iter().map(|w| w.0).fold(0., f32::max);
    println!("max translation error over all bones/ticks: {total:.4}");
    println!(
        "pass rule ({}): {} ({outside} of {} samples outside)",
        if strict {
            "strict 0.1"
        } else {
            "owner tolerance"
        },
        if outside == 0 { "PASS" } else { "FAIL" },
        ticks.len()
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Motion comparison rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
