//! The numbers of KH3's Sora (`game/kh3_sora`), read from the player's own install at run time. Each row of
//! `recipes/*.tsv` says where its value is, not what it is: a DataTable cell, a blueprint default, a clip's
//! notify, or (`X`) bytes of the player's executable. No KH3 value is in Git or in this program: without the player's
//! install there are none, and an executable of another build fails the digest of what was read (`EXE_DIGEST`), which
//! leaves the KH3 ruleset unavailable.
//!
//! Recipes (`|`-separated):
//! - `X|<place>`: from the executable, by file offset (hexadecimal). `f@`, `i@`, `u@`, `d@`: an f32, an i32, a u32, an
//!   f64; `b@a,b,c,d`: an f32 from four single bytes; `q@a,b`: the f32 at a over the f32 at b; `t@`: a byte, not zero; `s@at+len,...`: text from these pieces.
//!   `X|A|p;p`: a list of numbers, `X|L|p;p`: of texts. `X|N`: no value. `X|O`: a value of a form the sheet has only as
//!   information.
//! - `=<json>`: a table row's own id (only in a table's first column).
//! - `T|package|row|path[|mode]`: a DataTable cell.
//! - `P|package|export|path[|mode]`: a property of an export.
//! - `C|package`: the name of the blueprint's class.
//! - `B|package|row`: the names of a row's true flags, joined by `|`.
//! - `N|package|clip|notify`: the time a named notify fires. `NC|...`: how many of them there are.
//!
//! A path goes by `.name`, `[i]`, `[@field=value]` and ends in `#len` for a count. Modes turn the raw value into
//! the sheet's: `enum`, `strip=PREFIX`, `mul=K`, the object forms (`pathname`, `clsname`, `objname`).
use super::package::Package;
use super::props::{self, Props, Value};
use super::{Paks, Result, bad, material};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// One value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    N(f32),
    B(bool),
    S(String),
    A(Vec<f32>),
    L(Vec<String>),
    Nil,
}

/// A parameter sheet: its rows ascending by id.
pub type Rows = Vec<(String, Val)>;
/// A table sheet: per row, (column, value).
pub type Cells = Vec<Vec<(String, Val)>>;

/// Everything read.
#[derive(Debug, Default)]
pub struct Sheets {
    pub params: Vec<(&'static str, Rows)>,
    pub tables: Vec<(&'static str, Cells)>,
    /// Rows whose recipe found nothing in this install: (sheet, id, why).
    pub missing: Vec<(&'static str, String, String)>,
    /// The digest of what was read from the executable.
    pub exe_digest: String,
}

macro_rules! recipes {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("recipes/", $name, ".tsv")))),*]
    };
}

/// SHA-256 of the executable's bytes the recipes read, on the build they were made for.
const EXE_DIGEST: &str = "f54098f6af2dc1f7eb894cd5bf20f6188eead542a39be963a129528be31e1860";

/// The sheet `missing` names for an executable of another build.
pub const OTHER_BUILD: &str = "executable";

const PARAMS: &[(&str, &str)] = recipes![
    "sora-movement",
    "sora-actions",
    "sora-combat-params",
    "sora-stats-rules",
    "sora-bossfinish",
    "sora-magic-spells",
    "sora-magic-projectiles",
    "sora-magic-mp",
    "sora-magic-shotlocks",
    "sora-magic-items",
    "sora-xs-abilities",
    "sora-xs-battle-levels",
];

const TABLES: &[(&str, &str)] = recipes![
    "sora-combat-attacks",
    "sora-combat-hits",
    "sora-combat-collisions",
    "sora-combat-shots",
    "sora-combat-attack-data",
    "sora-combat-rules",
    "sora-combat-reaction-rules",
    "sora-combat-form-commands",
    "sora-combat-form-abilities",
    "sora-levels",
    "sora-abilities",
    "sora-difficulty",
    "sora-critical-start",
    "sora-equipment",
];

struct Loaded {
    package: Package,
    /// Export name to its properties, read on first use.
    props: HashMap<String, Arc<Props>>,
    /// A DataTable's rows by lower-case name.
    rows: Option<Arc<HashMap<String, Props>>>,
}

