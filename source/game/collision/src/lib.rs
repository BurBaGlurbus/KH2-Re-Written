//! Body-versus-map collision, translated from the PC exe: the COCT world
//! (FUN_1401722f0), the sphere sweep (FUN_1401752e0 → FUN_140174640 /
//! FUN_140175190), the polygon gather (FUN_1401741c0), the segment ray
//! (FUN_140173b30 / FUN_14018b4d0) and the floor probe (FUN_140172840 /
//! FUN_140172640), driven by FUN_14016fec0 for the static map.
//!
//! Coordinates are the engine's (Y down: "up" is −y). The static map's
//! world matrix is the identity (W+0xe5 = 1), so no transforms are applied.
use data::room_collision::RoomCollision;

pub type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
/// FUN_140140200 (three components).
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// FUN_140142120.
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
/// FUN_140142080: normalizes in place unless the length is 0; returns the
/// length.
fn normalize(a: &mut V3) -> f32 {
    let l = dot(*a, *a).sqrt();
    if l != 0. {
        *a = scale(*a, 1. / l);
    }
    l
}
/// FUN_1401a8f20.
fn dist2(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

/// Push clearance (DAT_1405b2288).
pub const CLEARANCE: f32 = 0.008;
/// Query half-size factor × radius (DAT_1405b22e0).
pub const QUERY_SCALE: f32 = 1.5;
/// Knee offsets: airborne pass 1 (DAT_140623a4c) and pass 2 (DAT_1405b22dc).
pub const KNEE_AIR: f32 = 0.5;
pub const KNEE_EDGE: f32 = 1.02;
/// Point-in-polygon tolerance (DAT_1406239d8).
pub const FACE_TOLERANCE: f32 = 0.01;
/// Floor ray depth × offset (DAT_1405a9cb8) and its rise clamp
/// (DAT_140623be4).
pub const RAY_DEPTH: f32 = 3.0;
pub const RAY_RISE_MAX: f32 = 100.0;
/// Floor height reported without a floor.
pub const NO_FLOOR: f32 = 320000.0;
/// Most polygons one gather collects (`0x3fe < count` stops).
pub const GATHER_MAX: usize = 0x3ff;
/// Pushes past which a grounded sub-step is reverted.
pub const PUSH_LIMIT: u32 = 9;
/// Edge successor tables DAT_14071ba50 (triangle) / DAT_14071ba60 (quad).
const NEXT3: [usize; 3] = [1, 2, 0];
const NEXT4: [usize; 4] = [1, 2, 3, 0];

struct Poly {
    verts: [u16; 4],
    plane: u16,
    bbox: u16,
    attr: u32,
    ground: u8,
}

struct Mesh {
    bbox: [i16; 6],
    polys: [u16; 2],
    visible: bool,
    group: u8,
}

struct Node {
    children: [i16; 8],
    bbox: [i16; 6],
    meshes: [u16; 2],
}

/// A collision world `W` (one COCT) with its mesh visibility.
pub struct World {
    nodes: Vec<Node>,
    meshes: Vec<Mesh>,
    polys: Vec<Poly>,
    verts: Vec<V3>,
    planes: Vec<[f32; 4]>,
    bboxes: Vec<[i16; 6]>,
    /// COCT+0x0c: int16 box quantisation index.
    quant: usize,
}

/// A camera segment hit (see `World::camera_ray`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraHit {
    pub point: V3,
    pub normal: V3,
    pub surface: u32,
}

/// One polygon for presentation (collision overlays).
pub struct PolyView {
    pub corners: Vec<V3>,
    pub attr: u32,
    pub visible: bool,
}

