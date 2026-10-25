//! Private research tool: for every spell file the `magc` table names and
//! `effect/texcommon.dpd`, lists each DPD texture (buffer pointer, size,
//! position in the file), the file's remastered textures, and what the
//! effect VM draws when each element runs: GS blend bytes, TEX0 fields,
//! whether a texture of the DPD (or the common one) covers the TEX0, and
//! how many frames the element lives.
//!
//! cargo run -p main --example effect_draw_census --release --locked -- <Steam-root> <manifest> <out.txt>
//!
//! Output holds numbers and installed file names only (no game bytes); keep
//! it outside the repository.
use data::{bar, object_effect, path_lookup::Library, probe_source::ProbeSource};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};

fn textures(out: &mut String, dpd: &object_effect::Dpd, base: usize) {
    for (i, t) in dpd.textures.iter().enumerate() {
        let at = t.data_offset;
        let _ = writeln!(
            out,
            "  tex{i}: dbp {:#x} cbp {:#x} dbw {} xy ({},{}) {}x{} at {:#x} tex0 {:#x}",
            t.dbp,
            t.cbp,
            t.dbw,
            t.x,
            t.y,
            t.width,
            t.height,
            base + at,
            t.tex0
        );
    }
}

fn tex0_text(t: u64) -> String {
    format!(
        "tbp {:#x} tbw {} psm {:#x} tw {} th {} tcc {} tfx {} cbp {:#x} cpsm {:#x} csm {} csa {} cld {}",
        t & 0x3fff,
        (t >> 14) & 0x3f,
        (t >> 20) & 0x3f,
        (t >> 26) & 0xf,
        (t >> 30) & 0xf,
        (t >> 34) & 1,
        (t >> 35) & 3,
        (t >> 37) & 0x3fff,
        (t >> 51) & 0xf,
        (t >> 55) & 1,
        (t >> 56) & 0x1f,
        (t >> 61) & 7
    )
}

fn covered(
    dpd: &object_effect::Dpd,
    common: Option<&object_effect::Dpd>,
    tbp: u16,
) -> &'static str {
    if dpd.textures.iter().any(|t| t.dbp == tbp) {
        "own"
    } else if common.is_some_and(|c| c.textures.iter().any(|t| t.dbp == tbp)) {
        "common"
    } else {
        "NONE"
    }
}

/// Count, largest colour bytes and largest texel coordinates of a quad kind.
type QuadStats = (usize, [u8; 4], [f32; 2]);

