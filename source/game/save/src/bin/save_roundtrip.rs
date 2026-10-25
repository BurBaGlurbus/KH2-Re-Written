//! Byte-match check for the save code on a container the original game
//! wrote. Both files are only read. For the container and every save image
//! in it, checks that:
//!
//! - the header mask made from the Steam id and the executable's S-box is the
//!   one the file was written with;
//! - `Container::open` then `to_png` gives the file again;
//! - the image's stored checksum is the one `checksum` computes;
//! - `Persistent::read` then `write` gives the image again;
//! - after every byte `write` sets is overwritten with junk, `write` still
//!   restores the image from the values read alone.
//!
//! Totals are printed; per-image results go to a new private report.
use save::{
    cipher::{Sbox, key_from_steam_id, mask},
    container::Container,
    data::SaveData,
    state::Persistent,
};
use std::{fs, io::Write, path::PathBuf};

/// The bytes `write` sets, whatever the image held before: the result is
/// the same over an all-zero and an all-0xff image.
fn written_bytes(p: &Persistent) -> Vec<bool> {
    let fill = |b: u8| {
        let mut image = SaveData::blank();
        let len = image.bytes().len();
        image.set_slice(0, &vec![b; len]);
        p.write(&mut image);
        image.bytes().to_vec()
    };
    let (zeros, ones) = (fill(0), fill(0xff));
    let mut set: Vec<bool> = zeros.iter().zip(&ones).map(|(a, b)| a == b).collect();
    // The checksum is resealed from the whole image, not from `p`.
    set[8..12].fill(false);
    set
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("usage: save_roundtrip <install-root> <Steam\\<id>\\KHIIFM_WW.png> <new-private-report>".into());
    }
    let report = &args[2];
    let parent = fs::canonicalize(report.parent().ok_or("report needs a folder")?)?;
    if parent.ancestors().any(|dir| dir.join(".git").exists()) {
        return Err("the report must be outside Git checkouts".into());
    }
    let steam_id: u64 = args[1]
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .and_then(|n| n.parse().ok())
        .ok_or("the container must sit in its Steam id folder")?;
    let bytes = fs::read(&args[1])?;
    let key = key_from_steam_id(steam_id);
    let sbox = Sbox::from_executable(&fs::read(args[0].join("KINGDOM HEARTS II FINAL MIX.exe"))?)?;
    let mask_matches = Container::recover_mask(&bytes) == Some(mask(&key, &sbox));
    let container = Container::open(&bytes, &key, &sbox)?;
    if !container.key_matches(&key) {
        return Err("the container was written for another Steam account".into());
    }
    let container_identical = container.to_png(&key, &sbox) == bytes;
    let mut lines = Vec::new();
    let (mut images, mut checksums, mut rewrites, mut rebuilds, mut other) = (0, 0, 0, 0, 0);
    for entry in container.entries() {
        let (_, data) = container
            .read(&entry.name)
            .ok_or("listed entry unreadable")?;
        images += usize::from(data.len() == save::container::SLOT_BYTES);
        let image = match SaveData::from_bytes(data) {
            Ok(image) => image,
            Err(problem) => {
                other += 1;
                lines.push(format!(
                    "{{\"entry\":{:?},\"size\":{},\"status\":{:?}}}",
                    entry.name,
                    data.len(),
                    problem.to_string()
                ));
                continue;
            }
        };
        checksums += 1;
        let p = Persistent::read(&image);
        let mut copy = image.clone();
        p.write(&mut copy);
        let rewritten = copy == image;
        let mut junk = image.clone();
        for (i, set) in written_bytes(&p).into_iter().enumerate() {
            if set {
                junk.set_u8(i, junk.u8_at(i) ^ 0x5a);
            }
        }
        p.write(&mut junk);
        let rebuilt = junk == image;
        rewrites += usize::from(rewritten);
        rebuilds += usize::from(rebuilt);
        lines.push(format!(
            "{{\"entry\":{:?},\"checksum\":true,\"read_write_identical\":{rewritten},\"rebuilt_from_values\":{rebuilt}}}",
            entry.name
        ));
    }
    let totals = format!(
        "{{\"mask_matches\":{mask_matches},\"container_identical\":{container_identical},\"save_images\":{images},\"checksums_match\":{checksums},\"read_write_identical\":{rewrites},\"rebuilt_from_values\":{rebuilds},\"other_entries\":{other}}}"
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(report.file_name().ok_or("report needs a name")?))?;
    writeln!(
        file,
        "{{\"schema_version\":1,\"totals\":{totals},\"entries\":[{}]}}",
        lines.join(",")
    )?;
    println!("{totals}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("save_roundtrip: {error}");
        std::process::exit(1);
    }
}
