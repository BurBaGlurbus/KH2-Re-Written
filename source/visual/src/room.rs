//! Source-bound room loading shared by the room preview and test room.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use data::{
    bar,
    model_texture_pixels::{self, DecodedTexture},
    original_payload,
    probe_source::ProbeSource,
    room_collision,
    room_geometry::{self, RoomModel},
};
use std::{fs::File, io::Read, path::Path};

pub struct Room {
    /// BAR entry name: MAP, SK0 or SK1.
    pub name: [u8; 3],
    pub model: RoomModel,
    pub textures: Vec<DecodedTexture>,
    /// Per chunk, the background group bit of its DOCT group (FUN_140186380
    /// entry byte +1), or `NO_GROUP` for models without a DOCT (sky).
    pub chunk_bits: Vec<u8>,
    /// Texture region per environment ordinal (CLAMP register, §4.6).
    pub regions: std::collections::HashMap<usize, EnvRegion>,
    /// UV-scroll records of the texture footer: (index, u speed, v speed)
    /// in counts per tick (draw-01 §7).
    pub uv_scroll: Vec<(u32, f32, f32)>,
    /// TEXA texture animations of the bank (draw-02 section 1).
    pub texa: Vec<Texa>,
}

/// A TEXA chunk: sprites of a sheet swapped into an environment's texture
/// by a frame script (draw-02 section 1.1).
#[derive(Clone, Debug)]
pub struct Texa {
    /// Target environment ordinal.
    pub env: usize,
    /// Destination in the environment's full texture and sprite size.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// RGBA sprites (`w * h * 4` bytes each).
    pub sprites: Vec<Vec<u8>>,
    /// Frames from the default group: ctrl, duration min, max, sprite.
    pub frames: Vec<[u16; 4]>,
    /// Index of the group's first frame.
    pub start: usize,
}

impl Texa {
    /// Reads a TEXA payload; the sprites decode through `clut` (RGBA of
    /// the environment's palette).
    pub fn read(p: &[u8], clut: &[u8]) -> Option<Self> {
        let u16_at = |o: usize| Some(u16::from_le_bytes(p.get(o..o + 2)?.try_into().ok()?));
        let u32_at = |o: usize| Some(u32::from_le_bytes(p.get(o..o + 4)?.try_into().ok()?));
        let env = u16_at(2)? as usize;
        let stride = u16_at(4)? as usize;
        let bpp = u16_at(6)? as usize;
        let sprite_count = u16_at(0xe)? as usize;
        let (x, y) = ((u16_at(0x10)? & 0xfff) as u32, u16_at(0x12)? as u32);
        let (w, h) = (u16_at(0x14)? as u32, u16_at(0x16)? as u32);
        let group_table = u32_at(0x1c)? as usize;
        let pixel_off = u32_at(0x20)? as usize;
        let default_group = u32_at(0x24)? as usize;
        if !matches!(bpp, 4 | 8) || w == 0 || h == 0 || sprite_count == 0 || sprite_count > 64 {
            return None;
        }
        let bytes_per_sprite = h as usize * bpp * w as usize / 8;
        let mut sprites = Vec::with_capacity(sprite_count);
        for s in 0..sprite_count {
            let raw =
                p.get(pixel_off + s * bytes_per_sprite..pixel_off + (s + 1) * bytes_per_sprite)?;
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for i in 0..(w * h) as usize {
                let index = if bpp == 8 {
                    raw[i] as usize
                } else {
                    // Linear raster, first texel in the low nibble [I].
                    ((raw[i / 2] >> if i % 2 == 0 { 0 } else { 4 }) & 15) as usize
                };
                rgba.extend_from_slice(clut.get(index * 4..index * 4 + 4)?);
            }
            sprites.push(rgba);
        }
        let first = u32_at(group_table + default_group * 4)? as usize;
        let frame_size = 8 + 2 * stride;
        // Frames run from the group start up to the pixel data.
        let count = pixel_off.checked_sub(first)? / frame_size;
        let frames = (0..count)
            .map(|i| {
                let o = first + i * frame_size;
                Some([u16_at(o)?, u16_at(o + 2)?, u16_at(o + 4)?, u16_at(o + 6)?])
            })
            .collect::<Option<Vec<_>>>()?;
        (!frames.is_empty()).then_some(Texa {
            env,
            x,
            y,
            w,
            h,
            sprites,
            frames,
            start: 0,
        })
    }
}

