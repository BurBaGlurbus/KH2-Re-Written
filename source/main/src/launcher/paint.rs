//! The launcher's drawing, made without game files: shapes generated here,
//! text from the system's own fonts, all as `Ui2dFrame` quads. Coordinates
//! are a 1280x720 design space mapped onto the 16:9 layout space.
use bevy::prelude::*;
use data::sprite_sequence::Quad;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use visual::overlay::{Sheet, Ui2dFrame, WIDE_WIDTH};

pub const W: f32 = 1280.;
pub const H: f32 = 720.;
const LEFT: f32 = 256. - WIDE_WIDTH / 2.;
const SX: f32 = WIDE_WIDTH / W;
const SY: f32 = visual::overlay::HEIGHT / H;

/// Glyph raster pixels per design pixel (sharp up to 1080p).
const RASTER: f32 = 1.5;
const ATLAS: u32 = 2048;
/// Rounded-rectangle texture: size and corner radius in texels.
const ROUND: u32 = 64;
const ROUND_CORNER: u32 = 32;

pub type Rgba = [f32; 4];

pub fn rgba(hex: u32, a: f32) -> Rgba {
    let c = |s: u32| ((hex >> s) & 0xff) as f32 / 255.;
    [c(16), c(8), c(0), a]
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

pub fn to_layout(x: f32, y: f32) -> [f32; 2] {
    [LEFT + x * SX, y * SY]
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    /// Headings: Trebuchet MS Bold Italic (close to KH2's menu lettering).
    Heading,
    /// Menu rows and body text: Segoe UI Semibold.
    Body,
}

/// The Windows names first, then the usual Linux packages' closest faces.
const FACES: [(Face, &[&str]); 2] = [
    (
        Face::Heading,
        &[
            "trebucbi.ttf",
            "seguisbi.ttf",
            "segoeuiz.ttf",
            "arialbi.ttf",
            "Trebuchet_MS_Bold_Italic.ttf",
            "Arial_Bold_Italic.ttf",
            "LiberationSans-BoldItalic.ttf",
            "NotoSans-BoldItalic.ttf",
            "DejaVuSans-BoldOblique.ttf",
            "FreeSansBoldOblique.ttf",
        ],
    ),
    (
        Face::Body,
        &[
            "seguisb.ttf",
            "segoeui.ttf",
            "arial.ttf",
            "Arial.ttf",
            "LiberationSans-Regular.ttf",
            "NotoSans-Regular.ttf",
            "DejaVuSans.ttf",
            "FreeSans.ttf",
        ],
    ),
];

/// How deep and how many files the search through Linux's font folders goes.
const FONT_SEARCH_DEPTH: u32 = 6;
const FONT_SEARCH_FILES: usize = 20_000;

#[derive(Clone, Copy)]
struct Glyph {
    uv: [i32; 4],
    /// Raster offset from the pen (x right, y down from the baseline).
    left: f32,
    top: f32,
    advance: f32,
}

pub struct Paint {
    pub white: usize,
    round: usize,
    atlas: usize,
    pixels: Vec<u8>,
    shelf: [u32; 3],
    dirty: bool,
    fonts: HashMap<Face, fontdue::Font>,
    glyphs: HashMap<(Face, u16, char), Option<Glyph>>,
}

fn sheet_from(
    images: &mut Assets<Image>,
    frame: &mut Ui2dFrame,
    rgba: Vec<u8>,
    size: u32,
) -> usize {
    frame.add_sheet(images, rgba, [size, size], 1.)
}

/// Generated textures: alpha shapes in white.
fn shape(size: u32, alpha: impl Fn(f32, f32) -> f32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let a = alpha(x as f32 + 0.5, y as f32 + 0.5).clamp(0., 1.);
            out.extend([255, 255, 255, (a * 255.).round() as u8]);
        }
    }
    out
}

