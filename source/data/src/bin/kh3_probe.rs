//! Research probe for the installed KH3 paks, used while mapping the KH3
//! option. Prints to the console only; write its output to a private folder.
//!
//! `kh3_probe list <install> <substring>...`: mounted paths with all substrings.
//! `kh3_probe exports <install> <package path>`: names, imports and exports.
//! `kh3_probe props <install> <package path> [export]`: tagged properties.
//! `kh3_probe table <install> <package path>`: a DataTable's rows.
//! `kh3_probe mesh <install> <package path>`: a skeletal mesh's summary.
//! `kh3_probe anim <install> <package path>`: an animation's summary.

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
        "{} paks, {} files, identity {}…, opened in {:.1} s",
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