/// The packages recipes read, each opened once.
struct Reader<'a> {
    paks: &'a Paks,
    open: HashMap<String, Option<Loaded>>,
    exe: ExeBytes,
}

/// The executable's bytes and the digest of those read.
struct ExeBytes {
    image: Vec<u8>,
    digest: Sha256,
}

impl ExeBytes {
    fn new(image: Vec<u8>) -> Self {
        Self {
            image,
            digest: Sha256::new(),
        }
    }

    fn take(&mut self, at: &str, len: usize) -> Result<&[u8]> {
        let at = usize::from_str_radix(at, 16).map_err(|_| bad("a bad offset"))?;
        let bytes = at
            .checked_add(len)
            .and_then(|end| self.image.get(at..end))
            .ok_or_else(|| bad("past the executable's end"))?;
        self.digest.update(bytes);
        Ok(bytes)
    }

    fn word(&mut self, at: &str) -> Result<[u8; 4]> {
        let b = self.take(at, 4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    /// One place: a number, a flag or a text.
    fn place(&mut self, place: &str) -> Result<Val> {
        let (kind, at) = place.split_once('@').ok_or_else(|| bad("a bad place"))?;
        Ok(match kind {
            "f" => Val::N(f32::from_le_bytes(self.word(at)?)),
            "i" => Val::N(i32::from_le_bytes(self.word(at)?) as f32),
            "u" => Val::N(u32::from_le_bytes(self.word(at)?) as f32),
            "d" => {
                let b = self.take(at, 8)?;
                let mut x = [0; 8];
                x.copy_from_slice(b);
                Val::N(f64::from_le_bytes(x) as f32)
            }
            "q" => {
                let (a, b) = at.split_once(',').ok_or_else(|| bad("a bad quotient"))?;
                let a = f32::from_le_bytes(self.word(a)?);
                Val::N(a / f32::from_le_bytes(self.word(b)?))
            }
            // Four bytes, each from its own place.
            "b" => {
                let mut x = [0; 4];
                for (slot, at) in x.iter_mut().zip(at.split(',')) {
                    *slot = self.take(at, 1)?[0];
                }
                Val::N(f32::from_le_bytes(x))
            }
            "t" => Val::B(self.take(at, 1)?[0] != 0),
            "s" => {
                let mut text = Vec::new();
                for piece in at.split(',').filter(|p| !p.is_empty()) {
                    let (at, len) = piece.split_once('+').ok_or_else(|| bad("a bad piece"))?;
                    let len = usize::from_str_radix(len, 16).map_err(|_| bad("a bad length"))?;
                    text.extend_from_slice(self.take(at, len)?);
                }
                Val::S(String::from_utf8_lossy(&text).into_owned())
            }
            _ => return Err(bad("a bad place")),
        })
    }

    fn value(&mut self, recipe: &str) -> Result<Val> {
        let places = |list: &'_ str| -> Vec<String> {
            list.split(';')
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect()
        };
        match recipe {
            "N" => Ok(Val::Nil),
            "O" => Err(bad("an object constant")),
            _ => {
                if let Some(list) = recipe.strip_prefix("A|") {
                    let mut out = Vec::new();
                    for p in places(list) {
                        if let Val::N(n) = self.place(&p)? {
                            out.push(n);
                        }
                    }
                    Ok(Val::A(out))
                } else if let Some(list) = recipe.strip_prefix("L|") {
                    let mut out = Vec::new();
                    for p in places(list) {
                        if let Val::S(s) = self.place(&p)? {
                            out.push(s);
                        }
                    }
                    Ok(Val::L(out))
                } else {
                    self.place(recipe)
                }
            }
        }
    }
}

fn to_game(pkg: &str) -> String {
    format!("kingdom hearts iii/content/{}", pkg.to_ascii_lowercase())
}

impl Reader<'_> {
    fn loaded(&mut self, pkg: &str) -> Result<&mut Loaded> {
        if !self.open.contains_key(pkg) {
            let package = self.paks.package(&to_game(pkg))?;
            self.open.insert(
                pkg.to_owned(),
                package.map(|package| Loaded {
                    package,
                    props: HashMap::new(),
                    rows: None,
                }),
            );
        }
        self.open
            .get_mut(pkg)
            .and_then(Option::as_mut)
            .ok_or_else(|| bad(format!("no package {pkg}")))
    }

