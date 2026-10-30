//! A compiled pixel shader (DXBC, shader model 5) to a WGSL function, one instruction for one statement, at run time
//! from the player's own install. Registers are `vec4<u32>`, as the instruction set's are typeless: an instruction
//! reads its operands as what it works on, so comparison masks and integers held in registers stay what they are.
//!
//! The harness (`kh3_material.wgsl`) gives the inputs by their D3D semantics (`KhIn`) and the constant buffers by name.
use super::{Result, bad};
use std::collections::BTreeMap;
use std::fmt::Write;

/// What a translation needs to know of the material.
#[derive(Clone, Debug, Default)]
pub struct Bindings {
    /// Uniform buffer names by constant buffer slot.
    pub buffers: Vec<String>,
    /// Texture register to the material's texture slot.
    pub textures: BTreeMap<u32, u32>,
    /// Sampler register to true when it clamps.
    pub clamps: BTreeMap<u32, bool>,
    /// Discards do nothing: the harness takes the mask from the depth-only shader.
    pub no_discard: bool,
}

/// The translated function's body and what it read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Translated {
    pub body: String,
    /// The highest row of the Material buffer read, plus one.
    pub material_rows: usize,
    pub textures: Vec<u32>,
    pub discards: bool,
}

fn word(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A chunk of the container by its tag.
fn chunk<'a>(container: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = word(container, 28)? as usize;
    for k in 0..count {
        let at = word(container, 32 + k * 4)? as usize;
        if container.get(at..at + 4)? == tag {
            let size = word(container, at + 4)? as usize;
            return container.get(at + 8..at + 8 + size);
        }
    }
    None
}

/// Input signature: per register, per component, the value it holds.
fn inputs(container: &[u8]) -> Result<BTreeMap<u32, [Option<(String, u8)>; 4]>> {
    let sig =
        chunk(container, b"ISGN").ok_or_else(|| bad("a shader without an input signature"))?;
    let n = word(sig, 0).ok_or_else(|| bad("a bad signature"))? as usize;
    let first = word(sig, 4).ok_or_else(|| bad("a bad signature"))? as usize;
    let mut out: BTreeMap<u32, [Option<(String, u8)>; 4]> = BTreeMap::new();
    for k in 0..n {
        let e = first + k * 24;
        let (Some(name_at), Some(index), Some(reg), Some(&mask)) = (
            word(sig, e),
            word(sig, e + 4),
            word(sig, e + 16),
            sig.get(e + 20),
        ) else {
            return Err(bad("a bad signature"));
        };
        let name: String = sig[name_at as usize..]
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as char)
            .collect();
        let field = match (name.as_str(), index) {
            ("TEXCOORD", n) => format!("tc{n}"),
            ("COLOR", 0) => "col0".to_owned(),
            ("SV_Position", _) => "svpos".to_owned(),
            ("SV_IsFrontFace", _) => "front".to_owned(),
            _ => "zero".to_owned(),
        };
        let start = (0..4).find(|c| mask & (1 << c) != 0).unwrap_or(0);
        let slot = out.entry(reg).or_default();
        for c in 0..4 {
            if mask & (1 << c) != 0 {
                slot[c] = Some((field.clone(), (c - start) as u8));
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Operand {
    kind: u32,
    /// Swizzle of a source or the mask's components of a destination.
    comps: Vec<u8>,
    /// Immediate index parts.
    index: Vec<u32>,
    /// A relative index: (operand, added immediate).
    relative: Option<(Box<Operand>, u32)>,
    neg: bool,
    abs: bool,
    imm: [u32; 4],
}

const TEMP: u32 = 0;
const INPUT: u32 = 1;
const OUTPUT: u32 = 2;
const INDEXABLE: u32 = 3;
const IMM32: u32 = 4;
const CONSTANT_BUFFER: u32 = 8;
const IMM_BUFFER: u32 = 9;
const NULL: u32 = 13;

struct Tokens<'a> {
    t: &'a [u8],
    at: usize,
}

impl Tokens<'_> {
    fn next(&mut self) -> Result<u32> {
        let v = word(self.t, self.at * 4).ok_or_else(|| bad("a shader ends early"))?;
        self.at += 1;
        Ok(v)
    }

    fn operand(&mut self) -> Result<Operand> {
        let t = self.next()?;
        let mut o = Operand {
            kind: (t >> 12) & 0xff,
            ..Operand::default()
        };
        let comp_type = t & 3;
        let mode = (t >> 2) & 3;
        if comp_type == 2 {
            o.comps = match mode {
                0 => (0..4)
                    .filter(|c| t >> (4 + c) & 1 != 0)
                    .map(|c| c as u8)
                    .collect(),
                1 => (0..4).map(|c| (t >> (4 + c * 2) & 3) as u8).collect(),
                _ => vec![(t >> 4 & 3) as u8],
            };
        } else if comp_type == 1 {
            o.comps = vec![0];
        }
        let dims = (t >> 20) & 3;
        let mut ext = t >> 31 != 0;
        while ext {
            let e = self.next()?;
            ext = e >> 31 != 0;
            if e & 0x3f == 1 {
                match (e >> 6) & 0xff {
                    1 => o.neg = true,
                    2 => o.abs = true,
                    3 => {
                        o.neg = true;
                        o.abs = true;
                    }
                    _ => {}
                }
            }
        }
        if o.kind == IMM32 {
            let n = if comp_type == 2 { 4 } else { 1 };
            for k in 0..n {
                o.imm[k] = self.next()?;
            }
            // An immediate has no swizzle: its four numbers go by the destination's components.
            o.comps = vec![0, 1, 2, 3];
            if n == 1 {
                o.imm = [o.imm[0]; 4];
            }
            return Ok(o);
        }
        for d in 0..dims {
            let rep = (t >> (22 + d * 3)) & 7;
            match rep {
                0 => {
                    let v = self.next()?;
                    o.index.push(v);
                }
                2 => {
                    o.index.push(0);
                    o.relative = Some((Box::new(self.operand()?), 0));
                }
                3 => {
                    let v = self.next()?;
                    o.index.push(v);
                    o.relative = Some((Box::new(self.operand()?), v));
                }
                _ => return Err(bad("an index form this does not read")),
            }
        }
        Ok(o)
    }
}

struct Gen<'a> {
    bindings: &'a Bindings,
    inputs: BTreeMap<u32, [Option<(String, u8)>; 4]>,
    out: String,
    material_rows: usize,
    textures: Vec<u32>,
    discards: bool,
}