impl World {
    /// FUN_1401722f0: every mesh starts hidden; meshes whose group bit is
    /// set in `group_mask` become visible.
    pub fn new(c: &RoomCollision, group_mask: u64) -> Result<Self, &'static str> {
        let attr_of = |i: i16| -> Result<u32, &'static str> {
            usize::try_from(i)
                .ok()
                .and_then(|i| c.attributes.get(i))
                .copied()
                .ok_or("polygon attribute outside table")
        };
        let polys = c
            .polygons
            .iter()
            .map(|p| {
                Ok(Poly {
                    verts: p.vertices.map(|v| v as u16),
                    plane: p.plane as u16,
                    bbox: p.bounding_box as u16,
                    attr: attr_of(p.attribute)?,
                    ground: p.ground,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let verts: Vec<V3> = c
            .vertex_bits
            .iter()
            .map(|v| [0, 1, 2].map(|i| f32::from_bits(v[i])))
            .collect();
        for p in &polys {
            let n = if p.verts[3] == 0xffff { 3 } else { 4 };
            if p.verts[..n].iter().any(|&v| v as usize >= verts.len())
                || p.plane as usize >= c.plane_bits.len()
                || (p.bbox != 0xffff && p.bbox as usize >= c.bounding_boxes.len())
            {
                return Err("polygon reference outside table");
            }
        }
        let meshes = c
            .meshes
            .iter()
            .map(|m| Mesh {
                bbox: m.bounds,
                polys: m.polygon_range,
                visible: m.group < 64 && group_mask >> m.group & 1 != 0,
                group: m.group,
            })
            .collect();
        Ok(Self {
            nodes: c
                .nodes
                .iter()
                .map(|n| Node {
                    children: n.children,
                    bbox: n.bounds,
                    meshes: n.mesh_range,
                })
                .collect(),
            meshes,
            polys,
            verts,
            planes: c.plane_bits.iter().map(|p| p.map(f32::from_bits)).collect(),
            bboxes: c.bounding_boxes.clone(),
            quant: (c.kind.clamp(0, 3)) as usize,
        })
    }

    /// FUN_1401715a0 / FUN_140174440: switch a background group's meshes.
    pub fn set_group(&mut self, group: u8, on: bool) {
        for m in self.meshes.iter_mut().filter(|m| m.group == group) {
            m.visible = on;
        }
    }

    pub fn polygons(&self) -> Vec<PolyView> {
        let mut visible = vec![false; self.polys.len()];
        for m in &self.meshes {
            for p in m.polys[0]..m.polys[1] {
                if let Some(v) = visible.get_mut(p as usize) {
                    *v |= m.visible;
                }
            }
        }
        self.polys
            .iter()
            .zip(visible)
            .map(|(p, visible)| PolyView {
                corners: self.corners(p),
                attr: p.attr,
                visible,
            })
            .collect()
    }

    fn corners(&self, p: &Poly) -> Vec<V3> {
        let n = if p.verts[3] == 0xffff { 3 } else { 4 };
        p.verts[..n]
            .iter()
            .map(|&v| self.verts[v as usize])
            .collect()
    }

    fn plane(&self, p: &Poly) -> [f32; 4] {
        self.planes[p.plane as usize]
    }

    /// FUN_1401741c0 with the body filter `(attr & 0xf) != 4`
    /// (FUN_1401745c0(W, 0xf, 4, 1, 0)). Body and floor queries clear
    /// W+0xe7, so the nibble-5 class bypass never applies here.
    fn gather(
        &self,
        node: usize,
        bx: &[i16; 6],
        class: u32,
        skip_nibble4: bool,
        out: &mut Vec<u16>,
    ) {
        let Some(n) = self.nodes.get(node) else {
            return;
        };
        if !overlap16(&n.bbox, bx) {
            return;
        }
        for mi in n.meshes[0]..n.meshes[1] {
            let Some(m) = self.meshes.get(mi as usize) else {
                continue;
            };
            if !m.visible || !overlap16(&m.bbox, bx) {
                continue;
            }
            for pi in m.polys[0]..m.polys[1] {
                let Some(p) = self.polys.get(pi as usize) else {
                    continue;
                };
                let a = p.attr;
                if skip_nibble4 && a & 0xf == 4 {
                    continue;
                }
                let pass = match class {
                    0 | 6 => a & 0x10 != 0,
                    1 => a & 0x20 != 0,
                    2 => a & 0x40 != 0,
                    3 => a & 0x10 != 0 || a & 0x80000 != 0,
                    4 => a & 0x80 != 0,
                    5 => a & 0x100 != 0,
                    _ => true,
                };
                if !pass {
                    continue;
                }
                if p.bbox != 0xffff && !overlap16(&self.bboxes[p.bbox as usize], bx) {
                    continue;
                }
                if out.len() > GATHER_MAX - 1 {
                    return;
                }
                out.push(pi);
            }
        }
        for &child in &n.children {
            if child == -1 {
                return;
            }
            self.gather(child as usize, bx, class, skip_nibble4, out);
        }
    }

    /// FUN_140184970: a float box to the int16 query box.
    fn quantize(&self, b: [f32; 6]) -> [i16; 6] {
        const OFF: [f32; 4] = [1., 10., 20., 100.];
        const SCALE: [f32; 4] = [1., 0.1, 0.05, 0.01];
        let (off, s) = (OFF[self.quant], SCALE[self.quant]);
        let q = |v: f32| {
            let v = if self.quant != 0 { v * s } else { v };
            v.clamp(-32767., 32767.) as i32 as i16
        };
        [
            q(b[0] - off),
            q(b[1] - off),
            q(b[2] - off),
            q(off + b[3]),
            q(off + b[4]),
            q(off + b[5]),
        ]
    }

    /// FUN_14018b760: the point lies inside the polygon's edges.
    fn inside(&self, p: &Poly, point: V3, normal: V3) -> bool {
        let c = self.corners(p);
        let next: &[usize] = if c.len() == 3 { &NEXT3 } else { &NEXT4 };
        for (i, &n) in next.iter().enumerate() {
            let edge = sub(c[n], c[i]);
            let to = sub(point, c[i]);
            if dot(cross(to, edge), normal) > FACE_TOLERANCE {
                return false;
            }
        }
        true
    }

    /// FUN_140173b30: nearest front-facing hit of a segment.
    pub fn ray(&self, p0: V3, p1: V3, class: u32) -> Option<(V3, u16)> {
        self.ray_filtered(p0, p1, class, true)
    }

    /// The camera's segment query (FUN_140171210): the same ray with the
    /// polygon filter cleared (FUN_1401745c0(W, 0, 0, 0, 0)), so nibble-4
    /// polygons count, and class 0xf. Returns the hit point, the polygon's
    /// stored plane normal and its surface nibble (`attr & 0xf`).
    pub fn camera_ray(&self, p0: V3, p1: V3) -> Option<CameraHit> {
        let (point, poly) = self.ray_filtered(p0, p1, 0xf, false)?;
        let p = self.polys.get(poly as usize)?;
        let plane = self.plane(p);
        Some(CameraHit {
            point,
            normal: [plane[0], plane[1], plane[2]],
            surface: p.attr & 0xf,
        })
    }

    fn ray_filtered(&self, p0: V3, p1: V3, class: u32, skip_nibble4: bool) -> Option<(V3, u16)> {
        let lo = [p0[0].min(p1[0]), p0[1].min(p1[1]), p0[2].min(p1[2])];
        let hi = [p0[0].max(p1[0]), p0[1].max(p1[1]), p0[2].max(p1[2])];
        let bx = self.quantize([lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]]);
        let mut list = Vec::new();
        self.gather(0, &bx, class, skip_nibble4, &mut list);
        let dir = sub(p1, p0);
        let mut best = 1.0e14f32;
        let mut hit = None;
        for &pi in &list {
            let p = &self.polys[pi as usize];
            let pl = self.plane(p);
            let n = [pl[0], pl[1], pl[2]];
            // FUN_14018b4d0.
            let denom = dot(n, dir);
            if denom >= 0. || denom.is_nan() {
                continue;
            }
            let v0 = self.verts[p.verts[0] as usize];
            let t = (dot(n, v0) - dot(n, p0)) / denom;
            if !(0. ..=1.0).contains(&t) {
                continue;
            }
            let point = add(scale(dir, t), p0);
            if !self.inside(p, point, n) {
                continue;
            }
            let d = dist2(point, p0);
            if d < best {
                best = d;
                hit = Some((point, pi));
            }
        }
        hit
    }
}

/// FUN_1401842d0 (int16, inclusive).
fn overlap16(a: &[i16; 6], b: &[i16; 6]) -> bool {
    b[0] <= a[3] && a[0] <= b[3] && b[1] <= a[4] && a[1] <= b[4] && b[2] <= a[5] && a[2] <= b[5]
}

/// FUN_140184280 (float, inclusive).
fn overlapf(a: &[f32; 6], b: &[f32; 6]) -> bool {
    b[0] <= a[3] && a[0] <= b[3] && b[1] <= a[4] && a[1] <= b[4] && b[2] <= a[5] && a[2] <= b[5]
}

fn box_around(c: V3, half: f32) -> [f32; 6] {
    [
        c[0] - half,
        c[1] - half,
        c[2] - half,
        c[0] + half,
        c[1] + half,
        c[2] + half,
    ]
}

/// A body's collision record (`CB`, obj+0x6d0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub radius: f32,
    pub height: f32,
    /// Sphere centre above the feet.
    pub offset: f32,
    /// Body mode (CB+0x70 bits 1–2): 1 when the object-collision entry's
    /// kind is not 0.
    pub mode: u32,
    /// Collision class (CB+0x70 bits 3–6): 0 for the player.
    pub class: u32,
}