    fn export(&mut self, pkg: &str, name: &str) -> Result<(Arc<Props>, &Package)> {
        let l = self.loaded(pkg)?;
        if !l.props.contains_key(name) {
            let i = l
                .package
                .export_named(name)
                .ok_or_else(|| bad(format!("{pkg}: no export {name}")))?;
            let (p, _) = props::export_props(&l.package, i)?;
            l.props.insert(name.to_owned(), Arc::new(p));
        }
        Ok((Arc::clone(&l.props[name]), &l.package))
    }

    fn row(&mut self, pkg: &str, row: &str) -> Result<(Props, &Package)> {
        let l = self.loaded(pkg)?;
        if l.rows.is_none() {
            let i = l
                .package
                .export_of_class("DataTable")
                .ok_or_else(|| bad(format!("{pkg}: not a table")))?;
            let map = props::data_table_rows(&l.package, i)?
                .into_iter()
                .map(|(n, p)| (n.to_ascii_lowercase(), p))
                .collect();
            l.rows = Some(Arc::new(map));
        }
        let rows = l.rows.clone().unwrap_or_default();
        let row = rows
            .get(&row.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| bad(format!("{pkg}: no row {row}")))?;
        Ok((row, &self.loaded(pkg)?.package))
    }
}

/// A step of a path.
enum Step<'a> {
    Name(&'a str, i32),
    Index(usize),
    Key(&'a str, &'a str),
}

fn steps(path: &str) -> Vec<Step<'_>> {
    let mut out = Vec::new();
    for part in path.split('.') {
        let (head, mut rest) = part.split_once('[').map_or((part, ""), |(h, r)| (h, r));
        if !head.is_empty() {
            let (name, index) = head
                .split_once('%')
                .map_or((head, 0), |(n, i)| (n, i.parse().unwrap_or(0)));
            out.push(Step::Name(name, index));
        }
        while !rest.is_empty() {
            let (inner, after) = rest.split_once(']').unwrap_or((rest, ""));
            if let Some(key) = inner.strip_prefix('@') {
                let (k, v) = key.split_once('=').unwrap_or((key, ""));
                out.push(Step::Key(k, v));
            } else if let Ok(i) = inner.parse() {
                out.push(Step::Index(i));
            }
            rest = after.strip_prefix('[').unwrap_or("");
        }
    }
    out
}

/// A value's end form: a number list is walked by index too.
#[derive(Clone, Debug)]
enum Found<'a> {
    Value(&'a Value),
    Number(f32),
}

fn key_matches(item: &Value, field: &str, want: &str) -> bool {
    let Some(p) = item.as_props() else {
        return false;
    };
    match p.get(field) {
        Some(Value::Name(s) | Value::Str(s)) => s == want || s.rsplit("::").next() == Some(want),
        Some(Value::Int(i)) => want.parse() == Ok(*i),
        Some(Value::Byte(b)) => want.parse() == Ok(i64::from(*b)),
        _ => false,
    }
}

fn walk<'a>(p: &'a Props, path: &str) -> Option<Found<'a>> {
    let path = path.strip_suffix("#len").unwrap_or(path);
    let mut list = steps(path).into_iter();
    let Some(Step::Name(first, index)) = list.next() else {
        return None;
    };
    let mut at =
        p.0.iter()
            .find(|q| q.name == first && q.index == index)
            .map(|q| Found::Value(&q.value))?;
    for step in list {
        at = match (at, step) {
            (Found::Value(Value::Struct(_, q)), Step::Name(n, i)) => {
                q.0.iter()
                    .find(|x| x.name == n && x.index == i)
                    .map(|x| Found::Value(&x.value))?
            }
            (Found::Value(Value::Array(a)), Step::Index(i)) => Found::Value(a.get(i)?),
            (Found::Value(Value::Array(a)), Step::Key(k, v)) => {
                Found::Value(a.iter().find(|x| key_matches(x, k, v))?)
            }
            (Found::Value(Value::Floats(f)), Step::Index(i)) => Found::Number(*f.get(i)?),
            (Found::Value(Value::Ints(f)), Step::Index(i)) => Found::Number(*f.get(i)? as f32),
            _ => return None,
        };
    }
    Some(at)
}

