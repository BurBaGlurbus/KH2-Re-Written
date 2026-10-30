//! `UTexture2D` exports: the mips and their format, the bytes left as they
//! are stored (block-compressed textures go to the GPU unchanged). Layout from
//! StrawHatPro's `docs/formats/texture.md` (MIT).
use super::package::{Package, Rd, read_name};
use super::props::{Props, read_props};
use super::{Result, bad};

#[derive(Clone, Debug, PartialEq)]
pub struct Mip {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct Texture {
    pub props: Props,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub mips: Vec<Mip>,
}

/// Bytes of a mip of nominal size `w`×`h`; `None` for an unknown format.
pub fn mip_size(format: &str, w: u32, h: u32) -> Option<usize> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let blocks = w.div_ceil(4) * h.div_ceil(4);
    Some(match format {
        "PF_DXT1" | "PF_BC4" => blocks * 8,
        "PF_DXT5" | "PF_BC5" | "PF_BC6H" | "PF_BC7" => blocks * 16,
        "PF_B8G8R8A8" => w * h * 4,
        "PF_G8" => w * h,
        "PF_FloatRGBA" => w * h * 8,
        _ => return None,
    })
}

impl Texture {
    pub fn read(p: &Package, i: usize) -> Result<Self> {
        let bytes = p.export_bytes(i)?;
        let mut r = Rd::new(bytes);
        let props = read_props(&mut r, &p.names)?;
        r.i32()?;
        r.skip(4)?;
        if r.i32()? != 1 {
            return Err(bad("texture is not cooked"));
        }
        let pixel = read_name(&mut r, &p.names)?;
        if pixel == "None" {
            return Err(bad("texture without platform data"));
        }
        r.i32()?;
        let (width, height) = (r.i32()?.max(0) as u32, r.i32()?.max(0) as u32);
        let slices = r.i32()?;
        if slices != 1 {
            return Err(bad("not a 2D texture"));
        }
        let format = r.fstring()?;
        let first = r.i32()?;
        let count = r.count(20)?;
        let mut mips = Vec::with_capacity(count);
        for m in 0..count {
            r.i32()?;
            let flags = r.u32()?;
            let elements = usize::try_from(r.i32()?).map_err(|_| bad("mip size"))?;
            r.i32()?;
            let offset = r.i64()?;
            let data = if flags & 0x100 != 0 {
                let from = offset
                    .checked_add(p.bulk_start)
                    .and_then(|o| usize::try_from(o).ok())
                    .ok_or_else(|| bad("mip offset"))?;
                p.ubulk
                    .get(from..from + elements)
                    .ok_or_else(|| bad("mip past the .ubulk"))?
                    .to_vec()
            } else {
                r.take(elements)?.to_vec()
            };
            r.i32()?;
            r.i32()?;
            // The nominal chain: a 2x2 mip is recorded as 4x4.
            let shift = (first.max(0) as u32) + m as u32;
            mips.push(Mip {
                width: (width >> shift).max(1),
                height: (height >> shift).max(1),
                bytes: data,
            });
        }
        Ok(Self {
            props,
            format,
            width,
            height,
            mips,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_sizes_follow_the_format() {
        assert_eq!(mip_size("PF_DXT1", 4, 4), Some(8));
        assert_eq!(mip_size("PF_BC7", 2, 2), Some(16));
        assert_eq!(mip_size("PF_B8G8R8A8", 2, 2), Some(16));
        assert_eq!(mip_size("PF_NOPE", 2, 2), None);
    }

    #[test]
    fn inline_and_bulk_mips_are_read() {
        let names: Vec<String> = ["None", "PF_DXT1"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let mut e = Vec::new();
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend([1, 0, 1, 0]);
        e.extend(1i32.to_le_bytes());
        e.extend(1i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(1i32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(b"PF_DXT1\0");
        e.extend(0i32.to_le_bytes());
        e.extend(2i32.to_le_bytes());
        // A bulk mip of 32 bytes at 0.
        e.extend(1i32.to_le_bytes());
        e.extend(0x0501u32.to_le_bytes());
        e.extend(32i32.to_le_bytes());
        e.extend(32i32.to_le_bytes());
        e.extend(0i64.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        // An inline mip of 8 bytes.
        e.extend(1i32.to_le_bytes());
        e.extend(0x0048u32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(0i64.to_le_bytes());
        e.extend([7; 8]);
        e.extend(4i32.to_le_bytes());
        e.extend(4i32.to_le_bytes());
        let p = Package {
            names,
            exports: vec![super::super::package::Export {
                class: 0,
                sup: 0,
                template: 0,
                outer: 0,
                name: "T".into(),
                flags: 0,
                size: e.len() as i64,
                offset: 0,
                is_asset: true,
            }],
            uexp: e,
            ubulk: vec![3; 32],
            ..Package::default()
        };
        let t = Texture::read(&p, 0).unwrap();
        assert_eq!(t.format, "PF_DXT1");
        assert_eq!(t.mips[0].bytes, vec![3; 32]);
        assert_eq!(t.mips[1].width, 4);
        assert_eq!(t.mips[1].bytes, vec![7; 8]);
    }
}