/// One tick's query.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Query {
    /// Feet position.
    pub start: V3,
    pub displacement: V3,
    /// CB+0x70 bit0.
    pub airborne: bool,
    /// CB+0x84: 30 while riding another object.
    pub extra_snap: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Solved {
    /// Result feet position.
    pub position: V3,
    /// CB+0x68.
    pub floor_y: f32,
    /// CB+0x60.
    pub floor_polygon: Option<u16>,
    /// CB+0x74 bit0: no floor (walk-off when grounded, no landing in air).
    pub no_floor: bool,
    /// CB+0x74 bit1: the floor ray missed.
    pub ray_missed: bool,
    /// CB+0x74 bit2: the sphere touched something.
    pub touched: bool,
    /// Contacts with attribute bit 11 / 12 polygons (CB+0x40 / +0x50).
    pub touch_11: Option<(u16, V3)>,
    pub touch_12: Option<(u16, V3)>,
}

/// Floor information (FUN_140170c10, obj+0x758) for the next tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorInfo {
    /// Plane normal of the floor, (0, −1, 0) without one.
    pub normal: V3,
    /// Poly byte 0 (`G+0x28`).
    pub ground: u8,
    /// Attribute bit 16 (slide surface).
    pub slide: bool,
}

struct Cb {
    body: Body,
    q: Query,
    result: V3,
    res: u32,
    touch_11: Option<(u16, V3)>,
    touch_12: Option<(u16, V3)>,
}

