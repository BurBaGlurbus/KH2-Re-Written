//! The "ppp" particle VM of the PC exe, translated: an effect instance runs
//! one PDATA program block. Each entry is a part type whose programs
//! (keyframed adds, integrators, matrix builders, emitters and draws) act
//! on a per-part work block at the byte offsets the data names.
//!
//! Stepping follows FUN_1402c28e0 / FUN_1404b0fd0 / FUN_1404b09e0: an
//! update pass every game frame, programs gated to even part frames, and a
//! draw pass that advances part frames and instance time by 0x1000 every
//! second game frame (one VM tick = 2.0 frame units, 30 Hz at 60 fps).
mod extra;
pub mod math;
mod mega_birth;
mod mega_model;
mod models;
pub mod prize;
mod programs;
mod ribbon;
mod shipoly;
mod tail;
mod tail2;
mod thunder;

use data::object_effect::{Dpd, Program, i32_at, u32_at};
use math::{IDENTITY, Mat, Vec4};
use std::sync::Arc;

/// The exe's default view→screen scale (`ede0` = the camera's
/// view-screen matrix, FUN_14019dc50 / FUN_14019d050): scrz =
/// (512 / 0.8) · 0.5 / tan(0.75) (DAT_1405b1390, DAT_140585c7c,
/// DAT_140623a68), times ax = 0.8 and ay = 0.8667 (DAT_1405b1380). The
/// camera may change scrz at run time.
pub const DEFAULT_SCREEN_SCALE: [f32; 2] = [274.797_1, 297.696_9];

/// Frame value parts saturate at (FUN_1404b09e0).
const FRAME_CAP: u32 = 0x6fff_f000;
const NO_LOOP: i32 = 0x7000_0000;

/// The CRT `rand()` the exe draws from (MSVC LCG). The game's own seed
/// and call history are not reproducible, so sequences differ in detail.
#[derive(Debug, Clone)]
pub struct Rng(pub u32);

impl Default for Rng {
    fn default() -> Self {
        Rng(1)
    }
}

impl Rng {
    pub fn rand(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7fff
    }

    /// FUN_1404dc710: three rand() low bytes as a 24-bit fraction.
    pub fn unit(&mut self) -> f32 {
        let a = self.rand() & 0xff;
        let b = self.rand() & 0xff;
        let c = self.rand() & 0xff;
        ((a << 16) | (b << 8) | c) as f32 * (1. / 16_777_216.)
    }
}

/// One drawn sprite quad. Corners are in the draw space of the effect
/// (view space: x right, y down, z forward, after the identity "ede0").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub corners: [[f32; 3]; 4],
    /// Texel coordinates (u = S/16, v = T/8 in the effect atlas).
    pub uv: [[f32; 2]; 4],
    /// Sprite colour Ã— part colour, 1.0 = PS2 0x80.
    pub colour: [f32; 4],
    /// GS ALPHA register byte: 0x48 additive, 0x44 alpha blend.
    pub blend: u8,
    /// Per-corner colours (same scale as `colour`) when the quad fades
    /// along itself, as the swing trail does.
    pub corner_colours: Option<[[f32; 4]; 4]>,
    /// Texture base pointer from the frame's TEX0 (selects the atlas).
    pub tbp: u16,
    /// The frame's full TEX0 register (TBP, CBP, size, format).
    pub tex0: u64,
}

/// One OMD model draw (pppDrawMdl3 / pppDrawMdlTs3): the model of the
/// instance's DPD through draw matrix `a` (view space: position = aÂ·v).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDraw {
    /// DPD-local model id.
    pub model: i16,
    /// Morphed vertices (pppVtMime) replacing the model's own array.
    pub vertices: Option<Arc<Vec<[i16; 3]>>>,
    pub a: Mat,
    /// The "Z correct" matrix of the exe's depth output (only depth).
    pub b: Option<Mat>,
    /// Part colour bytes, 0x80 = 1.0.
    pub rgba: [u8; 4],
    /// UV scroll ints (texels Ã— 65536, as the exe's cvttss2si gives them).
    pub scroll: [i32; 2],
    /// The draw-mode word a program hands FUN_1404dc760 in place of the
    /// model's own (header +0x16): pppDrawMdlSemi3's key +8, pppDrawMdlLoopZ's
    /// key +0x20. `None` = the model's own. Its bits are not decoded.
    pub mode: Option<u16>,
}