/// The raw value to the sheet's form.
fn finish(p: &Package, found: &Found, path: &str, mode: &str) -> Result<Val> {
    if path.ends_with("#len") {
        return Ok(match found {
            Found::Value(Value::Array(a)) => Val::N(a.len() as f32),
            _ => Val::N(0.),
        });
    }
    let mul = mode
        .strip_prefix("mul=")
        .and_then(|m| m.parse::<f32>().ok())
        .unwrap_or(1.);
    Ok(match found {
        Found::Number(n) => Val::N(n * mul),
        Found::Value(v) => match v {
            Value::Float(f) => Val::N(f * mul),
            Value::Int(i) => Val::N(*i as f32 * mul),
            Value::Byte(b) => Val::N(f32::from(*b) * mul),
            Value::Bool(b) if mode == "bool" => Val::N(f32::from(u8::from(*b))),
            Value::Bool(b) => Val::B(*b),
            Value::Floats(f) => Val::A(f.iter().map(|x| x * mul).collect()),
            Value::Ints(f) => Val::A(f.iter().map(|x| *x as f32 * mul).collect()),
            Value::Str(s) | Value::Name(s) => Val::S(text(s, mode)),
            Value::Object(o) => {
                let path = material::import_path(p, *o).unwrap_or_default();
                object(&path, mode)
            }
            Value::Array(a) => {
                let names: Vec<String> = a
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| text(s, mode)))
                    .collect();
                if names.len() == a.len() {
                    Val::L(names)
                } else {
                    Val::A(a.iter().filter_map(Value::as_f32).collect())
                }
            }
            other => return Err(bad(format!("a value of an unknown form: {other:?}"))),
        },
    })
}

fn text(raw: &str, mode: &str) -> String {
    let tail = raw.rsplit("::").next().unwrap_or(raw);
    if mode == "enum" {
        tail.to_owned()
    } else if mode == "enumlc" {
        tail.to_ascii_lowercase()
    } else if mode == "enumuc" {
        tail.to_ascii_uppercase()
    } else if let Some(pre) = mode.strip_prefix("strip=") {
        tail.strip_prefix(pre).unwrap_or(tail).to_owned()
    } else if let Some(pre) = mode.strip_prefix("stripk=") {
        tail.strip_prefix(pre).unwrap_or(tail).to_ascii_lowercase()
    } else {
        raw.to_owned()
    }
}

fn object(path: &str, mode: &str) -> Val {
    let short = path.rsplit('/').next().unwrap_or(path);
    Val::S(match mode {
        "pathname" => short.to_owned(),
        "clsname" => short.rsplit('.').next().unwrap_or(short).to_owned(),
        "objname" => short.split('.').next().unwrap_or(short).to_owned(),
        "pkgpath" => path.split('.').next().unwrap_or(path).to_owned(),
        "classof" => format!("{}_C", short.split('.').next().unwrap_or(short)),
        "x" => {
            let package = path.split('.').next().unwrap_or(path);
            let name = package.rsplit('/').next().unwrap_or(package);
            format!("{package}:{name}_C")
        }
        _ => path.to_owned(),
    })
}

fn literal(json: &str) -> Result<Val> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    Ok(match v {
        serde_json::Value::Null => Val::Nil,
        serde_json::Value::Bool(b) => Val::B(b),
        serde_json::Value::Number(n) => Val::N(n.as_f64().unwrap_or(0.) as f32),
        serde_json::Value::String(s) => Val::S(s),
        serde_json::Value::Array(a) => {
            if a.iter().all(serde_json::Value::is_number) {
                Val::A(a.iter().map(|x| x.as_f64().unwrap_or(0.) as f32).collect())
            } else {
                Val::L(
                    a.iter()
                        .map(|x| x.as_str().unwrap_or("").to_owned())
                        .collect(),
                )
            }
        }
        serde_json::Value::Object(_) => return Err(bad("an object constant")),
    })
}