fn run(out: &mut String, pax: &object_effect::Pax, common: Option<&object_effect::Dpd>) {
    // Each OMD model: header ALPHA and flags, meshes, and vertex extent.
    for (d, dpd) in pax.dpds.iter().enumerate() {
        let mut ids: Vec<_> = dpd.models.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let m = &dpd.models[&id];
            let header = object_effect::omd_header(&dpd.bytes, m.hdr).ok();
            let meshes = object_effect::omd_meshes(&dpd.bytes, m.hdr).unwrap_or_default();
            let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
            for i in 0..m.vertex_count {
                if let Some(v) = dpd.vertex(id, i as u16) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(v[k] as i32);
                        hi[k] = hi[k].max(v[k] as i32);
                    }
                }
            }
            let kinds: Vec<_> = meshes
                .iter()
                .map(|s| {
                    format!(
                        "{}:{}",
                        if s.textured { "tex" } else { "flat" },
                        s.triangles.len()
                    )
                })
                .collect();
            let _ = writeln!(
                out,
                "  dpd {d} model {id}: header {:?} vertices {} extent {lo:?}..{hi:?} meshes {kinds:?}",
                header.map(|h| (h.alpha, h.flags)),
                m.vertex_count
            );
        }
    }
    let view = effect::math::IDENTITY;
    let mut rng = effect::Rng::default();
    for element in &pax.elements {
        let Some(dpx) = pax.effect(element.effect_number) else {
            let _ = writeln!(out, "  element {}: no DPX", element.id);
            continue;
        };
        let dpd = Arc::clone(&pax.dpds[dpx.dpd]);
        let world = effect::instance_matrix(element.translation, element.rotation, element.scale);
        let Some(mut instance) =
            effect::Instance::new(Arc::clone(&dpd), dpx.pdata, world, element.start_wait)
        else {
            continue;
        };
        let mut quads: BTreeMap<(u8, u64, String), QuadStats> = BTreeMap::new();
        let mut tris: BTreeMap<u8, usize> = BTreeMap::new();
        let mut models: BTreeMap<(i16, Option<u16>, bool), usize> = BTreeMap::new();
        let mut frames = 0;
        let mut max_size = 0f32;
        for _ in 0..1200 {
            let q = instance.frame(&view, &mut rng);
            frames += 1;
            for q in q {
                let tbp = (q.tex0 & 0x3fff) as u16;
                let e = quads
                    .entry((q.blend, q.tex0, covered(&dpd, common, tbp).to_string()))
                    .or_insert((0, [0; 4], [0.; 2]));
                e.0 += 1;
                for uv in q.uv {
                    e.2[0] = e.2[0].max(uv[0]);
                    e.2[1] = e.2[1].max(uv[1]);
                }
                let c = q.colour.map(|v| (v * 128.).clamp(0., 255.) as u8);
                e.1 = std::array::from_fn(|i| e.1[i].max(c[i]));
                let dx = q.corners[0][0] - q.corners[2][0];
                let dy = q.corners[0][1] - q.corners[2][1];
                max_size = max_size.max((dx * dx + dy * dy).sqrt());
            }
            for t in &instance.triangles {
                *tris.entry(t.blend).or_default() += 1;
            }
            for m in &instance.model_draws {
                *models.entry((m.model, m.mode, m.depth_only)).or_default() += 1;
            }
            if instance.finished() {
                break;
            }
        }
        let _ = writeln!(
            out,
            "  element {} (effect {}, dpd {}, pdata {}, bone {:#x}, flags {:#x}, wait {}, cat {:#x}): {} frames{}{} max quad {:.1}",
            element.id,
            element.effect_number,
            dpx.dpd,
            dpx.pdata,
            element.bone,
            element.flags,
            element.start_wait,
            element.category,
            frames,
            if instance.finished() {
                ""
            } else {
                " (still running)"
            },
            if instance.looping() { " looping" } else { "" },
            max_size
        );
        let _ = writeln!(
            out,
            "    use_texture {:02x?}",
            &dpd.pdata[dpx.pdata].use_texture[..4]
        );
        if !instance.unsupported.is_empty() {
            let _ = writeln!(out, "    unsupported {:?}", instance.unsupported);
        }
        for ((blend, tex0, cov), (n, c, uv)) in quads {
            let _ = writeln!(
                out,
                "    quad blend {blend:#04x} {} [{cov}] x{n} max rgba {c:?} max uv {uv:?}",
                tex0_text(tex0)
            );
        }
        for (blend, n) in tris {
            let _ = writeln!(out, "    triangles blend {blend:#04x} x{n}");
        }
        for ((model, mode, depth_only), n) in models {
            let alpha = dpd
                .models
                .get(&model)
                .and_then(|m| object_effect::omd_header(&dpd.bytes, m.hdr).ok())
                .map(|h| (h.alpha, h.flags));
            let _ = writeln!(
                out,
                "    model {model} mode {mode:?} depth_only {depth_only} header (alpha, flags) {alpha:?} x{n}"
            );
            if let Some(m) = dpd.models.get(&model)
                && let Ok(meshes) = object_effect::omd_meshes(&dpd.bytes, m.hdr)
            {
                for mesh in meshes {
                    if !mesh.textured {
                        let cs: Vec<_> = mesh.triangles.iter().flat_map(|t| t.colour).collect();
                        let _ = writeln!(
                            out,
                            "      untextured colours {:?}",
                            &cs[..cs.len().min(12)]
                        );
                    }
                    let _ = writeln!(
                        out,
                        "      mesh textured {} {} [{}]",
                        mesh.textured,
                        tex0_text(mesh.tex0),
                        covered(&dpd, common, (mesh.tex0 & 0x3fff) as u16)
                    );
                }
            }
        }
    }
}

