//! What the PC binds when it draws an effect's quads and models: the
//! texture of a TEX0, the GS blend of an ALPHA byte, and the ALPHA a model
//! draw uses. Shared by the effect presenters (spells, Drive and system
//! effects, the world map and cutscenes).
//!
//! The PC port does not emulate GS memory for effects. FUN_1404aecd0 makes
//! one texture per DPD texture (its remastered page when the file has one,
//! [`data::object_effect::Dpd::attach_hd`]), and FUN_1402c2ae0 registers,
//! for each effect it draws, the textures its PDATA header (+0xb0) marks,
//! by buffer pointer (FUN_14012c620, later ones replacing earlier ones at
//! the same pointer, the palette pointer left 0). A draw's TEX0 then finds
//! its texture by buffer pointer alone (FUN_14012c0f0): the CLUT pointer is
//! not part of the match. The resident `effect/texcommon.dpd` textures are
//! registered the same way, all of them (FUN_1402c2640).
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use data::object_effect::{Dpd, DpdTexture};
use std::sync::Arc;

/// The texture a draw binds: texture `index` of the DPD at address `dpd`,
/// sampled over the TEX0 page of 2^tw × 2^th texels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TexKey {
    pub dpd: usize,
    pub index: usize,
    pub tw: u8,
    pub th: u8,
}

impl TexKey {
    /// The texture a draw with `tex0` finds: the last texture of `own`
    /// that the effect's `mask` (PDATA +0xb0) registers at the TEX0's
    /// buffer pointer, else the last of `own` at that pointer, else the
    /// last common texture there.
    pub fn pick(
        own: &Arc<Dpd>,
        mask: &[u8; 16],
        common: Option<&Arc<Dpd>>,
        tex0: u64,
    ) -> Option<Self> {
        let tbp = (tex0 & 0x3fff) as u16;
        let marked = |i: usize| mask.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 != 0);
        let at = |d: &Dpd| -> Vec<usize> {
            d.textures
                .iter()
                .enumerate()
                .filter(|(_, t)| t.dbp == tbp)
                .map(|(i, _)| i)
                .collect()
        };
        let own_at = at(own);
        let (dpd, index) = match own_at.iter().rev().find(|&&i| marked(i)).or(own_at.last()) {
            Some(&i) => (Arc::as_ptr(own) as usize, i),
            None => {
                let c = common?;
                (Arc::as_ptr(c) as usize, *at(c).last()?)
            }
        };
        Some(TexKey {
            dpd,
            index,
            tw: ((tex0 >> 26) & 0xf) as u8,
            th: ((tex0 >> 30) & 0xf) as u8,
        })
    }

    /// The page's size in PS2 texels.
    pub fn size(&self) -> [u32; 2] {
        [1 << self.tw.min(10), 1 << self.th.min(10)]
    }
}