/// One TEXA worker's frame state machine (FUN_1401c0310).
#[derive(Clone, Debug, Default)]
pub struct TexaPlayer {
    cur: Option<usize>,
    started: bool,
    skip: bool,
    elapsed: f32,
    dur: f32,
}

impl TexaPlayer {
    /// One 60 Hz tick; returns the sprite index to show while the current
    /// frame displays one (ctrl nibble 0).
    pub fn tick(&mut self, t: &Texa) -> Option<usize> {
        if !self.started {
            self.started = true;
            self.skip = true;
            self.elapsed = 0.;
            self.cur = Some(t.start);
            self.dur = t.frames[t.start][1] as f32;
        }
        let jump = |c: u16| ((c as i16) >> 4) as isize;
        if let Some(c) = self.cur {
            let ctrl = t.frames[c][0];
            if ctrl & 0xf == 3 {
                return None;
            }
            if ctrl & 0xf == 2 {
                self.cur = (c as isize + jump(ctrl)).try_into().ok();
            }
        }
        if self.skip {
            self.skip = false;
        } else {
            self.elapsed += 1.;
            while self.dur <= self.elapsed {
                self.elapsed -= self.dur;
                let Some(c) = self.cur else { break };
                let mut next = c + 1;
                if next >= t.frames.len() {
                    return None;
                }
                let n = t.frames[next][0];
                if n & 0xf == 2 {
                    let Ok(j) = usize::try_from(next as isize + jump(n)) else {
                        return None;
                    };
                    next = j;
                } else if n & 0xf == 3 {
                    self.cur = Some(next);
                    return None;
                }
                self.cur = Some(next);
                let f = t.frames.get(next)?;
                self.dur = f[1] as f32;
            }
        }
        let c = self.cur?;
        let f = t.frames.get(c)?;
        (f[0] & 0xf == 0).then_some(f[3] as usize)
    }
}

/// Copies sprite `s` into a cropped environment image with the exe's
/// 1-texel border rule (rows 0 and h-1 keep the static texture).
pub fn texa_upload(
    t: &Texa,
    s: usize,
    crop_origin: [u32; 2],
    width: u32,
    height: u32,
    data: &mut [u8],
) {
    let Some(sprite) = t.sprites.get(s) else {
        return;
    };
    let (dx, dy) = (
        t.x.saturating_sub(crop_origin[0]),
        t.y.saturating_sub(crop_origin[1]),
    );
    for row in 1..t.h.saturating_sub(1) {
        let y = dy + row;
        if y >= height {
            break;
        }
        let w = t.w.min(width.saturating_sub(dx)) as usize;
        let src = (row * t.w) as usize * 4;
        let dst = ((y * width + dx) as usize) * 4;
        if dst + w * 4 <= data.len() {
            data[dst..dst + w * 4].copy_from_slice(&sprite[src..src + w * 4]);
        }
    }
}

/// A texture environment's CLAMP-register region (draw-01 §4.6): the exe
/// samples a cropped copy through TexScale; WMS/WMT 0 and 3 wrap, 1 and 2
/// clamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvRegion {
    /// Cropped rectangle in texels: x, y, width, height.
    pub crop: [u32; 4],
    /// uv' = uv * scale + offset (TexScale xy, zw without scroll).
    pub scale: [f32; 2],
    pub offset: [f32; 2],
    pub wrap: [bool; 2],
    /// regionW / texW and regionH / texH for UV scrolling.
    pub scroll_ratio: [f32; 2],
}

impl EnvRegion {
    /// From the CLAMP register fields and the texture size.
    pub fn new(wms: u32, wmt: u32, [minu, maxu, minv, maxv]: [u32; 4], [w, h]: [u32; 2]) -> Self {
        let axis = |mode: u32, min: u32, max: u32, size: u32| -> (u32, u32, f32, f32, bool) {
            match mode {
                2 if max >= min && max < size => {
                    let len = max - min + 1;
                    let s = size as f32 / len as f32;
                    (min, len, s, -(min as f32 / size as f32) * s, false)
                }
                3 if min < size => {
                    let len = min + 1;
                    (0, len, size as f32 / len as f32, 0., true)
                }
                0 => (0, size, 1., 0., true),
                _ => (0, size, 1., 0., false),
            }
        };
        let (x, w2, sx, ox, wu) = axis(wms, minu, maxu, w);
        let (y, h2, sy, oy, wv) = axis(wmt, minv, maxv, h);
        Self {
            crop: [x, y, w2, h2],
            scale: [sx, sy],
            offset: [ox, oy],
            wrap: [wu, wv],
            scroll_ratio: [(minu + 1) as f32 / w as f32, (minv + 1) as f32 / h as f32],
        }
    }