/// FUN_14016fec0 for the static map (no dynamic colliders).
pub fn solve(w: &World, body: &Body, q: &Query) -> Solved {
    let mut cb = Cb {
        body: *body,
        q: *q,
        result: q.start,
        res: 0,
        touch_11: None,
        touch_12: None,
    };
    sweep(w, &mut cb);
    let mut out = Solved {
        floor_y: NO_FLOOR,
        ..Default::default()
    };
    let floor = floor_probe(w, &mut cb, &mut out);
    out.floor_polygon = floor;
    out.position = cb.result;
    out.no_floor = cb.res & 1 != 0;
    out.ray_missed = cb.res & 2 != 0;
    out.touched = cb.res & 4 != 0;
    out.touch_11 = cb.touch_11;
    out.touch_12 = cb.touch_12;
    out
}

/// FUN_140170c10 for the map world.
pub fn floor_info(w: &World, s: &Solved) -> FloorInfo {
    match s.floor_polygon {
        None => FloorInfo {
            normal: [0., -1., 0.],
            ground: 0,
            slide: false,
        },
        Some(pi) => {
            let p = &w.polys[pi as usize];
            let pl = w.plane(p);
            FloorInfo {
                normal: [pl[0], pl[1], pl[2]],
                ground: p.ground,
                slide: p.attr >> 16 & 1 != 0,
            }
        }
    }
}

/// FUN_1401752e0.
fn sweep(w: &World, cb: &mut Cb) -> bool {
    let class = cb.body.class;
    if class == 6 {
        cb.res = cb.res & !2 | 4;
    }
    let r = cb.body.radius;
    let c0 = [cb.q.start[0], cb.q.start[1] - cb.body.offset, cb.q.start[2]];
    let c1 = add(c0, cb.q.displacement);
    let d = sub(c1, c0);
    let n = ((dot(d, d).sqrt() / r) as i32 + 1) as u32;
    let step = if n == 1 { d } else { scale(d, 1.0 / n as f32) };
    let half = r * QUERY_SCALE;
    let around = box_around(c1, half);
    let bx = w.quantize([
        c0[0].min(around[0]),
        c0[1].min(around[1]),
        c0[2].min(around[2]),
        c0[0].max(around[3]),
        c0[1].max(around[4]),
        c0[2].max(around[5]),
    ]);
    let mut list = Vec::new();
    w.gather(0, &bx, class, true, &mut list);
    let ret = if list.is_empty() {
        cb.result = c1;
        false
    } else {
        let mut p = c0;
        for _ in 0..n {
            let prev = p;
            p = add(p, step);
            let mut it = 1;
            let qbox = box_around(p, half);
            let k = push(w, &list, &mut p, r, &qbox, cb);
            if k == -1 {
                cb.result = prev;
                cb.res |= 4;
            } else if k == 0 {
                cb.result = p;
            } else {
                cb.result = p;
                retest(w, &list, cb, &mut p, half, &mut it, prev);
                cb.res |= 4;
            }
            p = cb.result;
        }
        if class != 6 && cb.result != c0 && w.ray(c0, cb.result, class).is_some() {
            cb.result = cb.q.start;
            return false;
        }
        cb.res >> 2 & 1 != 0
    };
    cb.result[1] += cb.body.offset;
    ret
}

