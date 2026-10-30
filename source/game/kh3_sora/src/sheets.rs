//! The core's numbers at run time. The reference compiles its sheets in; here nothing of KH3's is in the
//! repository: `install` receives what the launcher read from the player's own files (the `data` crate's recipes)
//! and the tables look their values up by the sheet's row ids.

use std::collections::HashMap;
use std::sync::OnceLock;

/// One value of a sheet row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PV {
    /// A number.
    N(f32),
    B(bool),
    /// A name.
    S(&'static str),
    /// A list of numbers.
    A(&'static [f32]),
    /// A list of names.
    L(&'static [&'static str]),
    /// Nothing (an optional cell).
    Nil,
}

/// The rows of every parameter sheet (ascending by id) and every table.
#[derive(Debug, Default)]
pub struct Store {
    pub sheets: HashMap<String, Vec<(&'static str, PV)>>,
    pub tables: HashMap<String, Vec<HashMap<&'static str, PV>>>,
}

static STORE: OnceLock<Store> = OnceLock::new();

/// Installs the values (once a process; a later call with the same install changes nothing).
pub fn install(store: Store) {
    let _ = STORE.set(store);
}

/// Whether the values are there.
pub fn installed() -> bool {
    STORE.get().is_some()
}

fn store() -> &'static Store {
    STORE.get().expect("kh3_sora: the sheets are not installed")
}

/// A parameter sheet.
#[derive(Clone, Copy, Debug)]
pub struct Sheet {
    name: &'static str,
    rows: &'static [(&'static str, PV)],
}

impl Sheet {
    pub fn named(name: &'static str) -> Sheet {
        let rows = store()
            .sheets
            .get(name)
            .unwrap_or_else(|| panic!("kh3_sora: no sheet {name}"));
        Sheet { name, rows }
    }

    pub fn rows(&self) -> &'static [(&'static str, PV)] {
        self.rows
    }

    pub fn find(&self, id: &str) -> Option<PV> {
        self.rows
            .binary_search_by(|(key, _)| (*key).cmp(id))
            .ok()
            .map(|i| self.rows[i].1)
    }

    fn need(&self, id: &str) -> PV {
        self.find(id)
            .unwrap_or_else(|| panic!("kh3_sora: sheet {}: no row {id:?}", self.name))
    }

    pub fn num(&self, id: &str) -> f32 {
        match self.need(id) {
            PV::N(v) => v,
            PV::B(b) => f32::from(u8::from(b)),
            other => panic!("kh3_sora: sheet {}: {id:?} is {other:?}", self.name),
        }
    }

    pub fn int(&self, id: &str) -> i32 {
        self.num(id) as i32
    }

    pub fn flag(&self, id: &str) -> bool {
        match self.need(id) {
            PV::B(b) => b,
            PV::N(v) => v != 0.,
            other => panic!("kh3_sora: sheet {}: {id:?} is {other:?}", self.name),
        }
    }

    pub fn text(&self, id: &str) -> &'static str {
        match self.need(id) {
            PV::S(s) => s,
            other => panic!("kh3_sora: sheet {}: {id:?} is {other:?}", self.name),
        }
    }

    pub fn arr<const N: usize>(&self, id: &str) -> [f32; N] {
        match self.need(id) {
            PV::A(a) if a.len() == N => {
                let mut out = [0.; N];
                out.copy_from_slice(a);
                out
            }
            other => panic!("kh3_sora: sheet {}: {id:?} is {other:?}", self.name),
        }
    }
}

/// A table sheet's rows.
#[derive(Clone, Copy, Debug)]
pub struct Table {
    name: &'static str,
    rows: &'static [HashMap<&'static str, PV>],
}

impl Table {
    pub fn named(name: &'static str) -> Table {
        let rows = store()
            .tables
            .get(name)
            .unwrap_or_else(|| panic!("kh3_sora: no table {name}"));
        Table { name, rows }
    }

    pub fn rows(self) -> impl Iterator<Item = Row> {
        let name = self.name;
        self.rows.iter().map(move |cells| Row { name, cells })
    }
}

/// One row of a table.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    name: &'static str,
    cells: &'static HashMap<&'static str, PV>,
}

impl Row {
    fn need(&self, col: &str) -> PV {
        *self
            .cells
            .get(col)
            .unwrap_or_else(|| panic!("kh3_sora: table {}: no column {col}", self.name))
    }

    pub fn text(&self, col: &str) -> &'static str {
        match self.need(col) {
            PV::S(s) => s,
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn num(&self, col: &str) -> f32 {
        match self.need(col) {
            PV::N(v) => v,
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn int(&self, col: &str) -> i32 {
        self.num(col) as i32
    }

    pub fn uint(&self, col: &str) -> u32 {
        self.num(col) as u32
    }

    pub fn flag(&self, col: &str) -> bool {
        match self.need(col) {
            PV::B(b) => b,
            PV::N(v) => v != 0.,
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn yesno(&self, col: &str) -> bool {
        match self.need(col) {
            PV::B(b) => b,
            PV::S("yes") => true,
            PV::S("no") => false,
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn nums(&self, col: &str) -> &'static [f32] {
        match self.need(col) {
            PV::A(a) => a,
            PV::L([]) => &[],
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn ints(&self, col: &str) -> &'static [i32] {
        let nums = self.nums(col);
        let list: Vec<i32> = nums.iter().map(|v| *v as i32).collect();
        Box::leak(list.into_boxed_slice())
    }

    pub fn texts(&self, col: &str) -> &'static [&'static str] {
        match self.need(col) {
            PV::L(l) => l,
            PV::A([]) => &[],
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn opt_num(&self, col: &str) -> Option<f32> {
        match self.need(col) {
            PV::Nil | PV::S("-") => None,
            PV::N(v) => Some(v),
            other => panic!("kh3_sora: table {}: {col} is {other:?}", self.name),
        }
    }

    pub fn opt_int(&self, col: &str) -> Option<i32> {
        self.opt_num(col).map(|v| v as i32)
    }
}