    /// The cropped RGBA copy of a decoded texture.
    pub fn crop_rgba(&self, t: &DecodedTexture) -> (u32, u32, Vec<u8>) {
        let [x, y, w, h] = self.crop;
        if (x, y, w, h) == (0, 0, t.width, t.height) {
            return (t.width, t.height, t.pixels_rgba.clone());
        }
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in y..y + h {
            let start = ((row * t.width + x) * 4) as usize;
            out.extend_from_slice(&t.pixels_rgba[start..start + (w * 4) as usize]);
        }
        (w, h, out)
    }
}

/// Uploads with the exe's sampler: linear filtering and the region's
/// address modes.
pub fn region_image(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    wrap: [bool; 2],
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let handle = rgba_image(width, height, pixels, images);
    if let Some(mut image) = images.get_mut(&handle) {
        let mode = |w: bool| {
            if w {
                ImageAddressMode::Repeat
            } else {
                ImageAddressMode::ClampToEdge
            }
        };
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: mode(wrap[0]),
            address_mode_v: mode(wrap[1]),
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
    }
    handle
}

/// A chunk with no DOCT group: always drawn.
pub const NO_GROUP: u8 = 0xff;

/// Whether a chunk with background group bit `bit` draws under `mask`
/// (draw-01 §2.4: the group is enabled iff `bit < 64 && mask >> bit & 1`).
pub fn chunk_visible(bit: u8, mask: u64) -> bool {
    bit == NO_GROUP || (bit < 64 && mask >> bit & 1 != 0)
}

/// The group bit of every DOCT entry (BAR type 5, 0x1c-byte groups at the
/// offset in header word +0x1c, size in +0x20).
fn doct_bits(doct: &[u8]) -> Option<Vec<u8>> {
    if doct.get(..4)? != b"DOCT" {
        return None;
    }
    let word = |o: usize| -> Option<usize> {
        Some(u32::from_le_bytes(doct.get(o..o + 4)?.try_into().ok()?) as usize)
    };
    let (off, bytes) = (word(0x1c)?, word(0x20)?);
    (0..bytes / 0x1c)
        .map(|i| doct.get(off + i * 0x1c + 1).copied())
        .collect()
}
pub struct LoadedRoom {
    pub rooms: Vec<Room>,
    pub frame_points: Vec<Vec3>,
    pub inspection_ground: Option<f32>,
    /// Collision display triangles in scene coordinates (pinned MapStudio
    /// GenerateVertex conversion).
    pub collision_triangles: Vec<CollisionTriangle>,
    /// Camera collision (BAR type 11, COCT layout) triangles in scene
    /// coordinates.
    pub camera_triangles: Vec<[Vec3; 3]>,
    /// The same camera collision as full COCT records (planes and surface
    /// attributes), for the executable's camera ray.
    pub camera_collisions: Vec<room_collision::RoomCollision>,
    /// The map's type-14 environment record.
    pub environment: Option<Environment>,
}

/// Type-14 environment record (draw-01 §8.1, FUN_14016fa90).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Environment {
    /// Screen clear colour (+0x00).
    pub clear: [u8; 3],
    /// Fog colour (+0x44) and near/far/min/max floats (+0x48..0x58).
    pub fog: crate::area_fog::FogParams,
}

impl Environment {
    pub fn read(d: &[u8]) -> Option<Self> {
        let f = |o: usize| Some(f32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?));
        let (near, far, min, mut max) = (f(0x48)?, f(0x4c)?, f(0x50)?, f(0x54)?);
        if max == 0. {
            max = 255.;
        }
        let b = d.get(..0x58)?;
        Some(Environment {
            clear: [b[0], b[1], b[2]],
            fog: crate::area_fog::FogParams {
                colour: [b[0x44], b[0x45], b[0x46]],
                near,
                far,
                min,
                max,
            },
        })
    }
}

pub struct CollisionTriangle {
    pub polygon: usize,
    /// Raw COCT surface flags word (public labels: bit 0 partyStand,
    /// bit 4 partyCollide).
    pub surface_flags: u32,
    pub corners: [Vec3; 3],
}

