//! The numbers of KH3's Sora (`game/kh3_sora`), read from the player's own install at run time. Each row of
//! `recipes/*.tsv` says where its value is, not what it is: a DataTable cell, a blueprint default, a clip's
//! notify, or (`=`) a constant of the executable's code.
//!
//! Recipes (`|`-separated):
//! - `=<json>`: a constant.
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
}

macro_rules! recipes {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("recipes/", $name, ".tsv")))),*]
    };
}

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
    let mut r = Reader {
        paks,
        open: HashMap::new(),
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