/// A notify's list entry by its name.
fn notify_time(item: &Value) -> Option<f32> {
    let p = item.as_props()?;
    let link = p
        .f32("LinkValue")
        .or_else(|| p.f32("DisplayTime"))
        .unwrap_or(0.);
    Some(link + p.f32("TriggerTimeOffset").unwrap_or(0.))
}

fn evaluate(r: &mut Reader, recipe: &str) -> Result<Val> {
    if let Some(json) = recipe.strip_prefix('=') {
        return literal(json);
    }
    if let Some(place) = recipe.strip_prefix("X|") {
        return r.exe.value(place);
    }
    let mut parts = recipe.split('|');
    let kind = parts.next().unwrap_or("");
    let pkg = parts.next().unwrap_or("");
    match kind {
        "T" => {
            let (row, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let mode = parts.next().unwrap_or("");
            let (p, package) = r.row(pkg, row)?;
            if mode == "abil" {
                // The level table's ability column: "-" for none.
                let found = walk(&p, path).ok_or_else(|| bad(format!("{recipe}: no value")))?;
                let raw = finish(package, &found, path, "enum")?;
                return Ok(match raw {
                    Val::S(s) if s.ends_with("NONE") => Val::S("-".to_owned()),
                    other => other,
                });
            }
            let found = walk(&p, path).ok_or_else(|| bad(format!("{recipe}: no value")))?;
            finish(package, &found, path, mode)
        }
        "P" => {
            let (export, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let mode = parts.next().unwrap_or("");
            let (p, package) = r.export(pkg, export)?;
            let found = walk(&p, path).ok_or_else(|| bad(format!("{recipe}: no value")))?;
            finish(package, &found, path, mode)
        }
        "C" => {
            let l = r.loaded(pkg)?;
            l.package
                .exports
                .iter()
                .find(|e| e.name.ends_with("_C") && !e.name.starts_with("Default__"))
                .map(|e| Val::S(e.name.clone()))
                .ok_or_else(|| bad(format!("{pkg}: no class")))
        }
        "B" => {
            let row = parts.next().unwrap_or("");
            let (p, _) = r.row(pkg, row)?;
            let names: Vec<String> =
                p.0.iter()
                    .filter(|q| q.value == Value::Bool(true))
                    .map(|q| q.name.replace("m_bIs", "").replace("m_b", ""))
                    .collect();
            Ok(Val::S(names.join("|")))
        }
        "N" | "NC" => {
            let (clip, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let (p, _) = r.export(pkg, clip)?;
            let hits: Vec<&Value> = p
                .array("Notifies")
                .unwrap_or(&[])
                .iter()
                .filter(|x| key_matches(x, "NotifyName", name))
                .collect();
            if kind == "NC" {
                return Ok(Val::N(hits.len() as f32));
            }
            hits.first()
                .and_then(|x| notify_time(x))
                .map(Val::N)
                .ok_or_else(|| bad(format!("{recipe}: no such notify")))
        }
        other => Err(bad(format!("an unknown recipe kind {other}"))),
    }
}

/// Reads every sheet. A row that cannot be read is listed in `missing`; a caller treats any as an unsupported
/// install.
pub fn read(paks: &Paks) -> Sheets {
    let image = std::fs::read(&paks.exe).unwrap_or_default();
    let mut r = Reader {
        paks,
        open: HashMap::new(),
        exe: ExeBytes::new(image),
    };
    let mut out = Sheets::default();
    for (name, text) in PARAMS {
        let mut rows = Vec::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut cols = line.split('\t');
            let (id, recipe) = (cols.next().unwrap_or(""), cols.next().unwrap_or(""));
            match evaluate(&mut r, recipe) {
                Ok(v) => rows.push((id.to_owned(), v)),
                Err(e) => out.missing.push((name, id.to_owned(), e.to_string())),
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        out.params.push((name, rows));
    }
    for (name, text) in TABLES {
        let mut lines = text.lines();
        let cols: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        let mut rows = Vec::new();
        for line in lines.filter(|l| !l.is_empty()) {
            let mut cells = Vec::with_capacity(cols.len());
            for (col, recipe) in cols.iter().zip(line.split('\t')) {
                match evaluate(&mut r, recipe) {
                    Ok(v) => cells.push(((*col).to_owned(), v)),
                    Err(e) => out
                        .missing
                        .push((name, format!("{col}: {recipe}"), e.to_string())),
                }
            }
            rows.push(cells);
        }
        out.tables.push((name, rows));
    }
    let digest: String = r
        .exe
        .digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if digest != EXE_DIGEST {
        out.missing.insert(
            0,
            (
                OTHER_BUILD,
                digest.clone(),
                "this build of the game's executable is not supported".to_owned(),
            ),
        );
    }
    out.exe_digest = digest;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_read_the_executable() {
        let mut image = vec![0u8; 16];
        image[4..8].copy_from_slice(&2.5f32.to_le_bytes());
        image[8..12].copy_from_slice(&(-3i32).to_le_bytes());
        image[12..16].copy_from_slice(b"rock");
        let mut e = ExeBytes::new(image);
        assert_eq!(e.value("f@4").unwrap(), Val::N(2.5));
        assert_eq!(e.value("i@8").unwrap(), Val::N(-3.));
        assert_eq!(e.value("q@4,4").unwrap(), Val::N(1.));
        assert_eq!(e.value("t@4").unwrap(), Val::B(false));
        assert_eq!(e.value("s@c+2,e+2").unwrap(), Val::S("rock".to_owned()));
        assert_eq!(e.value("A|f@4;i@8").unwrap(), Val::A(vec![2.5, -3.]));
        assert_eq!(e.value("L|s@c+4").unwrap(), Val::L(vec!["rock".to_owned()]));
        assert_eq!(e.value("N").unwrap(), Val::Nil);
        assert!(e.value("f@ff").is_err());
    }

    /// The recipes name where a value is. Only a table row's id may be written out.
    #[test]
    fn recipes_carry_no_values() {
        for (name, text) in PARAMS {
            for line in text.lines().filter(|l| !l.is_empty()) {
                let recipe = line.split('\t').nth(1).unwrap_or("");
                assert!(!recipe.starts_with('='), "{name}: {line}");
            }
        }
        for (name, text) in TABLES {
            for line in text.lines().skip(1) {
                for recipe in line.split('\t').skip(1) {
                    assert!(!recipe.starts_with('='), "{name}: {line}");
                }
            }
        }
    }

    #[test]
    fn paths_walk_structs_lists_and_keys() {
        let unit = |group: &str, size: f32| {
            Value::Struct(
                "Unit".into(),
                Props(vec![
                    props::Prop {
                        name: "m_GrpName".into(),
                        index: 0,
                        struct_name: String::new(),
                        value: Value::Name(group.into()),
                    },
                    props::Prop {
                        name: "m_Size".into(),
                        index: 0,
                        struct_name: "Vector".into(),
                        value: Value::Floats(vec![size, 0., 2.]),
                    },
                ]),
            )
        };
        let p = Props(vec![props::Prop {
            name: "Shapes".into(),
            index: 0,
            struct_name: String::new(),
            value: Value::Array(vec![unit("A", 1.), unit("B", 7.)]),
        }]);
        let found = walk(&p, "Shapes[@m_GrpName=B].m_Size[0]").unwrap();
        assert!(matches!(found, Found::Number(n) if n == 7.));
        let found = walk(&p, "Shapes[0].m_Size").unwrap();
        assert!(matches!(found, Found::Value(Value::Floats(_))));
        assert!(walk(&p, "Shapes[@m_GrpName=C].m_Size[0]").is_none());
        assert!(
            matches!(walk(&p, "Shapes#len"), Some(Found::Value(Value::Array(a))) if a.len() == 2)
        );
    }

    #[test]
    fn modes_shape_the_values() {
        assert_eq!(text("E::TRES_ACS_SPHERE", "strip=TRES_ACS_"), "SPHERE");
        assert_eq!(text("E::THING", "enum"), "THING");
        assert_eq!(object("/Game/A/B.B_C", "clsname"), Val::S("B_C".to_owned()));
        assert_eq!(literal("[1, 2.5]").unwrap(), Val::A(vec![1., 2.5]));
        assert_eq!(literal("[\"a\"]").unwrap(), Val::L(vec!["a".to_owned()]));
        assert_eq!(literal("null").unwrap(), Val::Nil);
    }
}