/// Pinned MapStudio CollisionModel.GenerateVertex display conversion.
fn scene_point(bits: [u32; 4]) -> Vec3 {
    let [x, y, z, _] = bits.map(f32::from_bits);
    Vec3::new(x, -y, -z)
}

pub fn load_rooms(
    source: &ProbeSource,
    selection: &Path,
) -> Result<LoadedRoom, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(selection)?
        .take(1048577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 {
        return Err("room selection too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let items = value["selections"]
        .as_array()
        .ok_or("room selection absent")?;
    if value["schema_version"] != 1
        || value["source_content_identity"] != source.content_identity()
        || items.len() != 1
    {
        return Err("one source-bound room selection required".into());
    }
    let item = &items[0];
    let mut index = source.open_index(item["index"].as_str().ok_or("room index absent")?)?;
    let ordinal = usize::try_from(item["ordinal"].as_u64().ok_or("room ordinal absent")?)?;
    let entry = index
        .inventory
        .entries
        .get(ordinal)
        .ok_or("room entry absent")?;
    if serde_json::to_value(entry.name_hash)? != item["name_hash"] {
        return Err("room identity mismatch".into());
    }
    let archive = original_payload::read_original(&mut index.reader, entry)?;
    let ground = item
        .get("inspection_ground_polygon")
        .map(|p| p.as_u64().ok_or("inspection polygon ordinal invalid"))
        .transpose()?;
    let room = load_room_archive(&archive.bytes, ground)?;
    index.verify_unchanged()?;
    Ok(room)
}

/// Decodes a map archive (`map/<world><nn>.map`): its map models with their
/// textures, the type-6 collision and the type-11 camera collision.
/// `inspection_ground` optionally names a horizontal collision polygon
/// whose height is reported for inspection framing.
pub fn load_room_archive(
    bytes: &[u8],
    inspection_ground_polygon: Option<u64>,
) -> Result<LoadedRoom, Box<dyn std::error::Error>> {
    let table = bar::read_bar(bytes)?;
    let mut rooms = Vec::new();
    let mut frame_points = Vec::new();
    let mut inspection_ground = None;
    let mut collision_triangles = Vec::new();
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 6 && e.offset > 0 && e.size > 0)
    {
        let collision = room_collision::read(
            &bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        frame_points.extend(collision.vertex_bits.iter().map(|&p| scene_point(p)));
        for (polygon, [a, b, c]) in collision.triangle_indices()? {
            let attribute = collision.polygons[polygon].attribute;
            let surface_flags = usize::try_from(attribute)
                .ok()
                .and_then(|i| collision.attributes.get(i))
                .copied()
                .ok_or("collision surface flags index outside table")?;
            collision_triangles.push(CollisionTriangle {
                polygon,
                surface_flags,
                corners: [a, b, c].map(|i| scene_point(collision.vertex_bits[i as usize])),
            });
        }
        if let Some(polygon) = inspection_ground_polygon {
            if inspection_ground.is_some() {
                return Err("inspection ground polygon ambiguous across collision blocks".into());
            }
            let ordinal = usize::try_from(polygon)?;
            let polygon = collision
                .polygons
                .get(ordinal)
                .ok_or("inspection ground polygon absent")?;
            let heights: Vec<_> = polygon
                .vertices
                .iter()
                .filter(|&&i| i >= 0)
                .map(|&i| -f32::from_bits(collision.vertex_bits[i as usize][1]))
                .collect();
            if heights.len() < 3 || heights.iter().any(|&y| !y.is_finite() || y != heights[0]) {
                return Err(
                    "inspection grounding requires an explicitly selected horizontal polygon"
                        .into(),
                );
            }
            inspection_ground = Some(heights[0]);
        }
    }
    let mut camera_triangles = Vec::new();
    let mut camera_collisions = Vec::new();
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 11 && e.offset > 0 && e.size > 0)
    {
        let collision = room_collision::read(
            &bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        for (_, corners) in collision.triangle_indices()? {
            camera_triangles.push(corners.map(|i| scene_point(collision.vertex_bits[i as usize])));
        }
        camera_collisions.push(collision);
    }
    let mut rgba_bytes = 0;
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 4 && e.offset > 0 && e.size > 0)
    {
        let payload = &bytes[entry.offset as usize..entry.offset as usize + entry.size as usize];
        if payload.get(0x90..0x94) != Some(2u32.to_le_bytes().as_slice()) {
            continue;
        }
        let model = room_geometry::read(payload)?;
        // The DOCT (the BAR's last type-5 entry) binds to the model named
        // MAP only; its entry i is rendering group i [draw-01 §2].
        let mut chunk_bits = vec![NO_GROUP; model.chunks.len()];
        if entry.name_bytes[..3] == *b"MAP"
            && let Some(doct) = table
                .entries
                .iter()
                .rfind(|e| e.entry_type == 5 && e.size > 0)
                .and_then(|e| bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
                .and_then(doct_bits)
            && doct.len() == model.rendering_groups.len()
        {
            for (g, chunks) in model.rendering_groups.iter().enumerate() {
                for &c in chunks {
                    if let Some(b) = chunk_bits.get_mut(c as usize) {
                        *b = doct[g];
                    }
                }
            }
        }
        let textures: Vec<_> = table
            .entries
            .iter()
            .filter(|e| {
                e.entry_type == 7 && e.name_bytes == entry.name_bytes && e.offset > 0 && e.size > 0
            })
            .collect();
        if textures.len() != 1 {
            return Err("room requires unique same-name texture block".into());
        }
        let entry = textures[0];
        let bank = &bytes[entry.offset as usize..entry.offset as usize + entry.size as usize];
        let textures = model_texture_pixels::decode_images(bank)?;
        let mut regions = std::collections::HashMap::new();
        let mut uv_scroll = Vec::new();
        let mut texa = Vec::new();
        if let Ok(layout) = data::model_texture::read_layout(bank) {
            for f in &layout.footer {
                if let Some([i, u, v]) = f.uv_scroll_words {
                    uv_scroll.push((i, f32::from_bits(u), f32::from_bits(v)));
                }
                if &f.tag == b"TEXA"
                    && let Some(payload) = bank.get(f.offset + 8..f.offset + 8 + f.length)
                    && payload.len() >= 4
                {
                    let env = u16::from_le_bytes([payload[2], payload[3]]) as usize;
                    if let Some(t) = textures.iter().find(|t| t.environment_ordinal == env)
                        && let Some(t) = Texa::read(payload, &t.clut_rgba)
                    {
                        texa.push(t);
                    }
                }
            }
            for (ordinal, env) in layout.environments.iter().enumerate() {
                let (w, h) = (1u32 << env.tex0[3], 1u32 << env.tex0[4]);
                let a = env.address;
                regions.insert(
                    ordinal,
                    EnvRegion::new(a[0], a[1], [a[2], a[3], a[4], a[5]], [w, h]),
                );
            }
        }
        rgba_bytes += textures.iter().map(|t| t.pixels_rgba.len()).sum::<usize>();
        if rgba_bytes > 128 * 1024 * 1024 {
            return Err("room textures exceed cumulative limit".into());
        }
        rooms.push(Room {
            name: [
                entry.name_bytes[0],
                entry.name_bytes[1],
                entry.name_bytes[2],
            ],
            model,
            textures,
            chunk_bits,
            regions,
            uv_scroll,
            texa,
        });
    }
    if rooms.is_empty() {
        return Err("no decoded room model".into());
    }
    if frame_points.is_empty() || frame_points.iter().any(|p| !p.is_finite()) {
        return Err("finite collision area required for scene framing".into());
    }
    Ok(LoadedRoom {
        rooms,
        frame_points,
        inspection_ground,
        collision_triangles,
        camera_triangles,
        camera_collisions,
        environment: table
            .entries
            .iter()
            .find(|e| e.entry_type == 14 && e.size >= 0x58)
            .and_then(|e| bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
            .and_then(Environment::read),
    })
}

pub fn upload(texture: &DecodedTexture, images: &mut Assets<Image>) -> Handle<Image> {
    rgba_image(
        texture.width,
        texture.height,
        texture.pixels_rgba.clone(),
        images,
    )
}

pub fn rgba_image(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

pub fn mesh(positions: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, colors: Vec<[f32; 4]>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

/// Updates an animated part without replacing its CPU vertex allocation.
/// The caller still obtains the mesh through `Assets::get_mut`, so Bevy's
/// normal asset change tracking and GPU upload path are preserved.
pub fn update_positions(mesh: &mut Mesh, positions: impl IntoIterator<Item = [f32; 3]>) {
    if let Some(VertexAttributeValues::Float32x3(values)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        values.splice(.., positions);
    } else {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            positions.into_iter().collect::<Vec<_>>(),
        );
    }
}

/// Generated arena geometry (not game data): textured triangles in scene
/// coordinates with a tiling texture.
pub struct ArenaMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub texture: (u32, u32, Vec<u8>),
}

/// COCT-style surface flags used by the controller (public labels).
pub const ARENA_FLOOR: u32 = 1;
pub const ARENA_WALL: u32 = 1 << 4;

/// A square test arena centred on the origin: a tiled floor at y = 0 and
/// four walls. Collision and camera collision come from the same triangles.
pub fn arena(half_size: f32, wall_height: f32, tile: f32) -> (LoadedRoom, ArenaMesh) {
    let mut collision_triangles = Vec::new();
    let mut camera_triangles = Vec::new();
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut quad = |c: [Vec3; 4], uv: [[f32; 2]; 4], shade: f32, flags: u32, camera: bool| {
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            for &i in &tri {
                positions.push(c[i].to_array());
                uvs.push(uv[i]);
                colors.push([shade, shade, shade, 1.]);
            }
            let corners = tri.map(|i| c[i]);
            collision_triangles.push(CollisionTriangle {
                polygon: collision_triangles.len(),
                surface_flags: flags,
                corners,
            });
            if camera {
                camera_triangles.push(corners);
            }
        }
    };
    let n = (2. * half_size / tile).ceil().max(1.) as i32;
    let cell = 2. * half_size / n as f32;
    for ix in 0..n {
        for iz in 0..n {
            let (x0, z0) = (-half_size + ix as f32 * cell, -half_size + iz as f32 * cell);
            let (x1, z1) = (
                -half_size + (ix + 1) as f32 * cell,
                -half_size + (iz + 1) as f32 * cell,
            );
            let shade = if (ix + iz) % 2 == 0 { 1. } else { 0.92 };
            quad(
                [
                    Vec3::new(x0, 0., z0),
                    Vec3::new(x0, 0., z1),
                    Vec3::new(x1, 0., z1),
                    Vec3::new(x1, 0., z0),
                ],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                shade,
                ARENA_FLOOR,
                false,
            );
        }
    }
    let h = wall_height;
    let s = half_size;
    let walls = [
        [Vec3::new(-s, 0., -s), Vec3::new(s, 0., -s)],
        [Vec3::new(s, 0., -s), Vec3::new(s, 0., s)],
        [Vec3::new(s, 0., s), Vec3::new(-s, 0., s)],
        [Vec3::new(-s, 0., s), Vec3::new(-s, 0., -s)],
    ];
    for [a, b] in walls {
        let segments = ((b - a).length() / tile).round().max(1.) as i32;
        for k in 0..segments {
            let p0 = a.lerp(b, k as f32 / segments as f32);
            let p1 = a.lerp(b, (k + 1) as f32 / segments as f32);
            quad(
                [p0, p0 + Vec3::Y * h, p1 + Vec3::Y * h, p1],
                [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
                0.7,
                ARENA_WALL,
                true,
            );
        }
    }
    // Tile texture: dark slate with a lighter border and centre mark.
    let size = 128u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let edge = x < 3 || y < 3 || x >= size - 3 || y >= size - 3;
            let mark = (x as i32 - 64).abs() < 2 && (y as i32 - 64).abs() < 10
                || (y as i32 - 64).abs() < 2 && (x as i32 - 64).abs() < 10;
            let c: [u8; 3] = if edge {
                [150, 170, 200]
            } else if mark {
                [110, 125, 150]
            } else {
                [58, 66, 84]
            };
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    let frame_points = vec![Vec3::new(-s, 0., -s), Vec3::new(s, h, s)];
    (
        LoadedRoom {
            rooms: Vec::new(),
            frame_points,
            inspection_ground: Some(0.),
            collision_triangles,
            camera_triangles,
            camera_collisions: Vec::new(),
            environment: None,
        },
        ArenaMesh {
            positions,
            uvs,
            colors,
            texture: (size, size, rgba),
        },
    )
}

#[cfg(test)]
mod texa_tests {
    use super::*;

    #[test]
    fn animated_positions_keep_storage_and_exact_vertex_bytes() {
        let mut reused = mesh(vec![[0.; 3]; 12], vec![[0.25, 0.75]; 12], vec![[1.; 4]; 12]);
        let mut replaced = reused.clone();
        let storage = match reused.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(values) => (values.as_ptr(), values.capacity()),
            _ => unreachable!(),
        };
        // Preserve signed zero, subnormals, infinities and NaN payloads too:
        // this presentation helper copies bits and does not reinterpret them.
        let words = [0, 0x80000000, 1, 0x3f800001, 0x7f800000, 0x7fc01234];
        for count in [12, 3, 0, 9, 12] {
            let positions = || {
                (0..count).map(|i| {
                    std::array::from_fn(|axis| f32::from_bits(words[(i + axis) % words.len()]))
                })
            };
            replaced.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions().collect::<Vec<_>>());
            update_positions(&mut reused, positions());
            assert_eq!(
                reused.create_packed_vertex_buffer_data(),
                replaced.create_packed_vertex_buffer_data()
            );
            match reused.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
                VertexAttributeValues::Float32x3(values) => {
                    assert_eq!(values.as_ptr(), storage.0);
                    assert_eq!(values.capacity(), storage.1);
                    assert_eq!(values.len(), count);
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn animated_positions_create_and_grow_missing_storage() {
        let mut reused = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        );
        let mut replaced = reused.clone();
        for count in [0, 3, 257, 1] {
            let positions = || (0..count).map(|i| [i as f32, -0., 0.5]);
            replaced.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions().collect::<Vec<_>>());
            update_positions(&mut reused, positions());
            assert_eq!(
                reused.create_packed_vertex_buffer_data(),
                replaced.create_packed_vertex_buffer_data()
            );
        }
    }

    #[test]
    fn arena_floor_ends_at_walls_when_tile_does_not_divide_width() {
        let (room, mesh) = arena(1000., 800., 300.);
        let floor: Vec<_> = room
            .collision_triangles
            .iter()
            .filter(|t| t.surface_flags == ARENA_FLOOR)
            .collect();
        assert_eq!(floor.len(), 7 * 7 * 2);
        for t in &floor {
            for p in t.corners {
                assert!((-1000. ..=1000.).contains(&p.x));
                assert!((-1000. ..=1000.).contains(&p.z));
                assert_eq!(p.y, 0.);
            }
        }
        for edge in [-1000., 1000.] {
            assert!(floor.iter().any(|t| t.corners.iter().any(|p| p.x == edge)));
            assert!(floor.iter().any(|t| t.corners.iter().any(|p| p.z == edge)));
        }
        assert_eq!(mesh.positions.len(), room.collision_triangles.len() * 3);
    }

    fn texa(frames: Vec<[u16; 4]>) -> Texa {
        Texa {
            env: 0,
            x: 2,
            y: 1,
            w: 2,
            h: 4,
            sprites: vec![vec![1; 2 * 4 * 4], vec![2; 2 * 4 * 4]],
            frames,
            start: 0,
        }
    }

    #[test]
    fn frames_hold_then_show_sprites_and_jump_back() {
        // hold 2, sprite 0 for 2, sprite 1 for 2, jump back 3 frames.
        let t = texa(vec![
            [1, 2, 2, 0],
            [0, 2, 2, 0],
            [0, 2, 2, 1],
            [0xfff2, 0, 0, 0],
        ]);
        // 0xfff2: ctrl nibble 2, jump (i16)0xfff2 >> 4 = -1 frame.
        let mut p = TexaPlayer::default();
        let mut seen = Vec::new();
        for _ in 0..14 {
            seen.push(p.tick(&t));
        }
        assert_eq!(seen[0], None);
        assert!(seen.contains(&Some(0)));
        assert!(seen.contains(&Some(1)));
        // The cycle repeats sprite 1 after the jump.
        assert_eq!(seen.last().copied().flatten(), Some(1));
    }

    #[test]
    fn upload_keeps_the_border_rows() {
        let t = texa(vec![[0, 1, 1, 1]]);
        let (w, h) = (4u32, 6u32);
        let mut data = vec![9u8; (w * h * 4) as usize];
        texa_upload(&t, 1, [0, 0], w, h, &mut data);
        let px = |x: u32, y: u32| data[((y * w + x) * 4) as usize];
        assert_eq!(px(2, 1), 9, "row 0 of the sprite stays");
        assert_eq!(px(2, 2), 2);
        assert_eq!(px(3, 3), 2);
        assert_eq!(px(2, 4), 9, "last sprite row stays");
        assert_eq!(px(1, 2), 9, "outside the rectangle");
    }
}