/// One untextured gouraud triangle (pppKeTh's GS fans and strips, as
/// FUN_1404dfb20 hands them to the PC renderer). Corners are in the effect's
/// draw space like [`Quad`]; colours are the GS vertex bytes (RGB 0..255,
/// alpha 0x80 = 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub corners: [[f32; 3]; 3],
    pub colours: [[u8; 4]; 3],
    /// GS ALPHA register byte: 0x48 additive, 0x42 subtractive, 0xa8 replace.
    pub blend: u8,
}

pub(crate) struct Part {
    pub frame: u32,
    pub cur: Mat,
    pub draw: Mat,
    pub just_created: bool,
    /// Work variables from header +0xa0.
    pub vars: Vec<u8>,
    /// Current keyframe position per program (absolute in the DPD).
    pub cursors: Vec<usize>,
    /// pppRyjMegaBirth sub-particle pools by work offset u0.
    pub mega: std::collections::HashMap<usize, mega_birth::Pool>,
    /// pppRyjMegaBirthModel pools by work offset.
    pub mega_model: std::collections::HashMap<usize, mega_model::Pool>,
    /// Work +0x90 vertex handle: the array pppVtMime morphed this frame.
    pub vertex_override: Option<Arc<Vec<[i16; 3]>>>,
    /// pppEiDrawShipoly ribbon state by work offset u3.
    pub ribbons: std::collections::HashMap<usize, ribbon::Ribbon>,
    /// pppRyjDrawShipolyBone swing trails by program use-value offset.
    pub shipoly: std::collections::HashMap<usize, shipoly::Trail>,
    /// pppKeTh bolt pools by KeThRes work offset.
    pub thunder: std::collections::HashMap<usize, thunder::Pool>,
    /// Handle serial (the exe's generational handles, FUN_1404ad240).
    pub serial: u64,
    /// Work +4: the part that birthed this one, if it is still alive.
    pub parent: Option<(usize, u64)>,
    /// Work +0x9d: set when a loop range restarted the part (never
    /// cleared; programs skip their frame-0 set-up after it).
    pub restarted: bool,
    /// Work +0x9e: a loop restart with loop start 0 asks the next even
    /// update to restart the part once more (FUN_1404b0fd0/FUN_1404b09e0).
    pub restart_pending: bool,
}