fn read_font(path: &Path) -> Option<fontdue::Font> {
    let bytes = std::fs::read(path).ok()?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

/// Every `.ttf` or `.otf` under `folder`, in name order, added to `found`.
fn font_files(folder: &Path, depth: u32, found: &mut Vec<PathBuf>) {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    for path in paths {
        if found.len() >= FONT_SEARCH_FILES {
            return;
        }
        if path.is_dir() {
            if depth > 0 {
                font_files(&path, depth - 1, found);
            }
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
        {
            found.push(path);
        }
    }
}

/// `files` with the ones called `names` first, in that order, then the rest.
/// Linux packages differ in folders and file-name case, so names match any
/// folder and either case.
fn font_candidates<'a>(names: &[&str], files: &'a [PathBuf]) -> Vec<&'a PathBuf> {
    let rank = |path: &PathBuf| {
        let file = path.file_name().map(|n| n.to_string_lossy());
        file.and_then(|file| names.iter().position(|n| n.eq_ignore_ascii_case(&file)))
            .unwrap_or(names.len())
    };
    let mut ranked: Vec<&PathBuf> = files.iter().collect();
    ranked.sort_by_key(|path| rank(path));
    ranked
}

fn load_font(face: Face) -> Option<fontdue::Font> {
    let names = FACES.iter().find(|(f, _)| *f == face)?.1;
    if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        let fonts = Path::new(&windir).join("Fonts");
        return names.iter().find_map(|name| read_font(&fonts.join(name)));
    }
    let mut folders = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    folders.extend(validation::host::local_data_root().map(|data| data.join("fonts")));
    folders.extend(std::env::home_dir().map(|home| home.join(".fonts")));
    let mut files = Vec::new();
    for folder in &folders {
        font_files(folder, FONT_SEARCH_DEPTH, &mut files);
    }
    font_candidates(names, &files)
        .into_iter()
        .find_map(|path| read_font(path))
}

impl Paint {
    pub fn new(images: &mut Assets<Image>, frame: &mut Ui2dFrame) -> Result<Self, String> {
        let white = sheet_from(images, frame, vec![255; 4 * 4 * 4], 4);
        let r = ROUND as f32 / 2.;
        let round = sheet_from(
            images,
            frame,
            shape(ROUND, |x, y| {
                // Distance outside a rounded square of radius ROUND_CORNER.
                let c = ROUND_CORNER as f32 - 1.;
                let (dx, dy) = ((x - r).abs() - (r - c), (y - r).abs() - (r - c));
                let outside = dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - (c - 1.);
                0.5 - outside
            }),
            ROUND,
        );
        let pixels = vec![0; (ATLAS * ATLAS * 4) as usize];
        let atlas = sheet_from(images, frame, pixels.clone(), ATLAS);
        let mut fonts = HashMap::new();
        for (face, _) in FACES {
            let font = load_font(face).ok_or(
                "no usable system font found (on Linux, install the DejaVu or Liberation fonts)",
            )?;
            fonts.insert(face, font);
        }
        Ok(Self {
            white,
            round,
            atlas,
            pixels,
            shelf: [1, 1, 0],
            dirty: false,
            fonts,
            glyphs: HashMap::new(),
        })
    }