/// FUN_140175190.
fn retest(w: &World, list: &[u16], cb: &mut Cb, p: &mut V3, half: f32, it: &mut u32, prev: V3) {
    *it += 1;
    let qbox = box_around(*p, half);
    let k = push(w, list, p, cb.body.radius, &qbox, cb);
    if k == -1 {
        cb.result = prev;
    } else if k == 0 {
        cb.result = *p;
        return;
    } else {
        // `push` leaves the pushed centre in *p; FUN_140174640 writes it to
        // CB.result (param_6 = CB+0x30).
        cb.result = *p;
        if *it > 5 {
            return;
        }
        *p = cb.result;
        retest(w, list, cb, p, half, it, prev);
    }
    cb.res |= 4;
}

fn record_touch(cb: &mut Cb, attr: u32, poly: u16, at: V3) {
    if cb.body.class == 0 {
        if attr >> 11 & 1 != 0 {
            cb.touch_11 = Some((poly, at));
        }
        if attr >> 12 & 1 != 0 {
            cb.touch_12 = Some((poly, at));
        }
    }
}

/// FUN_140174640: pushes the sphere at `c` out of the listed polygons.
/// Returns the push count, or −1 when a grounded body needed more than 9.
fn push(w: &World, list: &[u16], c: &mut V3, r: f32, qbox: &[f32; 6], cb: &mut Cb) -> i32 {
    let mode = cb.body.mode;
    let air = cb.q.airborne;
    let mut count = 0u32;
    let mut edges = Vec::new();
    for &pi in list {
        let p = &w.polys[pi as usize];
        if p.attr >> 20 & 1 != 0 {
            continue;
        }
        let nib = p.attr & 0xf;
        let pl = w.plane(p);
        let n = [pl[0], pl[1], pl[2]];
        if mode != 1 {
            if mode == 2 && nib == 1 {
                continue;
            }
        } else if nib != 1 {
            // Knee sphere (static map, mode 1).
            let o = if !air {
                cb.body.radius
            } else {
                cb.body.offset - cb.body.height * KNEE_AIR
            };
            let knee = [c[0], c[1] + o, c[2]];
            let d = dot(knee, n) + pl[3];
            if d <= r && 0.0 <= d && overlapf(&poly_box(w, p), qbox) {
                let contact = sub(knee, scale(n, d));
                if w.inside(p, contact, n) {
                    record_touch(cb, p.attr, pi, contact);
                    let pushed = add(contact, scale(n, CLEARANCE + r));
                    *c = [pushed[0], pushed[1] - o, pushed[2]];
                }
            }
        }
        let d = dot(*c, n) + pl[3];
        if !(d <= r && 0.0 <= d) || !overlapf(&poly_box(w, p), qbox) {
            continue;
        }
        let contact = sub(*c, scale(n, d));
        if !w.inside(p, contact, n) {
            edges.push(pi);
            continue;
        }
        record_touch(cb, p.attr, pi, contact);
        *c = add(contact, scale(n, CLEARANCE + r));
        count += 1;
        if !air && count > PUSH_LIMIT {
            return -1;
        }
    }
    for &pi in &edges {
        let p = &w.polys[pi as usize];
        if p.attr >> 20 & 1 != 0 {
            continue;
        }
        let nib = p.attr & 0xf;
        if mode != 1 {
            if mode == 2 && nib == 1 {
                continue;
            }
        } else if nib != 1 && air {
            let o = cb.body.offset - cb.body.height * KNEE_EDGE;
            let knee = [c[0], c[1] + o, c[2]];
            if let Some(q) = closest(w, p, knee, r) {
                record_touch(cb, p.attr, pi, q);
                let mut away = sub(knee, q);
                normalize(&mut away);
                let pushed = add(q, scale(away, r + CLEARANCE));
                *c = [pushed[0], pushed[1] - o, pushed[2]];
            }
        }
        if let Some(q) = closest(w, p, *c, r) {
            record_touch(cb, p.attr, pi, q);
            let mut away = sub(*c, q);
            normalize(&mut away);
            *c = add(q, scale(away, CLEARANCE + r));
            count += 1;
            if !air && count > PUSH_LIMIT {
                return -1;
            }
        }
    }
    count as i32
}

