//! Byte-match check for the BAR writer: every installed record that is a BAR
//! (and every BAR nested inside one) is parsed, written again with
//! `bar::write_bar`, and compared by SHA-256 with the original bytes. The
//! per-file results go to a new private report; only totals are printed.
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

/// Large enough for every installed record; larger ones are counted unread.
const READ_LIMIT: u32 = 512 * 1024 * 1024;

#[derive(Serialize)]
struct Record {
    index: &'static str,
    ordinal: usize,
    name_hash: String,
    /// Entry positions from the outer BAR down to this one; empty at the top.
    nested: Vec<usize>,
    original_sha256: String,
    rewritten_sha256: String,
    original_len: usize,
    rewritten_len: usize,
    status: &'static str,
    first_difference: Option<usize>,
    error: Option<String>,
    /// Whether the plain packing rule (no recorded layout) also matches.
    packed_by_rule: bool,
    layout: Option<Layout>,
}

#[derive(Default, Serialize)]
struct Totals {
    records: usize,
    unread: usize,
    bars: usize,
    nested_bars: usize,
    identical: usize,
    different: usize,
    unparsable: usize,
    /// BARs the plain packing rule reproduces without a recorded layout.
    packed_by_rule: usize,
}

/// Layout facts about an original the plain packing rule does not reproduce.
#[derive(Serialize)]
struct Layout {
    entries: usize,
    shared_offsets: usize,
    shared_same_name: usize,
    unaligned: Vec<i32>,
    zero_size_offsets: Vec<i32>,
    out_of_order: bool,
    first_gap: i64,
    nonzero_padding: usize,
    trailing: i64,
    len_mod_16: usize,
}

fn layout(bytes: &[u8]) -> Layout {
    let table = bar::read_bar(bytes).unwrap();
    let table_end = 16 + 16 * table.entries.len() as i64;
    let present: Vec<_> = table.entries.iter().filter(|e| e.offset >= 0).collect();
    let mut shared_offsets = 0;
    let mut shared_same_name = 0;
    for (i, e) in present.iter().enumerate() {
        if let Some(prev) = present[..i].iter().find(|p| p.offset == e.offset) {
            shared_offsets += 1;
            if prev.name_bytes == e.name_bytes {
                shared_same_name += 1;
            }
        }
    }
    let mut ranges: Vec<_> = present
        .iter()
        .filter(|e| e.size > 0)
        .map(|e| (e.offset as i64, e.offset as i64 + e.size as i64))
        .collect();
    let out_of_order = ranges.windows(2).any(|w| w[1].0 < w[0].0);
    ranges.sort();
    let mut covered = table_end;
    let mut nonzero_padding = 0;
    for (start, end) in &ranges {
        if *start > covered {
            nonzero_padding += bytes[covered as usize..*start as usize]
                .iter()
                .filter(|b| **b != 0)
                .count();
        }
        covered = covered.max(*end);
    }
    Layout {
        entries: table.entries.len(),
        shared_offsets,
        shared_same_name,
        unaligned: present
            .iter()
            .filter(|e| e.size > 0 && e.offset % 16 != 0)
            .map(|e| e.offset % 16)
            .collect(),
        zero_size_offsets: present
            .iter()
            .filter(|e| e.size == 0)
            .map(|e| e.offset)
            .collect(),
        out_of_order,
        first_gap: ranges.first().map_or(0, |r| r.0 - table_end),
        nonzero_padding,
        trailing: bytes.len() as i64 - covered,
        len_mod_16: bytes.len() % 16,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn check(
    bytes: &[u8],
    base: &Record,
    nested: Vec<usize>,
    totals: &mut Totals,
    out: &mut Vec<Record>,
) {
    let original_sha256 = hex(&Sha256::digest(bytes));
    let mut record = Record {
        index: base.index,
        ordinal: base.ordinal,
        name_hash: base.name_hash.clone(),
        nested: nested.clone(),
        original_sha256,
        rewritten_sha256: String::new(),
        original_len: bytes.len(),
        rewritten_len: 0,
        status: "unparsable",
        first_difference: None,
        error: None,
        packed_by_rule: false,
        layout: None,
    };
    if nested.is_empty() {
        totals.bars += 1;
    } else {
        totals.nested_bars += 1;
    }
    let parsed = match bar::parse_bar(bytes) {
        Ok(parsed) => parsed,
        Err(e) => {
            totals.unparsable += 1;
            record.error = Some(e.to_string());
            out.push(record);
            return;
        }
    };
    let rewritten = bar::write_bar(&parsed);
    record.rewritten_sha256 = hex(&Sha256::digest(&rewritten));
    record.rewritten_len = rewritten.len();
    if record.rewritten_sha256 == record.original_sha256 {
        totals.identical += 1;
        record.status = "identical";
    } else {
        totals.different += 1;
        record.status = "different";
        record.first_difference = Some(
            bytes
                .iter()
                .zip(&rewritten)
                .position(|(a, b)| a != b)
                .unwrap_or(bytes.len().min(rewritten.len())),
        );
    }
    let packed = bar::write_bar(&bar::Bar {
        layout: None,
        ..parsed.clone()
    });
    record.packed_by_rule = packed == bytes;
    if record.packed_by_rule {
        totals.packed_by_rule += 1;
    } else {
        record.layout = Some(layout(bytes));
    }
    out.push(record);
    for (i, item) in parsed.items.iter().enumerate() {
        if let bar::BarData::Bytes(data) = &item.data
            && data.starts_with(b"BAR\x01")
        {
            let mut inner = nested.clone();
            inner.push(i);
            check(data, base, inner, totals, out);
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("usage: bar_roundtrip <Steam-root> <manifest> <new-private-report>".into());
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let report = source.output_path(&args[2])?;
    source.verify_session()?;
    let mut library = Library::open(&source)?;
    let mut totals = Totals::default();
    let mut records = Vec::new();
    for found in library.all() {
        totals.records += 1;
        let bytes = match library.read_found_limited(&found, READ_LIMIT) {
            Ok(bytes) => bytes,
            Err(_) => {
                totals.unread += 1;
                continue;
            }
        };
        if !bytes.starts_with(b"BAR\x01") {
            continue;
        }
        let base = Record {
            index: found.index,
            ordinal: found.ordinal,
            name_hash: hex(&found.name_hash),
            nested: Vec::new(),
            original_sha256: String::new(),
            rewritten_sha256: String::new(),
            original_len: 0,
            rewritten_len: 0,
            status: "",
            first_difference: None,
            error: None,
            packed_by_rule: false,
            layout: None,
        };
        check(&bytes, &base, Vec::new(), &mut totals, &mut records);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report)?;
    file.write_all(&serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1,
        "source_content_identity": source.content_identity(),
        "totals": &totals,
        "records": records,
    }))?)?;
    println!("{}", serde_json::to_string_pretty(&totals)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("bar_roundtrip: {error}");
        std::process::exit(1);
    }
}