    fn glyph(&mut self, face: Face, size: f32, c: char) -> Option<Glyph> {
        let key = (face, (size * 4.).round() as u16, c);
        if let Some(g) = self.glyphs.get(&key) {
            return *g;
        }
        let font = &self.fonts[&face];
        let (m, coverage) = font.rasterize(c, size * RASTER);
        let (w, h) = (m.width as u32, m.height as u32);
        let glyph = (font.lookup_glyph_index(c) != 0 || c == ' ').then_some(());
        let glyph = glyph.and_then(|()| {
            // Shelf packing with a 1-texel gap.
            let [mut x, mut y, mut row] = self.shelf;
            if x + w + 1 >= ATLAS {
                (x, y, row) = (1, y + row + 1, 0);
            }
            if y + h + 1 >= ATLAS {
                return None;
            }
            for gy in 0..h {
                for gx in 0..w {
                    let a = coverage[(gy * w + gx) as usize];
                    let at = (((y + gy) * ATLAS + x + gx) * 4) as usize;
                    self.pixels[at..at + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
            self.shelf = [x + w + 1, y, row.max(h)];
            self.dirty |= w * h > 0;
            Some(Glyph {
                uv: [x as i32, y as i32, (x + w) as i32, (y + h) as i32],
                left: m.xmin as f32 / RASTER,
                top: -(m.ymin as f32 + h as f32) / RASTER,
                advance: m.advance_width / RASTER,
            })
        });
        self.glyphs.insert(key, glyph);
        glyph
    }

    /// Re-uploads the glyph atlas after new glyphs (once per frame).
    pub fn flush(&mut self, images: &mut Assets<Image>, frame: &mut Ui2dFrame) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let fresh = frame.add_sheet(images, self.pixels.clone(), [ATLAS, ATLAS], 1.);
        let sheet: Sheet = frame.sheets[fresh].clone();
        frame.rebind(self.atlas, sheet);
        frame.sheets.pop();
    }

    pub fn measure(&mut self, face: Face, size: f32, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.glyph(face, size, c))
            .map(|g| g.advance)
            .sum()
    }

    /// `text` cut in the middle with "…" to fit `width`.
    pub fn fit(&mut self, face: Face, size: f32, text: &str, width: f32) -> String {
        if self.measure(face, size, text) <= width {
            return text.to_owned();
        }
        let chars: Vec<char> = text.chars().collect();
        let mut keep = chars.len();
        while keep > 4 {
            keep -= 1;
            let head = keep / 3;
            let tail = keep - head;
            let s: String = chars[..head]
                .iter()
                .chain(['…'].iter())
                .chain(chars[chars.len() - tail..].iter())
                .collect();
            if self.measure(face, size, &s) <= width {
                return s;
            }
        }
        "…".into()
    }

    /// Text with its top at `y` (the line's ascent below it); `align` 0 left,
    /// 0.5 centred, 1 right of `x`. A soft dark shadow is drawn under it when
    /// `shadow` > 0 (its opacity). Returns the width.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        frame: &mut Ui2dFrame,
        face: Face,
        size: f32,
        [x, y]: [f32; 2],
        align: f32,
        text: &str,
        colors: [Rgba; 2],
        shadow: f32,
    ) -> f32 {
        let width = self.measure(face, size, text);
        let ascent = self.fonts[&face]
            .horizontal_line_metrics(size)
            .map_or(size * 0.8, |m| m.ascent);
        let (x0, base) = (x - width * align, y + ascent);
        let mut quads = Vec::new();
        let mut shadows = Vec::new();
        let mut pen = x0;
        for c in text.chars() {
            let Some(g) = self.glyph(face, size, c) else {
                continue;
            };
            let (gw, gh) = (
                (g.uv[2] - g.uv[0]) as f32 / RASTER,
                (g.uv[3] - g.uv[1]) as f32 / RASTER,
            );
            if gw > 0. {
                let (l, t) = (pen + g.left, base + g.top);
                // Vertical gradient: colors[0] at the cap, colors[1] at the base.
                let top = mix(colors[0], colors[1], ((t - y) / size).clamp(0., 1.));
                let bottom = mix(colors[0], colors[1], ((t + gh - y) / size).clamp(0., 1.));
                quads.push(quad([l, t, gw, gh], g.uv, [top, top, bottom, bottom], 0));
                if shadow > 0. {
                    let d = (size * 0.06).max(1.5);
                    let s = [0., 0., 0.04, shadow * colors[0][3]];
                    shadows.push(quad([l + d, t + d, gw, gh], g.uv, [s; 4], 0));
                }
            }
            pen += g.advance;
        }
        if !shadows.is_empty() {
            frame.draws.push((self.atlas, shadows));
        }
        frame.draws.push((self.atlas, quads));
        width
    }

    /// A gradient rectangle (colors: top-left, top-right, bottom-left,
    /// bottom-right).
    pub fn rect(&self, frame: &mut Ui2dFrame, r: [f32; 4], colors: [Rgba; 4], additive: bool) {
        frame.draws.push((
            self.white,
            vec![quad(r, [1, 1, 3, 3], colors, additive as i32)],
        ));
    }

