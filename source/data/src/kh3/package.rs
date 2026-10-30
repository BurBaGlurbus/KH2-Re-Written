//! Cooked UE 4.17 packages: the `.uasset` header (summary, names, imports,
//! exports) over its `.uexp` and `.ubulk`, and the byte cursor the other
//! readers use. Layout from StrawHatPro's `docs/formats/uasset.md` (MIT).
use super::{Result, bad};

const TAG: u32 = 0x9e2a_83c1;

/// A bounds-checked little-endian cursor.
#[derive(Clone)]
pub struct Rd<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }

    pub fn at(b: &'a [u8], p: usize) -> Self {
        Self { b, p }
    }

    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.p.checked_add(n).ok_or_else(|| bad("overflow"))?;
        let s = self
            .b
            .get(self.p..end)
            .ok_or_else(|| bad(format!("read past the end at {} (+{n})", self.p)))?;
        self.p = end;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn u64(&mut self) -> Result<u64> {
        let s = self.take(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(s);
        Ok(u64::from_le_bytes(a))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(self.u64()? as i64)
    }

    /// A count that must fit in what is left, at `least` bytes an element.
    pub fn count(&mut self, least: usize) -> Result<usize> {
        let n = self.i32()?;
        let fits = n >= 0 && (n as usize).saturating_mul(least.max(1)) <= self.left();
        if !fits {
            return Err(bad(format!("bad count {n} at {}", self.p)));
        }
        Ok(n as usize)
    }

    /// An `FString`: i32 length with the NUL; negative means UTF-16.
    pub fn fstring(&mut self) -> Result<String> {
        let len = self.i32()?;
        if len == 0 {
            return Ok(String::new());
        }
        if len > 0 {
            let s = self.take(len as usize)?;
            Ok(s[..s.len() - 1].iter().map(|&c| c as char).collect())
        } else {
            let s = self.take(len.unsigned_abs() as usize * 2)?;
            let units: Vec<u16> = s
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            Ok(String::from_utf16_lossy(&units[..units.len() - 1]))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub class_package: String,
    pub class: String,
    pub outer: i32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Export {
    pub class: i32,
    pub sup: i32,
    pub template: i32,
    pub outer: i32,
    pub name: String,
    pub flags: u32,
    pub size: i64,
    pub offset: i64,
    pub is_asset: bool,
}

/// One cooked package.
#[derive(Debug, Default)]
pub struct Package {
    pub names: Vec<String>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub header_size: i64,
    pub bulk_start: i64,
    pub uexp: Vec<u8>,
    pub ubulk: Vec<u8>,
}

impl Package {
    /// `uasset`: the header; `uexp`, `ubulk`: the sibling files (empty when
    /// the package has none).
    pub fn parse(uasset: &[u8], uexp: Vec<u8>, ubulk: Vec<u8>) -> Result<Self> {
        let mut r = Rd::new(uasset);
        if r.u32()? != TAG {
            return Err(bad("not a package"));
        }
        let legacy = r.i32()?;
        if legacy != -7 {
            return Err(bad(format!("package version {legacy}")));
        }
        r.skip(12)?;
        let custom = r.count(20)?;
        r.skip(custom * 20)?;
        let header_size = i64::from(r.i32()?);
        r.fstring()?;
        r.u32()?;
        let (name_count, name_offset) = (r.i32()?, r.i32()?);
        r.skip(8)?;
        let (export_count, export_offset) = (r.i32()?, r.i32()?);
        let (import_count, import_offset) = (r.i32()?, r.i32()?);
        r.skip(4 + 8 + 8)?;
        r.skip(16)?;
        let generations = r.count(8)?;
        r.skip(generations * 8)?;
        for _ in 0..2 {
            r.skip(2 + 2 + 2 + 4)?;
            r.fstring()?;
        }
        r.u32()?;
        let chunks = r.count(1)?;
        if chunks != 0 {
            return Err(bad("compressed package"));
        }
        r.u32()?;
        let cook = r.count(4)?;
        for _ in 0..cook {
            r.fstring()?;
        }
        r.i32()?;
        let bulk_start = r.i64()?;

        let table = |count: i32, offset: i32, what: &str| -> Result<(usize, usize)> {
            if count < 0 || offset < 0 || offset as usize > uasset.len() {
                return Err(bad(format!("bad {what} table")));
            }
            Ok((count as usize, offset as usize))
        };
        let (n, at) = table(name_count, name_offset, "name")?;
        let mut r = Rd::at(uasset, at);
        let mut names = Vec::with_capacity(n);
        for _ in 0..n {
            names.push(r.fstring()?);
            r.skip(4)?;
        }
        let name_of = |r: &mut Rd| -> Result<String> { read_name(r, &names) };
        let (n, at) = table(import_count, import_offset, "import")?;
        let mut r = Rd::at(uasset, at);
        let mut imports = Vec::with_capacity(n);
        for _ in 0..n {
            imports.push(Import {
                class_package: name_of(&mut r)?,
                class: name_of(&mut r)?,
                outer: r.i32()?,
                name: name_of(&mut r)?,
            });
        }
        let (n, at) = table(export_count, export_offset, "export")?;
        let mut r = Rd::at(uasset, at);
        let mut exports = Vec::with_capacity(n);
        for _ in 0..n {
            let (class, sup, template, outer) = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
            let name = name_of(&mut r)?;
            let flags = r.u32()?;
            let (size, offset) = (r.i64()?, r.i64()?);
            r.skip(12 + 16 + 4 + 4)?;
            let is_asset = r.i32()? != 0;
            r.skip(20)?;
            exports.push(Export {
                class,
                sup,
                template,
                outer,
                name,
                flags,
                size,
                offset,
                is_asset,
            });
        }
        Ok(Self {
            names,
            imports,
            exports,
            header_size,
            bulk_start,
            uexp,
            ubulk,
        })
    }

    /// The bytes of export `i` (0-based) in the `.uexp`.
    pub fn export_bytes(&self, i: usize) -> Result<&[u8]> {
        let e = self.exports.get(i).ok_or_else(|| bad("no such export"))?;
        let from =
            usize::try_from(e.offset - self.header_size).map_err(|_| bad("export offset"))?;
        let len = usize::try_from(e.size).map_err(|_| bad("export size"))?;
        self.uexp
            .get(from..from + len)
            .ok_or_else(|| bad(format!("export {} past the .uexp", e.name)))
    }

    /// The class name of export `i`.
    pub fn class_of(&self, i: usize) -> &str {
        self.exports
            .get(i)
            .map_or("", |e| self.object_name(e.class))
    }

    /// The short name of a package index (+export, -import, 0 none).
    pub fn object_name(&self, index: i32) -> &str {
        if index > 0 {
            self.exports
                .get(index as usize - 1)
                .map_or("", |e| e.name.as_str())
        } else if index < 0 {
            self.imports
                .get(index.unsigned_abs() as usize - 1)
                .map_or("", |e| e.name.as_str())
        } else {
            ""
        }
    }

    /// The package path of an import (`/Game/...`), the outermost name.
    pub fn import_package(&self, index: i32) -> Option<&str> {
        let mut at = index;
        let mut guard = 0;
        loop {
            if at >= 0 {
                return None;
            }
            let import = self.imports.get(at.unsigned_abs() as usize - 1)?;
            if import.outer == 0 {
                return Some(&import.name);
            }
            at = import.outer;
            guard += 1;
            if guard > 32 {
                return None;
            }
        }
    }

    /// The first export of a class.
    pub fn export_of_class(&self, class: &str) -> Option<usize> {
        (0..self.exports.len()).find(|&i| self.class_of(i) == class)
    }

    /// The first export of any of the classes.
    pub fn export_of_classes(&self, classes: &[&str]) -> Option<usize> {
        (0..self.exports.len()).find(|&i| classes.contains(&self.class_of(i)))
    }

    /// The export with this name.
    pub fn export_named(&self, name: &str) -> Option<usize> {
        self.exports.iter().position(|e| e.name == name)
    }
}

/// An FName reference: index + number (n means suffix `_<n-1>`).
pub fn read_name(r: &mut Rd, names: &[String]) -> Result<String> {
    let (index, number) = (r.i32()?, r.i32()?);
    let base = usize::try_from(index)
        .ok()
        .and_then(|i| names.get(i))
        .ok_or_else(|| bad(format!("bad name index {index}")))?;
    Ok(if number > 0 {
        format!("{base}_{}", number - 1)
    } else {
        base.clone()
    })
}

/// An i32 count then that many u16s, and such small array shapes.
pub fn read_u16s(r: &mut Rd) -> Result<Vec<u16>> {
    let n = r.count(2)?;
    let s = r.take(n * 2)?;
    Ok(s.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect())
}

/// The package file path of a game path: `/Game/Foo/Bar` to
/// `kingdom hearts iii/content/foo/bar`.
pub fn mounted_of_game_path(game_path: &str) -> String {
    let path = game_path.split('.').next().unwrap_or(game_path);
    let rest = path
        .strip_prefix("/Game/")
        .unwrap_or(path.trim_start_matches('/'));
    format!("kingdom hearts iii/content/{rest}").to_ascii_lowercase()
}

#[cfg(test)]
pub(crate) mod testing {
    /// A little synthetic package writer for the readers' tests.
    #[derive(Default)]
    pub struct Writer {
        pub names: Vec<String>,
        pub imports: Vec<(String, String, i32, String)>,
        pub exports: Vec<(i32, String, Vec<u8>)>,
    }

    pub fn fstring(s: &str) -> Vec<u8> {
        let mut out = (s.len() as i32 + 1).to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out.push(0);
        out
    }

    impl Writer {
        pub fn name(&mut self, s: &str) -> i32 {
            if let Some(i) = self.names.iter().position(|n| n == s) {
                return i as i32;
            }
            self.names.push(s.to_owned());
            self.names.len() as i32 - 1
        }

        pub fn fname(&mut self, s: &str) -> Vec<u8> {
            let mut out = self.name(s).to_le_bytes().to_vec();
            out.extend_from_slice(&0i32.to_le_bytes());
            out
        }

        pub fn import_class(&mut self, class: &str) -> i32 {
            self.name("/Script/Engine");
            self.name("Class");
            self.name(class);
            self.imports
                .push(("/Script/Engine".into(), "Class".into(), 0, class.to_owned()));
            -(self.imports.len() as i32)
        }

        pub fn export(&mut self, class: i32, name: &str, bytes: Vec<u8>) -> i32 {
            self.name(name);
            self.exports.push((class, name.to_owned(), bytes));
            self.exports.len() as i32
        }

        /// (uasset, uexp)
        pub fn build(&self) -> (Vec<u8>, Vec<u8>) {
            let mut body = Vec::new();
            let mut h = Vec::new();
            h.extend_from_slice(&super::TAG.to_le_bytes());
            h.extend_from_slice(&(-7i32).to_le_bytes());
            h.extend_from_slice(&[0; 12]);
            h.extend_from_slice(&0i32.to_le_bytes());
            let header_size_at = h.len();
            h.extend_from_slice(&0i32.to_le_bytes());
            h.extend_from_slice(&fstring("None"));
            h.extend_from_slice(&0u32.to_le_bytes());
            let name_at_slot = h.len();
            h.extend_from_slice(&[0; 8]);
            h.extend_from_slice(&[0; 8]);
            let export_slot = h.len();
            h.extend_from_slice(&[0; 8]);
            let import_slot = h.len();
            h.extend_from_slice(&[0; 8]);
            h.extend_from_slice(&[0; 20]);
            h.extend_from_slice(&[0; 16]);
            h.extend_from_slice(&0i32.to_le_bytes());
            for _ in 0..2 {
                h.extend_from_slice(&[0; 10]);
                h.extend_from_slice(&fstring(""));
            }
            h.extend_from_slice(&0u32.to_le_bytes());
            h.extend_from_slice(&0i32.to_le_bytes());
            h.extend_from_slice(&0u32.to_le_bytes());
            h.extend_from_slice(&0i32.to_le_bytes());
            h.extend_from_slice(&0i32.to_le_bytes());
            h.extend_from_slice(&0i64.to_le_bytes());
            let name_at = h.len() as i32;
            for n in &self.names {
                h.extend_from_slice(&fstring(n));
                h.extend_from_slice(&[0; 4]);
            }
            let import_at = h.len() as i32;
            let index = |s: &str| self.names.iter().position(|n| n == s).unwrap() as i32;
            for (cp, c, outer, n) in &self.imports {
                for s in [cp, c] {
                    h.extend_from_slice(&index(s).to_le_bytes());
                    h.extend_from_slice(&0i32.to_le_bytes());
                }
                h.extend_from_slice(&outer.to_le_bytes());
                h.extend_from_slice(&index(n).to_le_bytes());
                h.extend_from_slice(&0i32.to_le_bytes());
            }
            let export_at = h.len() as i32;
            let total = h.len() as i64 + self.exports.len() as i64 * 104;
            for (class, name, bytes) in &self.exports {
                let offset = total + body.len() as i64;
                h.extend_from_slice(&class.to_le_bytes());
                h.extend_from_slice(&[0; 12]);
                h.extend_from_slice(&index(name).to_le_bytes());
                h.extend_from_slice(&0i32.to_le_bytes());
                h.extend_from_slice(&0u32.to_le_bytes());
                h.extend_from_slice(&(bytes.len() as i64).to_le_bytes());
                h.extend_from_slice(&offset.to_le_bytes());
                h.extend_from_slice(&[0; 12 + 16 + 4 + 4]);
                h.extend_from_slice(&1i32.to_le_bytes());
                h.extend_from_slice(&[0; 20]);
                body.extend_from_slice(bytes);
            }
            let put = |h: &mut Vec<u8>, at: usize, v: i32| {
                h[at..at + 4].copy_from_slice(&v.to_le_bytes());
            };
            put(&mut h, header_size_at, total as i32);
            put(&mut h, name_at_slot, self.names.len() as i32);
            put(&mut h, name_at_slot + 4, name_at);
            put(&mut h, export_slot, self.exports.len() as i32);
            put(&mut h, export_slot + 4, export_at);
            put(&mut h, import_slot, self.imports.len() as i32);
            put(&mut h, import_slot + 4, import_at);
            (h, body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Writer;
    use super::*;

    #[test]
    fn header_tables_round_trip() {
        let mut w = Writer::default();
        let class = w.import_class("Thing");
        w.export(class, "Asset", vec![1, 2, 3, 4]);
        w.export(class, "Second", vec![9]);
        let (head, body) = w.build();
        let p = Package::parse(&head, body, Vec::new()).unwrap();
        assert_eq!(p.exports.len(), 2);
        assert_eq!(p.class_of(0), "Thing");
        assert_eq!(p.export_bytes(0).unwrap(), [1, 2, 3, 4]);
        assert_eq!(p.export_bytes(1).unwrap(), [9]);
        assert_eq!(p.export_of_class("Thing"), Some(0));
        assert_eq!(p.export_named("Second"), Some(1));
        assert_eq!(p.import_package(class), Some("Thing"));
    }

    #[test]
    fn bad_tag_is_refused() {
        assert!(Package::parse(&[0; 64], Vec::new(), Vec::new()).is_err());
    }

    #[test]
    fn game_paths_map_to_mounted_files() {
        assert_eq!(
            mounted_of_game_path("/Game/Character/Foo.Foo"),
            "kingdom hearts iii/content/character/foo"
        );
    }

    #[test]
    fn cursor_is_bounded() {
        let mut r = Rd::new(&[1, 0]);
        assert_eq!(r.u16().unwrap(), 1);
        assert!(r.u8().is_err());
    }
}