/// The page image of `key` from its DPD: the remastered page when there is
/// one, else the PS2 texture alone at its place in its page, through its
/// own palette.
pub fn page_image(dpd: &Dpd, key: TexKey, repeat: bool) -> Image {
    let [w, h] = key.size();
    let (size, pixels) = match dpd.textures.get(key.index) {
        Some(DpdTexture { hd: Some(page), .. }) => ([page.width, page.height], page.rgba.clone()),
        Some(t) => ([w, h], ps2_page(t, [w, h])),
        None => ([w, h], vec![0; (w * h * 4) as usize]),
    };
    let mut image = Image::new(
        Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let mode = if repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A PS2 texture placed at its buffer position in a `size` page.
fn ps2_page(t: &DpdTexture, size: [u32; 2]) -> Vec<u8> {
    let [w, h] = size;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for row in 0..t.height {
        let y = t.y as u32 + row;
        if y >= h {
            break;
        }
        for col in 0..t.width {
            let x = t.x as u32 + col;
            if x >= w {
                break;
            }
            let s = ((row * t.width + col) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            pixels[d..d + 4].copy_from_slice(&t.rgba[s..s + 4]);
        }
    }
    pixels
}

/// The PC's blend for a GS ALPHA byte ((A − B)·C + D, FUN_140472110 →
/// a D3D blend state): which colours A, B and D pick decides the state.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Blend {
    /// (Cs − Cd)·As + Cd (0x44).
    Alpha,
    /// Cs·As + Cd (0x48).
    Add,
    /// Cd − Cs·As (0x42, D3D reverse subtract).
    Subtract,
    /// Cs·As, the frame buffer's colour dropped (0x88).
    Replace,
}

impl Blend {
    pub fn of(alpha: u8) -> Self {
        let (a, b, d) = (alpha & 3, alpha >> 2 & 3, alpha >> 6 & 3);
        match (a, b, d) {
            (0, 2, 1) => Blend::Add,
            (2, 0, 1) => Blend::Subtract,
            (_, _, 2) => Blend::Replace,
            _ => Blend::Alpha,
        }
    }

    /// The material's alpha mode; `None` for a subtractive draw, which a
    /// standard material cannot express (it is left out).
    pub fn alpha_mode(self) -> Option<AlphaMode> {
        match self {
            Blend::Alpha => Some(AlphaMode::Blend),
            Blend::Add => Some(AlphaMode::Add),
            // Cs·As over the frame buffer where the texture shows [I: the
            // GS alpha test of these draws is not traced; an alpha blend
            // gives the same colour wherever the texture is opaque].
            Blend::Replace => Some(AlphaMode::Blend),
            Blend::Subtract => None,
        }
    }
}

/// The ALPHA byte a model draw uses (FUN_1404b6860): the draw's own mode
/// word when it is not zero, else the model header's (+0x16).
pub fn model_alpha(header: u8, mode: Option<u16>) -> u8 {
    match mode {
        Some(m) if m != 0 => m as u8,
        _ => header,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::object_effect::Dpd;
    use std::collections::HashMap;

    fn dpd(buffers: &[u16]) -> Arc<Dpd> {
        let texture = |dbp: u16| DpdTexture {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            indices: vec![0],
            palette: vec![0; 1024],
            dbp,
            cbp: 0,
            dbw: 1,
            x: 0,
            y: 0,
            tex0: 0,
            data_offset: 0,
            hd: None,
        };
        Arc::new(Dpd {
            bytes: Vec::new(),
            pdata: Vec::new(),
            shapes: HashMap::new(),
            model_vertices: HashMap::new(),
            models: HashMap::new(),
            vsf: HashMap::new(),
            textures: buffers.iter().map(|&b| texture(b)).collect(),
        })
    }

    #[test]
    fn a_draw_binds_the_last_registered_texture_at_its_buffer() {
        let own = dpd(&[0x3000, 0x3020, 0x3000, 0x3000]);
        let common = dpd(&[0x2a00, 0x2a00]);
        let tex0 = |tbp: u64| tbp | 7 << 26 | 6 << 30;
        // Textures 0 and 2 registered: 2 replaced 0.
        let mut mask = [0u8; 16];
        mask[0] = 0b0101;
        let k = TexKey::pick(&own, &mask, Some(&common), tex0(0x3000)).unwrap();
        assert_eq!((k.index, k.size()), (2, [128, 64]));
        // Nothing registered there: the last at that buffer.
        let k = TexKey::pick(&own, &[0; 16], Some(&common), tex0(0x3000)).unwrap();
        assert_eq!(k.index, 3);
        // A common buffer: the common DPD's last texture there.
        let k = TexKey::pick(&own, &mask, Some(&common), tex0(0x2a00)).unwrap();
        assert_eq!((k.dpd, k.index), (Arc::as_ptr(&common) as usize, 1));
        assert!(TexKey::pick(&own, &mask, None, tex0(0x2c00)).is_none());
    }

    #[test]
    fn blend_bytes_map_to_their_d3d_states() {
        assert_eq!(Blend::of(0x48), Blend::Add);
        assert_eq!(Blend::of(0x44), Blend::Alpha);
        assert_eq!(Blend::of(0x42), Blend::Subtract);
        assert_eq!(Blend::of(0x88), Blend::Replace);
        assert_eq!(model_alpha(0x48, Some(0x42)), 0x42);
        assert_eq!(model_alpha(0x44, Some(0)), 0x44);
        assert_eq!(model_alpha(0x44, None), 0x44);
    }
}
