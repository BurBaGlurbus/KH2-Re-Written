//! Research probe for the installed KH3 paks, used while mapping the KH3
//! option. Prints to the console only; write its output to a private folder.
//!
//! `kh3_probe list <install> <substring>...`: mounted paths with all substrings.
//! `kh3_probe exports <install> <package path>`: names, imports and exports.
//! `kh3_probe props <install> <package path> [export]`: tagged properties.
//! `kh3_probe table <install> <package path>`: a DataTable's rows.
//! `kh3_probe mesh <install> <package path>`: a skeletal mesh's summary.
//! `kh3_probe anim <install> <package path>`: an animation's summary.
//! `kh3_probe dump <install> <out dir> <list file>`: each listed package's exports as JSON, for private research.

use data::kh3::{self, props, skeletal, texture};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, folder, rest @ ..] = args.as_slice() else {
        return Err("usage: kh3_probe <command> <install> [...]".into());
    };
    let install = kh3::admit(&PathBuf::from(folder))?;
    let started = std::time::Instant::now();
    let keep = command == "list";
    let paks = kh3::Paks::open(&install, None, keep)?;
    eprintln!(
        "{} paks, {} files, identity {}â€¦, opened in {:.1} s",
        install.pak_count,
        paks.mounted.len(),
        &install.identity_hex()[..12],
        started.elapsed().as_secs_f32()
    );
    let package = |path: &String| -> Result<data::kh3::package::Package> {
        paks.package(&path.to_ascii_lowercase())?
            .ok_or_else(|| format!("no package {path}").into())
    };
    match (command.as_str(), rest) {
        ("list", subs) => {
            let subs: Vec<String> = subs.iter().map(|s| s.to_ascii_lowercase()).collect();
            for name in paks.mounted.names() {
                if subs.iter().all(|s| name.contains(s.as_str())) {
                    println!("{name}");
                }
            }
        }
        ("exports", [path]) => {
            let p = package(path)?;
            for (i, e) in p.exports.iter().enumerate() {
                println!(
                    "export {i} {} class={} size={} outer={}",
                    e.name,
                    p.class_of(i),
                    e.size,
                    e.outer
                );
            }
            for (i, im) in p.imports.iter().enumerate() {
                println!(
                    "import -{} {} class={} outer={}",
                    i + 1,
                    im.name,
                    im.class,
                    im.outer
                );
            }
        }
        ("props", [path, tail @ ..]) => {
            let p = package(path)?;
            for i in 0..p.exports.len() {
                if tail.first().is_some_and(|t| *t != p.exports[i].name) {
                    continue;
                }
                println!("== {i} {} ({})", p.exports[i].name, p.class_of(i));
                match props::export_props(&p, i) {
                    Ok((list, end)) => {
                        println!("{list:#?}");
                        println!("props end at {end} of {}", p.exports[i].size);
                    }
                    Err(e) => println!("unreadable: {e}"),
                }
            }
        }
        ("table", [path]) => {
            let p = package(path)?;
            let i = p.export_of_class("DataTable").ok_or("no DataTable")?;
            for (name, row) in props::data_table_rows(&p, i)? {
                println!("{name}: {row:?}");
            }
        }
        ("mesh", [path]) => {
            let p = package(path)?;
            let i = p.export_of_class("SkeletalMesh").ok_or("no SkeletalMesh")?;
            let m = skeletal::SkeletalMesh::read(&p, i)?;
            println!(
                "{} bones, {} verts, {} indices, {} sections",
                m.bones.len(),
                m.lod.positions.len(),
                m.lod.indices.len(),
                m.lod.sections.len()
            );
            for s in &m.materials {
                println!(
                    "material {} -> {} {:?}",
                    s.slot,
                    p.object_name(s.object),
                    p.import_package(s.object)
                );
            }
            for b in m.bones.iter().take(12) {
                println!("{} parent {} at {:?}", b.name, b.parent, b.translation);
            }
        }
        ("anim", [path]) => {
            let p = package(path)?;
            let i = p.export_of_class("AnimSequence").ok_or("no AnimSequence")?;
            let a = kh3::anim::AnimSequence::read(&p, i)?;
            println!(
                "{} s, {} frames, {} tracks, scale table {}",
                a.length,
                a.num_frames,
                a.tracks.len(),
                a.has_scale_table
            );
            println!("{:#?}", a.props);
        }
        ("material", paths) => {
            for path in paths {
                let m = kh3::material::read(&paks, path)?;
                println!(
                    "== {path} root {} blend {:?} two-sided {}",
                    m.root, m.blend, m.two_sided
                );
                println!("textures {:#?}", m.textures);
                println!("vectors {:?}", m.vectors);
            }
        }
        ("sora", []) => {
            let t = std::time::Instant::now();
            let d = kh3::sora::SoraData::load(&paks)?;
            println!(
                "loaded in {:.2} s: {} rows, {} clips, {} textures, {} sockets, blade {} verts, {} blade sockets",
                t.elapsed().as_secs_f32(),
                d.anims.rows.len(),
                d.anims.clips.len(),
                d.textures.len(),
                d.sockets.len(),
                d.keyblade.lod.positions.len(),
                d.keyblade_sockets.len()
            );
            for l in d.looks.iter().chain(&d.keyblade_looks) {
                println!("{l:?}");
            }
            for (path, tex) in &d.textures {
                println!(
                    "{path} {} {}x{} mips {}",
                    tex.format,
                    tex.width,
                    tex.height,
                    tex.mips.len()
                );
            }
            let mut classes = std::collections::BTreeMap::<String, usize>::new();
            for c in &d.anims.clips {
                for n in &c.notifies {
                    *classes.entry(n.class.clone()).or_default() += 1;
                }
            }
            println!("{classes:#?}");
            for name in ["IdleN", "RunLoopN", "DodgeRoll", "1301", "M_1473"] {
                let Some(r) = d.anims.row(name) else {
                    println!("{name}: no row");
                    continue;
                };
                let c = d.anims.main_clip(r, 0.).unwrap();
                println!(
                    "{name}: clip {} {} s root {} blend {} notifies {}",
                    c.name,
                    c.seq.length,
                    c.root_motion,
                    c.blend,
                    c.notifies.len()
                );
                if name == "1301" {
                    for n in &c.notifies {
                        println!("  {} {:.3}-{:.3}", n.class, n.start, n.end);
                    }
                    println!(
                        "  root delta {:?}",
                        kh3::sora::root_delta(c, 0., c.seq.length)
                    );
                }
            }
        }
        ("rows", []) => {
            let set = kh3::sora::read_anim_set(&paks, kh3::sora::ANIM_SET)?;
            for r in &set.rows {
                match &r.data {
                    kh3::sora::RowData::Clip(i) => print!("{}={} ", r.name, set.clips[*i].name),
                    kh3::sora::RowData::Blend1D { samples, .. } => {
                        print!("{}=1D{} ", r.name, samples.len())
                    }
                    kh3::sora::RowData::Blend2D { samples, .. } => {
                        print!("{}=2D{} ", r.name, samples.len())
                    }
                }
            }
            println!();
        }
        // `sound <out dir> <asset>...`: decodes each asset's first notes to WAV files.
        ("sound", [out, assets @ ..]) => {
            let image = std::fs::read(install.exe())?;
            let tables = kh3::hca::Tables::find(&image)?;
            let banks = kh3::sead::Banks::default();
            let mut key = None;
            for asset in assets {
                let Some(r) = banks.resolve(&paks, asset) else {
                    println!("{asset}: not resolved");
                    continue;
                };
                if key.is_none() {
                    key = kh3::sead::find_key(&image, &tables, &r.bank).ok();
                }
                let keys = kh3::sead::Keys {
                    tables: tables.clone(),
                    sead_key: key.unwrap_or([0; 256]),
                };
                let Some(Some(sound)) = r.bank.sounds.get(r.slot) else {
                    println!("{asset}: empty slot {}", r.slot);
                    continue;
                };
                println!(
                    "{asset}: bank {} slot {} kind {:?} cat {} vol {} asset vol {} spatial {} seqs {}",
                    r.bank.number,
                    r.slot,
                    sound.kind,
                    sound.category,
                    sound.volume,
                    r.volume,
                    r.spatial,
                    sound.sequences.len()
                );
                for seq in sound.sequences.iter().take(1) {
                    for n in &seq.notes {
                        let m = &r.bank.materials[usize::from(n.material)];
                        let d = kh3::sead::decode_material(m, &keys)?;
                        let max = d
                            .samples
                            .iter()
                            .map(|x| x.unsigned_abs())
                            .max()
                            .unwrap_or(0);
                        println!(
                            "  note +{} ms material {} {} ch {} Hz {} samples peak {} bad frames {}",
                            n.delay_ms,
                            n.material,
                            d.channels,
                            d.rate,
                            d.samples.len(),
                            max,
                            d.bad_frames
                        );
                        let name = asset.rsplit('/').next().unwrap_or("sound");
                        let path =
                            std::path::Path::new(out).join(format!("{name}-{}.wav", n.material));
                        let mut w = Vec::new();
                        let bytes = d.samples.len() * 2;
                        w.extend_from_slice(b"RIFF");
                        w.extend_from_slice(&((36 + bytes) as u32).to_le_bytes());
                        w.extend_from_slice(b"WAVEfmt ");
                        w.extend_from_slice(&16u32.to_le_bytes());
                        w.extend_from_slice(&1u16.to_le_bytes());
                        w.extend_from_slice(&u16::from(d.channels).to_le_bytes());
                        w.extend_from_slice(&d.rate.to_le_bytes());
                        w.extend_from_slice(&(d.rate * u32::from(d.channels) * 2).to_le_bytes());
                        w.extend_from_slice(&(u16::from(d.channels) * 2).to_le_bytes());
                        w.extend_from_slice(&16u16.to_le_bytes());
                        w.extend_from_slice(b"data");
                        w.extend_from_slice(&(bytes as u32).to_le_bytes());
                        for x in &d.samples {
                            w.extend_from_slice(&x.to_le_bytes());
                        }
                        std::fs::write(path, w)?;
                    }
                }
            }
        }
        ("dump", [out, list]) => {
            std::fs::create_dir_all(out)?;
            for path in std::fs::read_to_string(list)?.lines() {
                let path = path.trim();
                if path.is_empty() {
                    continue;
                }
                let Some(p) = paks.package(&path.to_ascii_lowercase())? else {
                    eprintln!("no package {path}");
                    continue;
                };
                let mut exports = Vec::new();
                for i in 0..p.exports.len() {
                    let e = &p.exports[i];
                    let (data, rows) = match props::export_props(&p, i) {
                        Ok((list, _)) => (dump_props(&p, &list), serde_json::Value::Null),
                        Err(_) => (serde_json::Value::Null, serde_json::Value::Null),
                    };
                    let rows = if p.class_of(i) == "DataTable" {
                        props::data_table_rows(&p, i)
                            .map(|rows| {
                                serde_json::Value::Array(
                                    rows.iter()
                                        .map(|(n, r)| serde_json::json!([n, dump_props(&p, r)]))
                                        .collect(),
                                )
                            })
                            .unwrap_or(rows)
                    } else {
                        rows
                    };
                    exports.push(serde_json::json!({"name": e.name, "class": p.class_of(i),
                        "sup": e.sup, "outer": e.outer, "template": e.template,
                        "super_path": (e.sup < 0).then(|| p.import_package(e.sup).map(str::to_owned)),
                        "props": data, "rows": rows}));
                }
                let name = path.replace('/', "@");
                std::fs::write(
                    format!("{out}/{name}.json"),
                    serde_json::to_string(&serde_json::json!({"path": path, "exports": exports}))?,
                )?;
            }
        }
        // `sheets <out file>`: every recipe's value as JSON, for a private comparison.
        ("sheets", [out]) => {
            let sheets = kh3::sheets::read(&paks);
            for (sheet, id, why) in &sheets.missing {
                eprintln!("missing {sheet} {id}: {why}");
            }
            let val = |v: &kh3::sheets::Val| -> serde_json::Value {
                use kh3::sheets::Val as V;
                match v {
                    V::N(n) => serde_json::json!(n),
                    V::B(b) => serde_json::json!(b),
                    V::S(s) => serde_json::json!(s),
                    V::A(a) => serde_json::json!(a),
                    V::L(l) => serde_json::json!(l),
                    V::Nil => serde_json::Value::Null,
                }
            };
            let params: serde_json::Map<_, _> = sheets
                .params
                .iter()
                .map(|(s, rows)| {
                    let rows: serde_json::Map<_, _> =
                        rows.iter().map(|(i, v)| (i.clone(), val(v))).collect();
                    ((*s).to_owned(), serde_json::Value::Object(rows))
                })
                .collect();
            let tables: serde_json::Map<_, _> = sheets
                .tables
                .iter()
                .map(|(s, rows)| {
                    let rows: Vec<serde_json::Value> = rows
                        .iter()
                        .map(|cells| {
                            let cells: serde_json::Map<_, _> =
                                cells.iter().map(|(c, v)| (c.clone(), val(v))).collect();
                            serde_json::Value::Object(cells)
                        })
                        .collect();
                    ((*s).to_owned(), serde_json::Value::Array(rows))
                })
                .collect();
            std::fs::write(
                out,
                serde_json::to_string(&serde_json::json!({"params": params, "tables": tables}))?,
            )?;
            println!(
                "{} missing, executable digest {}",
                sheets.missing.len(),
                sheets.exe_digest
            );
        }
        ("texture", [path]) => {
            let p = package(path)?;
            let i = p.export_of_class("Texture2D").ok_or("no Texture2D")?;
            let t = texture::Texture::read(&p, i)?;
            println!(
                "{} {}x{} mips {}",
                t.format,
                t.width,
                t.height,
                t.mips.len()
            );
        }
        _ => return Err("unknown command or arguments".into()),
    }
    Ok(())
}