/// The View, Primitive and lighting rows the harness gives.
const VIEW_ROWS: &[u32] = &[
    0, 1, 2, 3, 8, 9, 10, 12, 13, 14, 26, 28, 36, 37, 38, 39, 53, 54, 55, 58, 117, 118, 119, 122,
    123, 124, 125, 126, 127, 128, 129, 130, 137, 141, 142, 151, 160, 161, 172,
];
const PRIMITIVE_ROWS: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18];
const LIGHTING_ROWS: &[u32] = &[4, 5, 8, 9, 10, 11, 12, 13, 14, 15];

const XYZW: [char; 4] = ['x', 'y', 'z', 'w'];

impl Gen<'_> {
    /// The operand's component `c` (of the instruction's order) as (expression, it is a float).
    fn base(&mut self, o: &Operand, c: usize) -> Result<(String, bool)> {
        let comp = *o.comps.get(c).or(o.comps.last()).unwrap_or(&0) as usize;
        let s = XYZW[comp];
        Ok(match o.kind {
            TEMP => (format!("r{}.{s}", o.index[0]), false),
            OUTPUT => (format!("o{}.{s}", o.index[0]), false),
            IMM32 => (format!("{}u", o.imm[comp.min(3)]), false),
            INPUT => {
                let held = self.inputs.get(&o.index[0]).and_then(|r| r[comp].clone());
                match held {
                    Some((f, k)) if f != "zero" => (format!("i.{f}.{}", XYZW[k as usize]), true),
                    _ => ("0.0".to_owned(), true),
                }
            }
            INDEXABLE => {
                let at = match &o.relative {
                    Some((r, add)) => {
                        let (e, _) = self.base(r, 0)?;
                        format!("min({e} + {add}u, X{}_LEN - 1u)", o.index[0])
                    }
                    None => format!("{}u", o.index[1]),
                };
                (format!("x{}[{at}].{s}", o.index[0]), false)
            }
            IMM_BUFFER => {
                let at = match &o.relative {
                    Some((r, add)) => {
                        let (e, _) = self.base(r, 0)?;
                        format!("min({e} + {add}u, ICB_LEN - 1u)")
                    }
                    None => format!("{}u", o.index.first().copied().unwrap_or(0)),
                };
                (format!("icb[{at}].{s}"), false)
            }
            CONSTANT_BUFFER => {
                if o.relative.is_some() {
                    return Err(bad("a constant buffer indexed by a register"));
                }
                let (slot, row) = (o.index[0], o.index[1]);
                let name = self
                    .bindings
                    .buffers
                    .get(slot as usize)
                    .map_or("", String::as_str);
                let f = if name == "Material" {
                    self.material_rows = self.material_rows.max(row as usize + 1);
                    format!("kh.m[{row}].{s}")
                } else if let Some(n) = name.strip_prefix("MaterialCollection") {
                    format!("kh.c{n}[{row}].{s}")
                } else if name == "View" && VIEW_ROWS.contains(&row) {
                    format!("i.v{row}.{s}")
                } else if name == "Primitive" && PRIMITIVE_ROWS.contains(&row) {
                    format!("i.p{row}.{s}")
                } else if name == "PrecomputedLightingBuffer" && LIGHTING_ROWS.contains(&row) {
                    format!("i.l{row}.{s}")
                } else {
                    "0.0".to_owned()
                };
                (f, true)
            }
            k => return Err(bad(format!("an operand of type {k}"))),
        })
    }

    /// As a float.
    fn f(&mut self, o: &Operand, c: usize) -> Result<String> {
        let (e, float) = self.base(o, c)?;
        let mut v = if float {
            e
        } else {
            format!("bitcast<f32>({e})")
        };
        if o.abs {
            v = format!("abs({v})");
        }
        if o.neg {
            v = format!("(-{v})");
        }
        Ok(v)
    }

    /// As bits.
    fn u(&mut self, o: &Operand, c: usize) -> Result<String> {
        let (e, float) = self.base(o, c)?;
        let v = if float {
            format!("bitcast<u32>({e})")
        } else {
            e
        };
        Ok(if o.neg { format!("(0u - {v})") } else { v })
    }

    fn i(&mut self, o: &Operand, c: usize) -> Result<String> {
        Ok(format!("bitcast<i32>({})", self.u(o, c)?))
    }

    fn line(&mut self, s: &str) {
        self.out.push_str("    ");
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// `dst.c = expr(comp index in the mask)`, each source read before any write.
    fn each(
        &mut self,
        d: &Operand,
        sat: bool,
        float: bool,
        mut expr: impl FnMut(&mut Self, usize) -> Result<String>,
    ) -> Result<()> {
        if d.kind == NULL {
            return Ok(());
        }
        let mut block = String::from("{ ");
        let mut writes = String::new();
        for &c in &d.comps {
            let mut e = expr(self, c as usize)?;
            if sat {
                e = format!("saturate({e})");
            }
            if float {
                e = format!("bitcast<u32>({e})");
            }
            let _ = write!(block, "let t{c} = {e}; ");
            let _ = write!(writes, "{} = t{c}; ", self.place(d, c as usize)?);
        }
        block.push_str(&writes);
        block.push('}');
        self.line(&block);
        Ok(())
    }

    fn place(&mut self, d: &Operand, c: usize) -> Result<String> {
        let s = XYZW[c];
        Ok(match d.kind {
            TEMP => format!("r{}.{s}", d.index[0]),
            OUTPUT => format!("o{}.{s}", d.index[0]),
            INDEXABLE => {
                let at = match &d.relative {
                    Some((r, add)) => {
                        let (e, _) = self.base(r, 0)?;
                        format!("min({e} + {add}u, X{}_LEN - 1u)", d.index[0])
                    }
                    None => format!("{}u", d.index[1]),
                };
                format!("x{}[{at}].{s}", d.index[0])
            }
            k => return Err(bad(format!("a destination of type {k}"))),
        })
    }

    fn sample(&mut self, code: u32, a: &[Operand], sat: bool) -> Result<()> {
        let (d, addr, res, samp) = (&a[0], &a[1], &a[2], &a[3]);
        let uv = format!("vec2<f32>({}, {})", self.f(addr, 0)?, self.f(addr, 1)?);
        let slot = self.bindings.textures.get(&res.index[0]).copied();
        let value = match slot {
            None => "vec4<f32>(0.0)".to_owned(),
            Some(slot) => {
                if !self.textures.contains(&slot) {
                    self.textures.push(slot);
                }
                let clamp = self
                    .bindings
                    .clamps
                    .get(&samp.index[0])
                    .copied()
                    .unwrap_or(false);
                let s = if clamp { "kh_sclamp" } else { "kh_swrap" };
                let t = format!("kh_t{slot}");
                match code {
                    69 => format!("textureSample({t}, {s}, {uv})"),
                    72 => format!("textureSampleLevel({t}, {s}, {uv}, {})", self.f(&a[4], 0)?),
                    74 => format!("textureSampleBias({t}, {s}, {uv}, {})", self.f(&a[4], 0)?),
                    73 => format!(
                        "textureSampleGrad({t}, {s}, {uv}, vec2<f32>({}, {}), vec2<f32>({}, {}))",
                        self.f(&a[4], 0)?,
                        self.f(&a[4], 1)?,
                        self.f(&a[5], 0)?,
                        self.f(&a[5], 1)?
                    ),
                    _ => return Err(bad("a sampling form")),
                }
            }
        };
        let mut s = format!("{{ let s = {value}; ");
        for &c in &d.comps {
            let comp = res.comps.get(c as usize).copied().unwrap_or(c) as usize;
            let mut e = format!("s.{}", XYZW[comp]);
            if sat {
                e = format!("saturate({e})");
            }
            let _ = write!(s, "{} = bitcast<u32>({e}); ", self.place(d, c as usize)?);
        }
        s.push('}');
        self.line(&s);
        Ok(())
    }
}

