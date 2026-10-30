//! The generated tables of the magic side: the five `sheets/port/sora-magic-*` sheets as sorted
//! lookup tables (`tools/gen_port.js` `values: true`, registry `tools/lib/port_gen_defs_magic.js`),
//! and the typed accessors the rest of `magic/` reads them through.
//!
//! The sheets are parameter-shaped (one row = one value, `<group>.<parameter>`) and uniform per
//! spell / stage / attack row, so the code composes ids ("fira.burn_shape0_size") instead of
//! naming five thousand struct fields. A missing id is a data error of the build: the accessors
//! panic with the id, and `tests/magic.rs` reads every spell, stage and row once.

pub use crate::sheets::PV;

/// Which sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    /// `sora-magic-spells`: the cast path (part A).
    Spells,
    /// `sora-magic-projectiles`: the spell actors (parts B, I).
    Proj,
    /// `sora-magic-mp`: MP, costs, attack rows (part C).
    Mp,
    /// `sora-magic-shotlocks`: focus, aim mode, shotlocks, airstep (parts D, F, G, H).
    Shot,
    /// `sora-magic-items`: items, pouch, menu (parts E, J).
    Items,
}

impl Sheet {
    pub const ALL: [Sheet; 5] = [
        Sheet::Spells,
        Sheet::Proj,
        Sheet::Mp,
        Sheet::Shot,
        Sheet::Items,
    ];

    /// The rows, ascending by id.
    pub fn rows(self) -> &'static [(&'static str, PV)] {
        crate::sheets::Sheet::named(self.name()).rows()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Sheet::Spells => "sora-magic-spells",
            Sheet::Proj => "sora-magic-projectiles",
            Sheet::Mp => "sora-magic-mp",
            Sheet::Shot => "sora-magic-shotlocks",
            Sheet::Items => "sora-magic-items",
        }
    }

    /// The value of a row, when the sheet has it.
    pub fn find(self, id: &str) -> Option<PV> {
        let rows = self.rows();
        rows.binary_search_by(|(key, _)| (*key).cmp(id))
            .ok()
            .map(|i| rows[i].1)
    }

    fn need(self, id: &str) -> PV {
        match self.find(id) {
            Some(v) => v,
            None => panic!("sheet {}: no row {id:?} (regenerate with node tools/gen_port.js; a row the code reads was renamed or is unchecked)", self.name()),
        }
    }

    /// A number (a boolean reads as 0 / 1).
    pub fn num(self, id: &str) -> f32 {
        match self.need(id) {
            PV::N(v) => v,
            PV::B(b) => b as u8 as f32,
            other => panic!(
                "sheet {}: row {id:?} is {other:?}, not a number",
                self.name()
            ),
        }
    }

    pub fn int(self, id: &str) -> i32 {
        self.num(id) as i32
    }

    pub fn flag(self, id: &str) -> bool {
        match self.need(id) {
            PV::B(b) => b,
            PV::N(v) => v != 0.0,
            other => panic!("sheet {}: row {id:?} is {other:?}, not a flag", self.name()),
        }
    }

    pub fn text(self, id: &str) -> &'static str {
        match self.need(id) {
            PV::S(s) => s,
            other => panic!("sheet {}: row {id:?} is {other:?}, not a name", self.name()),
        }
    }

    pub fn list(self, id: &str) -> &'static [f32] {
        match self.need(id) {
            PV::A(a) => a,
            other => panic!(
                "sheet {}: row {id:?} is {other:?}, not a list of numbers",
                self.name()
            ),
        }
    }

    /// A list of three numbers as a vector.
    pub fn vec3(self, id: &str) -> glam::Vec3 {
        let a = self.list(id);
        glam::Vec3::new(
            a.first().copied().unwrap_or(0.0),
            a.get(1).copied().unwrap_or(0.0),
            a.get(2).copied().unwrap_or(0.0),
        )
    }

    /// A number, or `default` when the sheet has no such row (rows that only some spells have).
    pub fn num_or(self, id: &str, default: f32) -> f32 {
        match self.find(id) {
            Some(PV::N(v)) => v,
            Some(PV::B(b)) => b as u8 as f32,
            _ => default,
        }
    }

    pub fn flag_or(self, id: &str, default: bool) -> bool {
        match self.find(id) {
            Some(PV::B(b)) => b,
            Some(PV::N(v)) => v != 0.0,
            _ => default,
        }
    }

    pub fn text_or(self, id: &str, default: &'static str) -> &'static str {
        match self.find(id) {
            Some(PV::S(s)) => s,
            _ => default,
        }
    }
}
