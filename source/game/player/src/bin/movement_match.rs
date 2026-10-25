//! Output match for the movement physics: replays a private oracle file of
//! the executable's own results (its routines run in an emulator on a grid
//! of inputs, one case per line: `kind input-bits… = output-bits…`) through
//! `player::actor_physics` and compares every f32 bit for bit. Prints the
//! totals per routine; mismatching lines go to an optional new private file.
use player::actor_physics::{Arc, Blow, Knock, Sstm, rise_time, steer};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};

fn sstm(brake_xz: f32, min_xz: f32, brake_up: f32) -> Sstm {
    Sstm {
        fall_max: 0.,
        blow_brake_xz: brake_xz,
        blow_min_xz: min_xz,
        blow_brake_up: brake_up,
        blow_up: 0.,
        blow_speed: 0.,
        blow_to_hit_back: 0.,
        hit_back: 0.,
        hit_back_small: 0.,
        hit_back_to_jump: 0.,
    }
}

/// The rewrite's result for one case, in the oracle's output layout.
fn rewrite(kind: &str, i: &[f32]) -> Option<Vec<f32>> {
    Some(match kind {
        "rise" => vec![rise_time(i[0])],
        "launch" => {
            let a = Arc::launch(0., i[0]);
            vec![a.t, a.rise, a.base, a.span]
        }
        "arc" => {
            let mut a = Arc {
                t: i[0],
                rise: i[1],
                base: i[2],
                span: i[3],
            };
            // The executable returns the Y-down displacement.
            vec![-a.step(i[4], i[5], false, i[6])]
        }
        "steer" => steer([i[0], i[1]], [i[2], i[3]], i[4], i[5], i[6]).to_vec(),
        "knock_new" => {
            let k = Knock::new([i[0], i[1], i[2]], i[3], 0.)?;
            vec![k.direction[0], k.direction[1], k.direction[2], k.speed]
        }
        "knock_step" => {
            let mut k = Knock {
                direction: [i[0], i[1], i[2]],
                speed: i[3],
            };
            let d = k.step().unwrap_or([0.; 3]);
            vec![d[0], d[1], d[2], k.speed]
        }
        "blow_new" => vec![Blow::new([i[0], i[1], i[2]], &sstm(0., i[3], 0.)).min_xz],
        "blow_step" => {
            let mut b = Blow {
                velocity: [i[0], i[1], i[2]],
                min_xz: i[3],
            };
            let d = b.step(&sstm(i[4], 0., i[5]), i[6], i[7]);
            let v = b.velocity;
            vec![d[0], d[1], d[2], v[0], v[1], v[2]]
        }
        _ => return None,
    })
}

fn words(text: &str) -> Result<Vec<f32>, std::num::ParseIntError> {
    text.split_whitespace()
        .map(|w| u32::from_str_radix(w, 16).map(f32::from_bits))
        .collect()
}

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() || args.len() > 2 {
        return Err("usage: movement_match <oracle> [<new-private-mismatch-file>]".into());
    }
    let text = fs::read_to_string(&args[0])?;
    let mut totals: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut mismatches = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let (head, want) = line.split_once(" = ").ok_or("line without ' = '")?;
        let (kind, input) = head.split_once(' ').ok_or("line without inputs")?;
        let (input, want) = (words(input)?, words(want)?);
        let got = rewrite(kind, &input).ok_or_else(|| format!("{kind}: no result"));
        let same = got.as_ref().is_ok_and(|got| {
            got.len() == want.len()
                && got
                    .iter()
                    .zip(&want)
                    .all(|(g, w)| g.to_bits() == w.to_bits())
        });
        let entry = totals.entry(kind).or_default();
        entry.0 += 1;
        if same {
            entry.1 += 1;
        } else {
            let got = match got {
                Ok(v) => v
                    .iter()
                    .map(|g| format!("{:08x}", g.to_bits()))
                    .collect::<Vec<_>>()
                    .join(" "),
                Err(e) => e,
            };
            mismatches.push(format!("{line} | rewrite {got}"));
        }
    }
    let (mut cases, mut matched) = (0, 0);
    for (kind, (n, ok)) in &totals {
        println!("{kind}: {ok} of {n} bit-identical");
        cases += n;
        matched += ok;
    }
    println!("all: {matched} of {cases} bit-identical");
    if let Some(path) = args.get(1) {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        for m in &mismatches {
            writeln!(file, "{m}")?;
        }
    }
    Ok(mismatches.is_empty())
}

fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(2),
        Err(error) => {
            eprintln!("movement_match: {error}");
            std::process::exit(1);
        }
    }
}