fn replacements(out: &mut String, library: &mut Library, name: &str) {
    match library.read_replacements(name) {
        Ok(list) => {
            for r in list {
                let m = &r.metadata;
                let n = String::from_utf8_lossy(&m.name_bytes)
                    .trim_end_matches('\0')
                    .to_string();
                let size = data::hd_image::decode_hd_image(
                    &r.bytes,
                    m.declared_decompressed_length as usize,
                )
                .map(|d| format!("{:?} {}x{}", d.format, d.width, d.height))
                .unwrap_or_else(|e| format!("undecoded ({e})"));
                let _ = writeln!(
                    out,
                    "  remastered {}: {n:?} declared {:#x} original {:#x} {size}",
                    m.ordinal, m.declared_offset, m.original_asset_offset
                );
            }
        }
        Err(e) => {
            let _ = writeln!(out, "  remastered: none ({e})");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = ProbeSource::bind_session(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
    )?;
    source.verify_session()?;
    let mut library = Library::open(&source)?;
    let mut out = String::new();
    let common_bytes = library.read("effect/texcommon.dpd")?;
    let common = object_effect::read_dpd_file(&mut library, "effect/texcommon.dpd")?;
    let _ = writeln!(out, "effect/texcommon.dpd ({} bytes)", common_bytes.len());
    textures(&mut out, &common, 0);
    replacements(&mut out, &mut library, "effect/texcommon.dpd");
    out += &alpha_report(&common);
    let battle = library.read("00battle.bin")?;
    let table = bar::read_bar(&battle)?;
    let magc = table
        .entries
        .iter()
        .find(|e| &e.name_bytes == b"magc")
        .ok_or("no magc")?;
    let m = &battle[magc.offset as usize..magc.offset as usize + magc.size as usize];
    let count = u32::from_le_bytes(m[4..8].try_into()?) as usize;
    let mut files: Vec<String> = Vec::new();
    for k in 0..count {
        let r = &m[8 + k * 0x38..8 + (k + 1) * 0x38];
        let end = r[4..0x24].iter().position(|&b| b == 0).unwrap_or(32);
        let f = String::from_utf8_lossy(&r[4..4 + end]).into_owned();
        if r[2] == 0 && !f.is_empty() && !files.contains(&f) {
            files.push(f);
        }
    }
    if let Some(extra) = args.get(3) {
        files.extend(extra.split(',').map(String::from));
    }
    for file in files {
        let Ok(bytes) = library.read(&file) else {
            let _ = writeln!(out, "\n{file}: unreadable");
            continue;
        };
        let _ = writeln!(out, "\n{file} ({} bytes)", bytes.len());
        replacements(&mut out, &mut library, &file);
        let Ok(t) = bar::read_bar(&bytes) else {
            continue;
        };
        for e in t
            .entries
            .iter()
            .filter(|e| e.entry_type == 18 && e.size > 0)
        {
            match object_effect::read_file_pax(&mut library, &file).ok_or("unreadable") {
                Ok(pax) => {
                    for d in &pax.dpds {
                        out += &alpha_report(d);
                    }
                    let _ = writeln!(out, " PAX {:?} at {:#x}", pax.name, e.offset);
                    for (i, d) in pax.dpds.iter().enumerate() {
                        let at = pax.dpd_offsets[i];
                        let _ = writeln!(out, " dpd {i} at {:#x}", e.offset as usize + at);
                        textures(&mut out, d, e.offset as usize + at);
                    }
                    run(&mut out, &pax, Some(&common));
                }
                Err(err) => {
                    let _ = writeln!(out, " PAX unreadable: {err}");
                }
            }
        }
    }
    std::fs::write(&args[2], out)?;
    Ok(())
}

/// PS2 texture alpha (decoded, 0..255) against its page's HD alpha over the
/// texture's own area, when the page is stored at 1:1.
pub fn alpha_report(dpd: &object_effect::Dpd) -> String {
    // KH2_DUMP_PAGES=<private dir>: each remastered page as a PNG (game
    // pixels; keep the folder outside the repository).
    if let Some(dir) = std::env::var_os("KH2_DUMP_PAGES") {
        for (i, t) in dpd.textures.iter().enumerate() {
            if let Some(hd) = &t.hd {
                let path = std::path::Path::new(&dir).join(format!(
                    "{}-{:x}-{i}.png",
                    dpd.bytes.len(),
                    t.data_offset
                ));
                let _ = image::save_buffer(
                    path,
                    &hd.rgba,
                    hd.width,
                    hd.height,
                    image::ExtendedColorType::Rgba8,
                );
            }
        }
    }
    let mut out = String::new();
    for (i, t) in dpd.textures.iter().enumerate() {
        let Some(hd) = &t.hd else { continue };
        let tw = 1u32 << ((t.tex0 >> 26) & 0xf);
        let scale = hd.width / tw.max(1);
        if scale == 0 {
            continue;
        }
        let (mut a_ps2, mut a_hd, mut n, mut rgb_ps2, mut rgb_hd) = (0u64, 0u64, 0u64, 0u64, 0u64);
        let mut max_hd = 0u8;
        for y in 0..t.height {
            for x in 0..t.width {
                let s = ((y * t.width + x) * 4) as usize;
                let (hx, hy) = ((t.x as u32 + x) * scale, (t.y as u32 + y) * scale);
                if hx >= hd.width || hy >= hd.height {
                    continue;
                }
                let d = ((hy * hd.width + hx) * 4) as usize;
                a_ps2 += t.rgba[s + 3] as u64;
                a_hd += hd.rgba[d + 3] as u64;
                rgb_ps2 += t.rgba[s] as u64 + t.rgba[s + 1] as u64 + t.rgba[s + 2] as u64;
                rgb_hd += hd.rgba[d] as u64 + hd.rgba[d + 1] as u64 + hd.rgba[d + 2] as u64;
                max_hd = max_hd.max(hd.rgba[d + 3]);
                n += 1;
            }
        }
        let n = n.max(1);
        let _ = writeln!(
            out,
            "  tex{i}: scale {scale} mean alpha ps2 {} hd {} (hd max {max_hd}), mean rgb ps2 {} hd {}",
            a_ps2 / n,
            a_hd / n,
            rgb_ps2 / n / 3,
            rgb_hd / n / 3
        );
    }
    out
}