fn dump_props(p: &data::kh3::package::Package, list: &props::Props) -> serde_json::Value {
    serde_json::Value::Array(
        list.0
            .iter()
            .map(|q| serde_json::json!([q.name, q.index, q.struct_name, dump_value(p, &q.value)]))
            .collect(),
    )
}

fn dump_value(p: &data::kh3::package::Package, v: &props::Value) -> serde_json::Value {
    use props::Value as V;
    match v {
        V::Bool(b) => serde_json::json!(b),
        V::Int(i) => serde_json::json!(i),
        V::Float(f) => serde_json::json!(f),
        V::Str(s) | V::Name(s) => serde_json::json!(s),
        V::Object(o) => {
            serde_json::json!({"obj": o, "path": data::kh3::material::import_path(p, *o)})
        }
        V::Byte(b) => serde_json::json!(b),
        V::Floats(f) => serde_json::json!(f),
        V::Ints(i) => serde_json::json!(i),
        V::Struct(n, q) => serde_json::json!({"struct": n, "props": dump_props(p, q)}),
        V::Array(a) => serde_json::Value::Array(a.iter().map(|x| dump_value(p, x)).collect()),
        V::Map(m) => serde_json::Value::Array(
            m.iter()
                .map(|(k, x)| serde_json::json!({"k": dump_value(p, k), "v": dump_value(p, x)}))
                .collect(),
        ),
        V::Raw(r) => serde_json::json!({"raw": r.len()}),
    }
}