fn operand_count(code: u32) -> Option<usize> {
    Some(match code {
        2 | 7 | 18 | 21 | 22 | 48 | 58 | 62 => 0,
        3 | 8 | 13 | 31 | 63 => 1,
        11
        | 12
        | 25
        | 26
        | 27
        | 28
        | 40
        | 43
        | 47
        | 54
        | 59
        | 64
        | 65
        | 66
        | 67
        | 68
        | 75
        | 86
        | 122..=125
        | 129
        | 134..=137
        | 141 => 2,
        0 | 1 | 14 | 15 | 16 | 17 | 24 | 29 | 30 | 32 | 33 | 34 | 36 | 37 | 39 | 41 | 42 | 49
        | 51 | 52 | 56 | 57 | 60 | 79 | 80 | 83 | 84 | 85 | 87 => 3,
        35 | 50 | 55 | 77 | 78 | 138 | 139 => 4,
        38 | 81 | 82 => 4,
        140 => 5,
        45 | 61 => 3,
        69 => 4,
        72 | 74 => 5,
        73 => 6,
        _ => return None,
    })
}

/// Translates a container's pixel shader.
pub fn translate(container: &[u8], bindings: &Bindings) -> Result<Translated> {
    let shader = chunk(container, b"SHEX")
        .or_else(|| chunk(container, b"SHDR"))
        .ok_or_else(|| bad("a shader without code"))?;
    let mut g = Gen {
        bindings,
        inputs: inputs(container)?,
        out: String::new(),
        material_rows: 0,
        textures: Vec::new(),
        discards: false,
    };
    let length = word(shader, 4).ok_or_else(|| bad("a bad shader"))? as usize;
    let mut temps = 0;
    let mut head = String::new();
    let mut at = 2;
    while at < length {
        let mut t = Tokens { t: shader, at };
        let token = t.next()?;
        let code = token & 0x7ff;
        // Custom data: its length is the next token.
        if code == 53 {
            let n = t.next()? as usize;
            if (token >> 11) & 0x1f == 3 && n >= 2 {
                // The immediate constant buffer.
                let rows = (n - 2) / 4;
                let _ = write!(
                    head,
                    "    const ICB_LEN = {rows}u;\n    var icb = array<vec4<u32>, {rows}>("
                );
                for _ in 0..rows {
                    let (a, b, c, d) = (t.next()?, t.next()?, t.next()?, t.next()?);
                    let _ = write!(head, "vec4<u32>({a}u, {b}u, {c}u, {d}u), ");
                }
                head.push_str(");\n");
            }
            at += n;
            continue;
        }
        let len = ((token >> 24) & 0x7f) as usize;
        if len == 0 {
            return Err(bad("a zero-length instruction"));
        }
        let next = at + len;
        let mut ext = token >> 31 != 0;
        while ext {
            ext = t.next()? >> 31 != 0;
        }
        let sat = (token >> 13) & 1 != 0;
        // Nonzero test.
        let nz = (token >> 18) & 1 != 0;
        match code {
            88..=107 | 113..=120 | 143..=163 => {
                // Declarations: only what shapes the function.
                if code == 104 {
                    temps = t.next()?;
                } else if code == 105 {
                    let (reg, rows, _) = (t.next()?, t.next()?, t.next()?);
                    let _ = writeln!(
                        head,
                        "    const X{reg}_LEN = {rows}u;\n    var x{reg}: array<vec4<u32>, {rows}>;"
                    );
                } else if code == 101 {
                    t.operand()?;
                }
            }
            _ => {
                let n = operand_count(code)
                    .ok_or_else(|| bad(format!("an instruction with opcode {code}")))?;
                let mut a = Vec::with_capacity(n);
                for _ in 0..n {
                    a.push(t.operand()?);
                }
                instruction(&mut g, code, &a, sat, nz)?;
            }
        }
        at = next;
    }
    let mut body = head;
    for r in 0..temps {
        let _ = writeln!(body, "    var r{r}: vec4<u32>;");
    }
    for r in 0..7 {
        let _ = writeln!(body, "    var o{r}: vec4<u32>;");
    }
    body.push_str(&g.out);
    body.push_str(RETURN);
    body.push('\n');
    let mut textures = g.textures;
    textures.sort_unstable();
    Ok(Translated {
        body,
        material_rows: g.material_rows,
        textures,
        discards: g.discards,
    })
}