    /// A rounded box with corner radius `radius`, its top edge shifted right
    /// by `skew`. Colours as in `rect`.
    #[allow(clippy::too_many_arguments)]
    pub fn plate(
        &self,
        frame: &mut Ui2dFrame,
        [x, y, w, h]: [f32; 4],
        radius: f32,
        skew: f32,
        colors: [Rgba; 4],
        additive: bool,
    ) {
        let r = radius.min(w / 2.).min(h / 2.);
        let xs = [x, x + r, x + w - r, x + w];
        let ys = [y, y + r, y + h - r, y + h];
        let c = ROUND_CORNER as i32;
        let us = [0, c, c, ROUND as i32];
        let colour_at = |px: f32, py: f32| {
            let (u, v) = ((px - x) / w, (py - y) / h);
            mix(
                mix(colors[0], colors[1], u),
                mix(colors[2], colors[3], u),
                v,
            )
        };
        let mut quads = Vec::with_capacity(9);
        for j in 0..3 {
            for i in 0..3 {
                if xs[i + 1] <= xs[i] || ys[j + 1] <= ys[j] {
                    continue;
                }
                let corner = |ii: usize, jj: usize| {
                    let (px, py) = (xs[ii], ys[jj]);
                    let shift = skew * (1. - (py - y) / h);
                    (to_layout(px + shift, py), colour_at(px, py))
                };
                let k = [
                    corner(i, j),
                    corner(i + 1, j),
                    corner(i, j + 1),
                    corner(i + 1, j + 1),
                ];
                quads.push(Quad {
                    corners: k.map(|(p, _)| p),
                    uv: [us[i], us[j], us[i + 1], us[j + 1]],
                    colors: k.map(|(_, c)| c),
                    blend: additive as i32,
                });
            }
        }
        frame.draws.push((self.round, quads));
    }
}

/// A quad over design rectangle `[x, y, w, h]`.
pub fn quad(r: [f32; 4], uv: [i32; 4], colors: [Rgba; 4], blend: i32) -> Quad {
    let [x, y, w, h] = r;
    Quad {
        corners: [
            to_layout(x, y),
            to_layout(x + w, y),
            to_layout(x, y + h),
            to_layout(x + w, y + h),
        ],
        uv,
        colors,
        blend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_space_covers_the_wide_layout() {
        assert_eq!(to_layout(0., 0.), [LEFT, 0.]);
        let [r, b] = to_layout(W, H);
        assert!((r - (LEFT + WIDE_WIDTH)).abs() < 1e-3);
        assert!((b - visual::overlay::HEIGHT).abs() < 1e-3);
    }

    #[test]
    fn fonts_are_ranked_by_name_in_any_folder_and_case() {
        let files = [
            PathBuf::from("/fonts/other/Zed.ttf"),
            PathBuf::from("/fonts/dejavu/DejaVuSans.ttf"),
            PathBuf::from("/fonts/msttcore/arial.TTF"),
        ];
        let ranked = font_candidates(&["Arial.ttf", "DejaVuSans.ttf"], &files);
        assert_eq!(ranked, [&files[2], &files[1], &files[0]]);
    }

    #[test]
    fn font_search_finds_font_files_in_subfolders() {
        let dir = std::env::temp_dir().join(format!("kh2-font-search-{}", std::process::id()));
        let deep = dir.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("x.TTF"), b"synthetic").unwrap();
        std::fs::write(deep.join("y.otf"), b"synthetic").unwrap();
        std::fs::write(deep.join("readme.txt"), b"synthetic").unwrap();
        let mut found = Vec::new();
        font_files(&dir, 2, &mut found);
        assert_eq!(found, [deep.join("x.TTF"), deep.join("y.otf")]);
        found.clear();
        font_files(&dir, 1, &mut found);
        assert!(found.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn colours_parse_and_mix() {
        assert_eq!(rgba(0xff8000, 1.), [1., 128. / 255., 0., 1.]);
        assert_eq!(mix([0.; 4], [1.; 4], 0.25), [0.25; 4]);
    }
}