/// FUN_1401838e0: a polygon's AABB.
fn poly_box(w: &World, p: &Poly) -> [f32; 6] {
    let c = w.corners(p);
    let mut b = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
    for v in c {
        for i in 0..3 {
            b[i] = b[i].min(v[i]);
            b[i + 3] = b[i + 3].max(v[i]);
        }
    }
    b
}

/// FUN_140178cb0: the closest edge point within `r`, else the nearest
/// vertex within `r`.
fn closest(w: &World, p: &Poly, c: V3, r: f32) -> Option<V3> {
    let v = w.corners(p);
    let next: &[usize] = if v.len() == 3 { &NEXT3 } else { &NEXT4 };
    let r2 = r * r;
    let mut found: Vec<V3> = Vec::new();
    for (i, &n) in next.iter().enumerate() {
        let (a, b) = (v[i], v[n]);
        let d = sub(b, a);
        let len2 = dot(d, d);
        let q = if len2 != 0. {
            add(scale(d, -(dot(sub(a, c), d) / len2)), a)
        } else {
            a
        };
        if dist2(c, q) <= r2 {
            let hi = add(a, [CLEARANCE; 3]);
            let lo = add(b, [CLEARANCE; 3]);
            if (q[0] < hi[0]) == (lo[0] < q[0])
                || (q[1] < hi[1]) == (lo[1] < q[1])
                || (q[2] < hi[2]) == (lo[2] < q[2])
            {
                found.push(q);
            }
        }
    }
    match found.len() {
        0 => {
            let mut best = r2;
            let mut pick = None;
            for &vv in &v {
                let d = dist2(c, vv);
                if d <= best {
                    best = d;
                    pick = Some(vv);
                }
            }
            pick
        }
        1 => Some(found[0]),
        _ => {
            let mut best = 1.0e14f32;
            let mut pick = 0;
            for (i, &q) in found.iter().enumerate() {
                let d = dist2(c, q);
                if d < best {
                    best = d;
                    pick = i;
                }
            }
            Some(found[pick])
        }
    }
}

/// FUN_140172840 / FUN_140172640. Returns the floor polygon (CB+0x60).
fn floor_probe(w: &World, cb: &mut Cb, out: &mut Solved) -> Option<u16> {
    let mut dy = cb.q.displacement[1].abs();
    if cb.body.class == 0 {
        dy = if 0.0 <= dy { dy.min(RAY_RISE_MAX) } else { 0. };
    }
    let r = cb.result;
    let p0 = [r[0], r[1] - cb.body.offset - dy, r[2]];
    let p1 = [r[0], r[1] + cb.body.offset * RAY_DEPTH, r[2]];
    match w.ray(p0, p1, cb.body.class) {
        None => {
            out.floor_y = NO_FLOOR;
            cb.res |= 2;
            cb.res |= 1;
            None
        }
        Some((pt, pi)) => {
            out.floor_y = pt[1];
            cb.res &= !2;
            let p = &w.polys[pi as usize];
            if !cb.q.airborne {
                let mut y = cb.q.start[1];
                if 0.0 < cb.q.displacement[1] {
                    y += cb.q.displacement[1];
                }
                if cb.body.offset - cb.body.height + y + cb.q.extra_snap - 1.0 < pt[1] {
                    cb.res |= 1;
                    return Some(pi);
                }
                cb.res &= !1;
            } else {
                // FUN_14018b3b0: the side of the floor plane the feet are on.
                let pl = w.plane(p);
                let v0 = w.verts[p.verts[0] as usize];
                let s = dot(sub(v0, cb.result), [pl[0], pl[1], pl[2]]);
                let side = if s < -0.001 {
                    1
                } else if 0.001 < s {
                    0
                } else {
                    2
                };
                if side != 0 {
                    cb.res |= 1;
                    return Some(pi);
                }
                if p.attr & 0xf == 2 {
                    cb.res |= 1;
                } else {
                    cb.res &= !1;
                }
            }
            cb.result = pt;
            Some(pi)
        }
    }
}

/// FUN_1403b9090's ground projection: the displacement turned into the
/// floor plane, keeping its length. `down` is obj+0x768 (−normal).
pub fn project_on_floor(v: V3, down: V3) -> V3 {
    let mut v = v;
    let length = normalize(&mut v);
    let mut a = cross(down, v);
    normalize(&mut a);
    let mut b = cross(a, down);
    normalize(&mut b);
    scale(b, length)
}

#[cfg(test)]
mod tests;