impl Part {
    pub fn f32(&self, o: usize) -> f32 {
        self.vars
            .get(o..o + 4)
            .map_or(0., |w| f32::from_le_bytes(w.try_into().unwrap()))
    }
    pub fn set_f32(&mut self, o: usize, v: f32) {
        if let Some(w) = self.vars.get_mut(o..o + 4) {
            w.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub fn i32(&self, o: usize) -> i32 {
        self.f32(o).to_bits() as i32
    }
    pub fn set_i32(&mut self, o: usize, v: i32) {
        self.set_f32(o, f32::from_bits(v as u32));
    }
    pub fn i16(&self, o: usize) -> i16 {
        self.vars
            .get(o..o + 2)
            .map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
    }
    pub fn set_i16(&mut self, o: usize, v: i16) {
        if let Some(w) = self.vars.get_mut(o..o + 2) {
            w.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub fn u8(&self, o: usize) -> u8 {
        self.vars.get(o).copied().unwrap_or(0)
    }
    pub fn set_u8(&mut self, o: usize, v: u8) {
        if let Some(b) = self.vars.get_mut(o) {
            *b = v;
        }
    }
    pub fn vec4(&self, o: usize) -> Vec4 {
        std::array::from_fn(|i| self.f32(o + 4 * i))
    }
    pub fn set_vec4(&mut self, o: usize, v: Vec4) {
        for (i, x) in v.into_iter().enumerate() {
            self.set_f32(o + 4 * i, x);
        }
    }
    pub fn zero(&mut self, o: usize, n: usize) {
        if let Some(w) = self.vars.get_mut(o..o + n) {
            w.fill(0);
        }
    }
}

struct Group {
    entry: usize,
    start: i32,
    parts: Vec<usize>,
}

/// Matrices the exe keeps in globals while an instance runs.
pub(crate) struct Ctx<'a> {
    pub dpd: &'a Dpd,
    /// ec20 instance world, ec60 view, ed60 = viewÂ·world, ede0 draw space.
    pub world: Mat,
    pub view: Mat,
    pub ed60: Mat,
    pub ede0: Mat,
    /// f010..f018: lengths of the instance world axes.
    pub scales: [f32; 3],
    /// DAT_142b0ec50 / eff0: the instance position (the translation the
    /// instance was given, before the PDATA header's local transform).
    pub origin: Vec4,
    pub colour: Vec4,
    /// The current part's parent `cur` matrix (work +4 handle; +0x10).
    pub parent_cur: Option<Mat>,
    /// Instance skip flag (+0xc4): odd half of a VM tick.
    pub skip: bool,
    pub rng: &'a mut Rng,
    pub quads: &'a mut Vec<Quad>,
    pub model_draws: &'a mut Vec<ModelDraw>,
    pub triangles: &'a mut Vec<Triangle>,
    /// Screen units per view unit at depth 1 (x, y) of the exe's
    /// view→screen matrix: pppKeTh widths are screen offsets.
    pub screen: [f32; 2],
    /// Work variables of each entry's first part (pppKeTh targets), filled
    /// before a pppKeTh update.
    pub heads: Vec<Vec<u8>>,
    pub births: Vec<programs::Birth>,
    pub unsupported: &'a mut Vec<u32>,
    /// `DAT_142b0efe4`: set while a loop restart catches the part up to the
    /// loop start (some programs skip their effects then).
    pub catch_up: bool,
    /// Instance flag bytes set by pppRyjMngFlag: draw layer (+0xc0),
    /// pause (+0xc1), camera offset (+0xc2).
    pub flags: &'a mut [u8; 3],
    /// The trail's two world points of this frame (see [`Instance::trail`]).
    pub trail: Option<[[f32; 3]; 2]>,
}

/// A running effect.
pub struct Instance {
    dpd: Arc<Dpd>,
    pdata: usize,
    groups: Vec<Group>,
    parts: Vec<Option<Part>>,
    time: i32,
    end: i32,
    looping: bool,
    wait: f32,
    acc: f32,
    pub world: Mat,
    pub colour: Vec4,
    finished: bool,
    /// Program types met that this translation does not run.
    pub unsupported: Vec<u32>,
    /// Model draws of the last [`Instance::frame`].
    pub model_draws: Vec<ModelDraw>,
    /// Untextured gouraud triangles of the last [`Instance::frame`].
    pub triangles: Vec<Triangle>,
    /// Screen units per view unit at depth 1 (x, y) for screen-space
    /// widths ([`DEFAULT_SCREEN_SCALE`] unless the camera says otherwise).
    pub screen_scale: [f32; 2],
    /// The two world-space points a swing trail (pppRyjDrawShipolyBone)
    /// follows this frame: the element's BonePos pair on its bone. The
    /// host sets it before [`Instance::frame`].
    pub trail: Option<[[f32; 3]; 2]>,
    /// pppRyjMngFlag bytes: draw layer, pause, camera offset.
    flags: [u8; 3],
    next_serial: u64,
    /// SeqEffect EndType 2: a looping effect ends when its parts are gone.
    released: bool,
    /// SeqEffect EndType 3: remaining alpha and its per-frame step.
    fade: Option<(f32, f32)>,
}

impl Instance {
    /// Starts PDATA `pdata` of `dpd` with world matrix `world` (FUN_1404b1600
    /// and FUN_1404b1280 composed by the caller) after `wait` frames.
    pub fn new(dpd: Arc<Dpd>, pdata: usize, world: Mat, wait: f32) -> Option<Self> {
        let data = dpd.pdata.get(pdata)?;
        let groups = data
            .entries
            .iter()
            .enumerate()
            .map(|(entry, e)| Group {
                entry,
                start: e.start_frame,
                parts: Vec::new(),
            })
            .collect();
        let (end, looping) = (data.end_time, data.looping);
        Some(Instance {
            dpd,
            pdata,
            groups,
            parts: Vec::new(),
            time: 0,
            end,
            looping,
            wait,
            acc: 0.,
            world,
            colour: [1.; 4],
            finished: false,
            unsupported: Vec::new(),
            model_draws: Vec::new(),
            triangles: Vec::new(),
            screen_scale: DEFAULT_SCREEN_SCALE,
            trail: None,
            flags: [0; 3],
            next_serial: 1,
            released: false,
            fade: None,
        })
    }

    /// The DPD this instance reads its models, shapes and textures from.
    pub fn dpd(&self) -> &Arc<Dpd> {
        &self.dpd
    }

    /// The draw layer pppRyjMngFlag set: odd values draw behind the
    /// depth-sorted effects, even values in front, 0 sorts by depth.
    pub fn layer(&self) -> u8 {
        self.flags[0]
    }

    /// The PDATA header's loop flag (a looping effect runs until stopped).
    pub fn looping(&self) -> bool {
        self.looping
    }

    /// SeqEffect EndType 2 (FUN_1402be8f0): a looping effect now ends as
    /// soon as its parts are gone.
    pub fn release(&mut self) {
        self.released = true;
    }

    /// SeqEffect EndType 3 (FUN_1402be6a0): the colour alpha falls to zero
    /// over `frames` event frames (two game frames each), then the effect
    /// ends.
    pub fn fade_out(&mut self, frames: f32) {
        if frames > 0. {
            self.fade = Some((1., 1. / (frames * 2.)));
        } else {
            self.finished = true;
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// An end was requested (`release`, FUN_1402c2830 sets +0x91).
    pub fn is_released(&self) -> bool {
        self.released
    }

    pub fn live_parts(&self) -> usize {
        self.parts.iter().flatten().count()
    }

    /// The PDATA header's own transform (offset, angle, scale), which
    /// FUN_1404b1280 applies after the element/instance transform.
    pub fn local_matrix(&self) -> Mat {
        let d = &self.dpd.pdata[self.pdata];
        let mut m = math::rot_xyz([d.angle[0], d.angle[1], d.angle[2]]);
        for (c, s) in d.scale.iter().take(3).enumerate() {
            m[c] = math::scale(m[c], *s);
        }
        m[3] = [d.offset[0], d.offset[1], d.offset[2], 1.];
        m
    }

    /// One game frame: the update pass, then the draw pass (which advances
    /// time every second frame). Returns the quads drawn this frame.
    pub fn frame(&mut self, view: &Mat, rng: &mut Rng) -> Vec<Quad> {
        let mut quads = Vec::new();
        self.model_draws.clear();
        self.triangles.clear();
        if self.finished {
            return quads;
        }
        if let Some((level, step)) = self.fade {
            let level = level - step;
            if level <= 0. {
                self.finished = true;
                self.parts.clear();
                for g in &mut self.groups {
                    g.parts.clear();
                }
                return quads;
            }
            self.fade = Some((level, step));
            self.colour[3] = level;
        }
        if self.wait > 0. {
            self.wait -= 1.;
            if self.wait > 0. {
                return quads;
            }
        }
        self.acc += 1.;
        let skip = self.acc < 2.;
        if !skip {
            self.acc -= 2.;
        }
        let world = mul_world(&self.world, &self.local_matrix());
        let dpd = self.dpd.clone();
        let scales = std::array::from_fn(|c| {
            let a = world[c];
            (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
        });
        let mut unsupported = std::mem::take(&mut self.unsupported);
        let mut model_draws = Vec::new();
        let mut triangles = Vec::new();
        let mut flags = self.flags;
        let mut ctx = Ctx {
            dpd: &dpd,
            world,
            view: *view,
            ed60: math::mul(view, &world),
            ede0: IDENTITY,
            scales,
            origin: self.world[3],
            colour: self.colour,
            parent_cur: None,
            skip,
            rng,
            quads: &mut quads,
            model_draws: &mut model_draws,
            triangles: &mut triangles,
            screen: self.screen_scale,
            heads: Vec::new(),
            births: Vec::new(),
            unsupported: &mut unsupported,
            catch_up: false,
            flags: &mut flags,
            trail: self.trail,
        };
        self.update_pass(&mut ctx);
        self.draw_pass(&mut ctx, true);
        self.unsupported = unsupported;
        self.model_draws = model_draws;
        self.triangles = triangles;
        self.flags = flags;
        // FUN_1404b08e0: time stands still while the instance is paused.
        if !skip && flags[1] == 0 {
            self.time += 0x1000;
            let drained = self.released && self.looping && self.parts.iter().all(Option::is_none);
            if (!self.looping && self.end <= self.time) || drained {
                self.finished = true;
                self.parts.clear();
                for g in &mut self.groups {
                    g.parts.clear();
                }
            }
        }
        quads
    }

    fn entry_programs(&self, group: usize) -> &[Program] {
        &self.dpd.pdata[self.pdata].entries[self.groups[group].entry].programs
    }

    /// FUN_1404b02f0 / FUN_1404aff20: a new part of `group` at frame 0,
    /// cursors on the first keyframes, create hooks run. A spawned part is
    /// inserted after the group's first part.
    fn create(&mut self, group: usize, ctx: &mut Ctx, parent: Option<usize>) -> usize {
        let entry = &ctx.dpd.pdata[self.pdata].entries[self.groups[group].entry];
        let serial = self.next_serial;
        self.next_serial += 1;
        let parent = parent.and_then(|id| {
            self.parts
                .get(id)
                .and_then(Option::as_ref)
                .map(|p| (id, p.serial))
        });
        let mut part = Part {
            frame: 0,
            cur: IDENTITY,
            draw: IDENTITY,
            just_created: true,
            vars: vec![0; entry.work_size.saturating_sub(0xa0)],
            cursors: entry.programs.iter().map(|p| p.params).collect(),
            mega: Default::default(),
            mega_model: Default::default(),
            vertex_override: None,
            ribbons: Default::default(),
            shipoly: Default::default(),
            thunder: Default::default(),
            serial,
            parent,
            restarted: false,
            restart_pending: false,
        };
        for p in &entry.programs {
            programs::create(p, &mut part, ctx);
        }
        let id = match self.parts.iter().position(Option::is_none) {
            Some(free) => {
                self.parts[free] = Some(part);
                free
            }
            None => {
                self.parts.push(Some(part));
                self.parts.len() - 1
            }
        };
        let list = &mut self.groups[group].parts;
        let at = list.len().min(1);
        list.insert(at, id);
        id
    }

    fn spawn_groups(&mut self, ctx: &mut Ctx) {
        for g in 0..self.groups.len() {
            if self.groups[g].start <= self.time {
                self.groups[g].start = i32::MAX;
                let id = self.create(g, ctx, None);
                if let Some(p) = self.parts[id].as_mut() {
                    p.just_created = false;
                }
            }
        }
    }

    /// Births queued by the program that just ran on part `emitter`: the
    /// children keep the emitter as their parent handle (work +4).
    fn births(&mut self, ctx: &mut Ctx, emitter: usize) {
        for b in std::mem::take(&mut ctx.births) {
            if b.entry >= self.groups.len() {
                continue;
            }
            let id = self.create(b.entry, ctx, Some(emitter));
            if let Some(p) = self.parts[id].as_mut() {
                for (i, v) in b.position.iter().take(b.position_len).enumerate() {
                    p.set_f32(b.position_var + 4 * i, *v);
                }
                if let Some(var) = b.velocity_var {
                    p.set_vec4(var, b.velocity);
                }
                for (offset, value) in b.ints {
                    p.set_i32(offset, value);
                }
            }
        }
    }

    /// Each entry's first part's work variables (empty when it has none).
    fn heads(&self) -> Vec<Vec<u8>> {
        self.groups
            .iter()
            .map(|g| {
                g.parts
                    .first()
                    .and_then(|&id| self.parts[id].as_ref())
                    .map_or_else(Vec::new, |p| p.vars.clone())
            })
            .collect()
    }

    /// The `cur` matrix of part `id`'s parent while that part is alive.
    fn parent_matrix(&self, id: usize) -> Option<Mat> {
        let (pid, serial) = self.parts.get(id)?.as_ref()?.parent?;
        self.parts
            .get(pid)?
            .as_ref()
            .filter(|p| p.serial == serial)
            .map(|p| p.cur)
    }

    /// Cursor advance: when the next keyframe's time word equals the part
    /// frame, it becomes current (the word may lie past the keyframes, as
    /// in the exe).
    fn advance(dpd: &Dpd, part: &mut Part, j: usize, p: &Program) {
        let next = part.cursors[j] + p.param_size;
        if u32_at(&dpd.bytes, next).is_ok_and(|t| t == part.frame) {
            part.cursors[j] = next;
        }
    }

    /// FUN_1404b0fd0: spawn due groups, then run every update-only program
    /// over each group's parts.
    fn update_pass(&mut self, ctx: &mut Ctx) {
        self.spawn_groups(ctx);
        for g in 0..self.groups.len() {
            let programs: Vec<Program> = self.entry_programs(g).to_vec();
            for (j, p) in programs.iter().enumerate() {
                if !programs::has_update(p.kind) || programs::has_draw(p.kind) {
                    continue;
                }
                for id in self.groups[g].parts.clone() {
                    let parent_cur = self.parent_matrix(id);
                    let Some(part) = self.parts[id].as_mut() else {
                        continue;
                    };
                    if part.just_created {
                        continue;
                    }
                    if ctx.flags[1] == 0 && part.frame & 1 == 0 && part.restart_pending {
                        Self::restart(part, &programs, ctx);
                    }
                    Self::advance(ctx.dpd, part, j, p);
                    let cursor = part.cursors[j];
                    ctx.parent_cur = parent_cur;
                    if ctx.flags[1] == 0 || p.kind == programs::MNG_FLAG {
                        programs::update(p, cursor, part, ctx);
                    }
                    self.births(ctx, id);
                }
            }
        }
    }

    /// FUN_1404b09e0: programs with a draw function (updating first when
    /// they have one), then the frame advance and lifetime check.
    fn draw_pass(&mut self, ctx: &mut Ctx, draw: bool) {
        for g in 0..self.groups.len() {
            if self.groups[g].parts.is_empty() {
                continue;
            }
            let programs: Vec<Program> = self.entry_programs(g).to_vec();
            for (j, p) in programs.iter().enumerate() {
                if !programs::has_draw(p.kind) {
                    continue;
                }
                for id in self.groups[g].parts.clone() {
                    let parent_cur = self.parent_matrix(id);
                    if p.kind == thunder::KIND {
                        ctx.heads = self.heads();
                    }
                    let Some(part) = self.parts[id].as_mut() else {
                        continue;
                    };
                    if part.just_created {
                        continue;
                    }
                    let update = programs::has_update(p.kind);
                    if update && ctx.flags[1] == 0 && part.frame & 1 == 0 && part.restart_pending {
                        Self::restart(part, &programs, ctx);
                    }
                    Self::advance(ctx.dpd, part, j, p);
                    let cursor = part.cursors[j];
                    ctx.parent_cur = parent_cur;
                    if update && ctx.flags[1] == 0 {
                        programs::update(p, cursor, part, ctx);
                    }
                    if draw {
                        programs::draw(p, cursor, part, ctx);
                    }
                }
            }
            let paused = ctx.flags[1] != 0;
            for id in self.groups[g].parts.clone() {
                let Some(part) = self.parts[id].as_mut() else {
                    continue;
                };
                if !ctx.skip && !paused {
                    if part.just_created {
                        part.just_created = false;
                    } else {
                        part.frame = next_frame(part.frame);
                    }
                } else if ctx.skip && !paused && !part.just_created {
                    part.frame |= 1;
                }
            }
        }
        if ctx.skip || ctx.flags[1] != 0 {
            return;
        }
        self.lifetime(ctx);
    }

    /// The tail of FUN_1404b09e0: a part at its entry's loop end restarts
    /// (and is caught up to the loop start); any other part at or past the
    /// entry's total frame is destroyed. A released looping instance whose
    /// parts all live forever loses them at once.
    fn lifetime(&mut self, ctx: &mut Ctx) {
        let mut totals = 0u32;
        for g in 0..self.groups.len() {
            let entry = &ctx.dpd.pdata[self.pdata].entries[self.groups[g].entry];
            let total = entry.total_frame as u32;
            let (loop_start, loop_end) = (entry.loop_start, entry.loop_end);
            let looped = loop_start < NO_LOOP && loop_end < NO_LOOP;
            let programs = entry.programs.clone();
            let mut keep = Vec::new();
            for id in self.groups[g].parts.clone() {
                let Some(part) = self.parts[id].as_mut() else {
                    continue;
                };
                if !self.released && looped && loop_end <= part.frame as i32 {
                    Self::restart(part, &programs, ctx);
                    ctx.catch_up = true;
                    if loop_start == 0 {
                        // FUN_1404b0550: one update at frame 0, then the
                        // next even update restarts the part again.
                        self.catch_up_tick(id, &programs, ctx);
                        ctx.catch_up = false;
                        if let Some(part) = self.parts[id].as_mut() {
                            part.restart_pending = true;
                        }
                    } else {
                        while self.parts[id]
                            .as_ref()
                            .is_some_and(|p| (p.frame as i32) < loop_start)
                        {
                            self.catch_up_tick(id, &programs, ctx);
                        }
                        ctx.catch_up = false;
                    }
                    keep.push(id);
                    continue;
                }
                totals |= total;
                if total <= part.frame && !part.just_created {
                    if let Some(mut part) = self.parts[id].take() {
                        for p in &programs {
                            programs::destroy(p, &mut part, ctx);
                        }
                    }
                } else {
                    keep.push(id);
                }
            }
            self.groups[g].parts = keep;
        }
        if self.looping && self.released && totals == NO_LOOP as u32 {
            for g in 0..self.groups.len() {
                let entry = &ctx.dpd.pdata[self.pdata].entries[self.groups[g].entry];
                let programs = entry.programs.clone();
                for id in std::mem::take(&mut self.groups[g].parts) {
                    if let Some(mut part) = self.parts[id].take() {
                        for p in &programs {
                            programs::destroy(p, &mut part, ctx);
                        }
                    }
                }
            }
        }
    }

    /// FUN_1404b0120: frame 0, cursors back on the first keyframes, the
    /// programs' restart hooks (table +0x40) run.
    fn restart(part: &mut Part, programs: &[Program], ctx: &mut Ctx) {
        part.frame = 0;
        part.restarted = true;
        part.restart_pending = false;
        for (j, p) in programs.iter().enumerate() {
            part.cursors[j] = p.params;
            programs::restart(p, part, ctx);
        }
    }

    /// FUN_1404b0550 (and the catch-up loop of FUN_1404b09e0): every
    /// program's update once (except those listed at PTR_FUN_140779990),
    /// then the frame advance.
    fn catch_up_tick(&mut self, id: usize, programs: &[Program], ctx: &mut Ctx) {
        let parent_cur = self.parent_matrix(id);
        for (j, p) in programs.iter().enumerate() {
            let Some(part) = self.parts[id].as_mut() else {
                return;
            };
            Self::advance(ctx.dpd, part, j, p);
            if programs::has_update(p.kind) && !programs::skips_catch_up(p.kind) {
                if p.kind == thunder::KIND {
                    let heads = self.heads();
                    ctx.heads = heads;
                }
                let Some(part) = self.parts[id].as_mut() else {
                    return;
                };
                let cursor = part.cursors[j];
                ctx.parent_cur = parent_cur;
                programs::update(p, cursor, part, ctx);
                self.births(ctx, id);
            }
        }
        if let Some(part) = self.parts[id].as_mut() {
            part.frame = next_frame(part.frame);
        }
    }
}

/// A part frame one tick on, saturating at [`FRAME_CAP`].
fn next_frame(frame: u32) -> u32 {
    ((frame + 0x1000) & 0xffff_f000).min(FRAME_CAP)
}

/// ec20 composed with the PDATA local transform (FUN_1404b1280 order).
fn mul_world(world: &Mat, local: &Mat) -> Mat {
    math::mul(world, local)
}

/// The instance world matrix for translation `t`, radians `r` and scale
/// `s` (FUN_1404b1280: identity â†’ RotX â†’ RotY â†’ RotZ, axes scaled).
pub fn instance_matrix(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Mat {
    let mut m = math::rot_radians(&IDENTITY, r);
    for c in 0..3 {
        m[c] = math::scale(m[c], s[c]);
    }
    m[3] = [t[0], t[1], t[2], 1.];
    m
}

pub(crate) fn key_i32(dpd: &Dpd, at: usize) -> i32 {
    i32_at(&dpd.bytes, at).unwrap_or(0)
}

#[cfg(test)]
mod tests;