const RETURN: &str = "    return KhOut(bitcast<vec4<f32>>(o0), bitcast<vec4<f32>>(o1), bitcast<vec4<f32>>(o2), \
                      bitcast<vec4<f32>>(o3), bitcast<vec4<f32>>(o4), bitcast<vec4<f32>>(o5), bitcast<vec4<f32>>(o6));";

fn instruction(g: &mut Gen, code: u32, a: &[Operand], sat: bool, nz: bool) -> Result<()> {
    let test = |g: &mut Gen, o: &Operand| -> Result<String> {
        let v = g.u(o, 0)?;
        Ok(format!("{v} {} 0u", if nz { "!=" } else { "==" }))
    };
    macro_rules! float1 {
        ($f:expr) => {
            g.each(&a[0], sat, true, |g, c| Ok(format!($f, g.f(&a[1], c)?)))?
        };
    }
    macro_rules! float2 {
        ($f:expr) => {
            g.each(&a[0], sat, true, |g, c| {
                let (x, y) = (g.f(&a[1], c)?, g.f(&a[2], c)?);
                Ok(format!($f, x, y))
            })?
        };
    }
    macro_rules! compare {
        ($op:expr, $read:ident) => {
            g.each(&a[0], false, false, |g, c| {
                let (x, y) = (g.$read(&a[1], c)?, g.$read(&a[2], c)?);
                Ok(format!("select(0u, 0xffffffffu, {x} {} {y})", $op))
            })?
        };
    }
    macro_rules! bits2 {
        ($f:expr, $read:ident) => {
            g.each(&a[0], false, false, |g, c| {
                let (x, y) = (g.$read(&a[1], c)?, g.$read(&a[2], c)?);
                Ok(format!($f, x, y))
            })?
        };
    }
    match code {
        58 => {}
        62 => g.line(RETURN.trim_start()),
        63 => {
            let t = test(g, &a[0])?;
            g.line(&format!("if ({t}) {{ {} }}", RETURN.trim_start()));
        }
        2 => g.line("break;"),
        7 => g.line("continue;"),
        3 => {
            let t = test(g, &a[0])?;
            g.line(&format!("if ({t}) {{ break; }}"));
        }
        8 => {
            let t = test(g, &a[0])?;
            g.line(&format!("if ({t}) {{ continue; }}"));
        }
        13 => {
            g.discards = true;
            if !g.bindings.no_discard {
                let t = test(g, &a[0])?;
                g.line(&format!("if ({t}) {{ discard; }}"));
            }
        }
        31 => {
            let t = test(g, &a[0])?;
            g.line(&format!("if ({t}) {{"));
        }
        18 => g.line("} else {"),
        21 | 22 => g.line("}"),
        48 => g.line("loop {"),
        54 => g.each(&a[0], false, false, |g, c| {
            let v = g.u(&a[1], c)?;
            // A modifier on a move is a float's.
            if a[1].abs {
                Ok(format!("bitcast<u32>({})", g.f(&a[1], c)?))
            } else if a[1].neg {
                Ok(format!("bitcast<u32>({})", g.f(&a[1], c)?))
            } else if sat {
                Ok(format!("bitcast<u32>(saturate(bitcast<f32>({v})))"))
            } else {
                Ok(v)
            }
        })?,
        55 => g.each(&a[0], false, false, |g, c| {
            let (m, x, y) = (g.u(&a[1], c)?, g.u(&a[2], c)?, g.u(&a[3], c)?);
            let e = format!("select({y}, {x}, {m} != 0u)");
            Ok(if sat {
                format!("bitcast<u32>(saturate(bitcast<f32>({e})))")
            } else {
                e
            })
        })?,
        0 => float2!("({} + {})"),
        14 => float2!("kh_div({}, {})"),
        51 => float2!("min({}, {})"),
        52 => float2!("max({}, {})"),
        56 => float2!("({} * {})"),
        50 => g.each(&a[0], sat, true, |g, c| {
            let (x, y, z) = (g.f(&a[1], c)?, g.f(&a[2], c)?, g.f(&a[3], c)?);
            Ok(format!("({x} * {y} + {z})"))
        })?,
        15..=17 => {
            let n = (code - 13) as usize;
            let mut sum = Vec::new();
            for k in 0..n {
                sum.push(format!("{} * {}", g.f(&a[1], k)?, g.f(&a[2], k)?));
            }
            let e = sum.join(" + ");
            g.each(&a[0], sat, true, |_, _| Ok(format!("({e})")))?;
        }
        11 => float1!("dpdx({})"),
        12 => float1!("dpdy({})"),
        122 => float1!("dpdxCoarse({})"),
        123 => float1!("dpdxFine({})"),
        124 => float1!("dpdyCoarse({})"),
        125 => float1!("dpdyFine({})"),
        25 => float1!("exp2({})"),
        26 => float1!("fract({})"),
        47 => float1!("log2({})"),
        64 => float1!("round({})"),
        65 => float1!("floor({})"),
        66 => float1!("ceil({})"),
        67 => float1!("trunc({})"),
        68 => float1!("inverseSqrt({})"),
        75 => float1!("sqrt({})"),
        129 => float1!("kh_div(1.0, {})"),
        77 => {
            if a[0].kind != NULL {
                g.each(&a[0], sat, true, |g, c| {
                    Ok(format!("sin({})", g.f(&a[2], c)?))
                })?;
            }
            if a[1].kind != NULL {
                g.each(&a[1], sat, true, |g, c| {
                    Ok(format!("cos({})", g.f(&a[2], c)?))
                })?;
            }
        }
        24 => compare!("==", f),
        29 => compare!(">=", f),
        49 => compare!("<", f),
        57 => compare!("!=", f),
        32 => compare!("==", u),
        33 => compare!(">=", i),
        34 => compare!("<", i),
        39 => compare!("!=", u),
        79 => compare!("<", u),
        80 => compare!(">=", u),
        1 => bits2!("({} & {})", u),
        60 => bits2!("({} | {})", u),
        87 => bits2!("({} ^ {})", u),
        30 => bits2!("({} + {})", u),
        36 => bits2!("bitcast<u32>(max({}, {}))", i),
        37 => bits2!("bitcast<u32>(min({}, {}))", i),
        83 => bits2!("max({}, {})", u),
        84 => bits2!("min({}, {})", u),
        41 => bits2!("({} << ({} & 31u))", u),
        85 => bits2!("({} >> ({} & 31u))", u),
        42 => g.each(&a[0], false, false, |g, c| {
            let (x, y) = (g.i(&a[1], c)?, g.u(&a[2], c)?);
            Ok(format!("bitcast<u32>({x} >> ({y} & 31u))"))
        })?,
        59 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("(~{})", g.u(&a[1], c)?))
        })?,
        40 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("(0u - {})", g.u(&a[1], c)?))
        })?,
        43 => g.each(&a[0], sat, true, |g, c| {
            Ok(format!("f32({})", g.i(&a[1], c)?))
        })?,
        86 => g.each(&a[0], sat, true, |g, c| {
            Ok(format!("f32({})", g.u(&a[1], c)?))
        })?,
        27 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("bitcast<u32>(i32({}))", g.f(&a[1], c)?))
        })?,
        28 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("u32({})", g.f(&a[1], c)?))
        })?,
        35 => g.each(&a[0], false, false, |g, c| {
            let (x, y, z) = (g.u(&a[1], c)?, g.u(&a[2], c)?, g.u(&a[3], c)?);
            Ok(format!("({x} * {y} + {z})"))
        })?,
        82 => g.each(&a[1], false, false, |g, c| {
            let (x, y, z) = (g.u(&a[2], c)?, g.u(&a[3], c)?, g.u(&a[4], c)?);
            Ok(format!("({x} * {y} + {z})"))
        })?,
        38 | 81 => g.each(&a[1], false, false, |g, c| {
            let (x, y) = (g.u(&a[2], c)?, g.u(&a[3], c)?);
            Ok(format!("({x} * {y})"))
        })?,
        78 => {
            // Quotient and remainder of the same operands.
            let (q, r) = (&a[0], &a[1]);
            let list = if q.kind != NULL { &q.comps } else { &r.comps };
            let mut s = String::from("{ ");
            let mut writes = String::new();
            for &c in list {
                let c = c as usize;
                let (x, y) = (g.u(&a[2], c)?, g.u(&a[3], c)?);
                let _ = write!(s, "let a{c} = {x}; let b{c} = {y}; ");
                if q.kind != NULL {
                    let _ = write!(writes, "{} = a{c} / b{c}; ", g.place(q, c)?);
                }
                if r.kind != NULL {
                    let _ = write!(writes, "{} = a{c} % b{c}; ", g.place(r, c)?);
                }
            }
            s.push_str(&writes);
            s.push('}');
            g.line(&s);
        }
        138 | 139 => {
            let signed = code == 139;
            g.each(&a[0], false, false, |g, c| {
                let (w, o, v) = (g.u(&a[1], c)?, g.u(&a[2], c)?, g.u(&a[3], c)?);
                Ok(if signed {
                    format!("bitcast<u32>(extractBits(bitcast<i32>({v}), {o} & 31u, {w} & 31u))")
                } else {
                    format!("extractBits({v}, {o} & 31u, {w} & 31u)")
                })
            })?;
        }
        140 => g.each(&a[0], false, false, |g, c| {
            let (w, o, ins, base) = (
                g.u(&a[1], c)?,
                g.u(&a[2], c)?,
                g.u(&a[3], c)?,
                g.u(&a[4], c)?,
            );
            Ok(format!("insertBits({base}, {ins}, {o} & 31u, {w} & 31u)"))
        })?,
        134 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("countOneBits({})", g.u(&a[1], c)?))
        })?,
        141 => g.each(&a[0], false, false, |g, c| {
            Ok(format!("reverseBits({})", g.u(&a[1], c)?))
        })?,
        69 | 72 | 73 | 74 => g.sample(code, a, sat)?,
        // A texel load or a size: nothing a character's material needs from a scene texture.
        45 | 61 => g.each(&a[0], false, false, |_, _| Ok("0u".to_owned()))?,
        other => return Err(bad(format!("an instruction with opcode {other}"))),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A container with an input signature (TEXCOORD0 in v0) and these code tokens.
    fn container(code: &[u32]) -> Vec<u8> {
        let mut sig = Vec::new();
        sig.extend_from_slice(&1u32.to_le_bytes());
        sig.extend_from_slice(&8u32.to_le_bytes());
        let name_at = 8 + 24u32;
        sig.extend_from_slice(&name_at.to_le_bytes());
        sig.extend_from_slice(&0u32.to_le_bytes());
        sig.extend_from_slice(&0u32.to_le_bytes());
        sig.extend_from_slice(&3u32.to_le_bytes());
        sig.extend_from_slice(&0u32.to_le_bytes());
        sig.extend_from_slice(&[15, 15, 0, 0]);
        sig.extend_from_slice(b"TEXCOORD\0\0\0\0");
        let mut shex = vec![0u32, (code.len() + 2) as u32];
        shex.extend_from_slice(code);
        let shex: Vec<u8> = shex.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut out = vec![0u8; 28];
        out.extend_from_slice(&2u32.to_le_bytes());
        let first = 28 + 4 + 8;
        out.extend_from_slice(&(first as u32).to_le_bytes());
        out.extend_from_slice(&((first + 8 + sig.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"ISGN");
        out.extend_from_slice(&(sig.len() as u32).to_le_bytes());
        out.extend_from_slice(&sig);
        out.extend_from_slice(b"SHEX");
        out.extend_from_slice(&(shex.len() as u32).to_le_bytes());
        out.extend_from_slice(&shex);
        out
    }

    /// A 4-component temp or output destination, mask xyzw.
    fn dest(kind: u32, reg: u32) -> [u32; 2] {
        (2 | (0xf << 4) | (kind << 12) | (1 << 20), reg).into()
    }

    /// A source with the identity swizzle.
    fn source(kind: u32, reg: u32) -> [u32; 2] {
        (
            2 | (1 << 2) | (0b11100100 << 4) | (kind << 12) | (1 << 20),
            reg,
        )
            .into()
    }

    fn op(code: u32, operands: &[[u32; 2]]) -> Vec<u32> {
        let mut out = vec![code | (((1 + operands.len() * 2) as u32) << 24)];
        for o in operands {
            out.extend_from_slice(o);
        }
        out
    }

    #[test]
    fn registers_hold_bits_and_inputs_are_named_by_semantic() {
        let mut code = vec![104 | (2 << 24), 1];
        code.extend(op(
            56,
            &[dest(OUTPUT, 0), source(INPUT, 0), source(INPUT, 0)],
        ));
        code.extend(op(49, &[dest(TEMP, 0), source(INPUT, 0), source(TEMP, 0)]));
        let t = translate(&container(&code), &Bindings::default()).unwrap();
        assert!(t.body.contains("var r0: vec4<u32>;"));
        assert!(
            t.body.contains("bitcast<u32>((i.tc0.x * i.tc0.x))"),
            "{}",
            t.body
        );
        assert!(t.body.contains("o0.x = t0;"));
        assert!(
            t.body
                .contains("select(0u, 0xffffffffu, i.tc0.y < bitcast<f32>(r0.y))")
        );
        assert!(t.body.trim_end().ends_with(";"));
    }

    #[test]
    fn an_immediate_goes_by_the_destinations_components() {
        // mov o0.xyzw, l(1, 2, 3, 4)
        let mut code = vec![54 | (8 << 24)];
        code.extend_from_slice(&dest(OUTPUT, 0));
        code.extend_from_slice(&[2 | (IMM32 << 12), 1, 2, 3, 4]);
        let t = translate(&container(&code), &Bindings::default()).unwrap();
        assert!(
            t.body
                .contains("let t0 = 1u; let t1 = 2u; let t2 = 3u; let t3 = 4u;"),
            "{}",
            t.body
        );
    }

    #[test]
    fn a_discard_can_be_left_to_the_harness() {
        let mut code = vec![13 | (3 << 24) | (1 << 18)];
        code.extend_from_slice(&source(INPUT, 0));
        let mut b = Bindings::default();
        let t = translate(&container(&code), &b).unwrap();
        assert!(t.discards && t.body.contains("discard;"));
        b.no_discard = true;
        assert!(
            !translate(&container(&code), &b)
                .unwrap()
                .body
                .contains("discard;")
        );
    }

    #[test]
    fn an_unknown_instruction_refuses() {
        let code = vec![109 | (1 << 24)];
        assert!(translate(&container(&code), &Bindings::default()).is_err());
    }
}
