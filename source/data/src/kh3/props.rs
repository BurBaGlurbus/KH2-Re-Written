//! UE 4.17 tagged properties (FPropertyTag lists). A value this reader does
//! not interpret is kept as raw bytes, its tag's size makes that exact.
//! Layout from StrawHatPro's `docs/formats/uasset.md` (MIT).
use super::package::{Package, Rd, read_name};
use super::{Result, bad};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    Name(String),
    Object(i32),
    Byte(u8),
    /// Natively serialized numbers (Vector, Rotator, Quat, ...), as floats.
    Floats(Vec<f32>),
    /// Natively serialized integers (IntPoint, Color's bytes, ...).
    Ints(Vec<i64>),
    Struct(String, Props),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Raw(Vec<u8>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props(pub Vec<Prop>);

#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    pub name: String,
    pub index: i32,
    pub struct_name: String,
    pub value: Value,
}

impl Props {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|p| p.name == name).map(|p| &p.value)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Prop> + 'a {
        self.0.iter().filter(move |p| p.name == name)
    }

    pub fn f32(&self, name: &str) -> Option<f32> {
        self.get(name).and_then(Value::as_f32)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(Value::as_int)
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(Value::as_bool)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Value::as_str)
    }

    pub fn object(&self, name: &str) -> Option<i32> {
        self.get(name).and_then(Value::as_object)
    }

    pub fn props(&self, name: &str) -> Option<&Props> {
        self.get(name).and_then(Value::as_props)
    }

    pub fn floats(&self, name: &str) -> Option<&[f32]> {
        match self.get(name)? {
            Value::Floats(f) => Some(f),
            _ => None,
        }
    }

    pub fn array(&self, name: &str) -> Option<&[Value]> {
        match self.get(name)? {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
}

impl Value {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Byte(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Name(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<i32> {
        match self {
            Value::Object(o) => Some(*o),
            _ => None,
        }
    }

    pub fn as_props(&self) -> Option<&Props> {
        match self {
            Value::Struct(_, p) => Some(p),
            _ => None,
        }
    }
}

struct Tag {
    name: String,
    kind: String,
    size: i32,
    index: i32,
    struct_name: String,
    bool_val: u8,
    inner: String,
    value: String,
}

fn read_tag(r: &mut Rd, names: &[String]) -> Result<Option<Tag>> {
    let name = read_name(r, names)?;
    if name == "None" {
        return Ok(None);
    }
    let kind = read_name(r, names)?;
    let (size, index) = (r.i32()?, r.i32()?);
    let mut t = Tag {
        name,
        kind,
        size,
        index,
        struct_name: String::new(),
        bool_val: 0,
        inner: String::new(),
        value: String::new(),
    };
    match t.kind.as_str() {
        "StructProperty" => {
            t.struct_name = read_name(r, names)?;
            r.skip(16)?;
        }
        "BoolProperty" => t.bool_val = r.u8()?,
        "ByteProperty" | "EnumProperty" => t.inner = read_name(r, names)?,
        "ArrayProperty" | "SetProperty" => t.inner = read_name(r, names)?,
        "MapProperty" => {
            t.inner = read_name(r, names)?;
            t.value = read_name(r, names)?;
        }
        _ => {}
    }
    if r.u8()? != 0 {
        r.skip(16)?;
    }
    Ok(Some(t))
}

fn native(name: &str) -> Option<&'static [u8]> {
    // f = f32, u = u8, i = i32
    Some(match name {
        "Vector" | "Rotator" => b"fff",
        "Vector2D" => b"ff",
        "Vector4" | "Quat" | "LinearColor" => b"ffff",
        "Color" => b"uuuu",
        "IntPoint" => b"ii",
        "IntVector" => b"iii",
        "Guid" => b"iiii",
        "RichCurveKey" => b"uuuffffff",
        "Box" => b"ffffffu",
        _ => return None,
    })
}

fn read_native(r: &mut Rd, layout: &[u8]) -> Result<Value> {
    if layout.contains(&b'f') {
        let mut floats = Vec::with_capacity(layout.len());
        for &k in layout {
            floats.push(match k {
                b'f' => r.f32()?,
                b'u' => f32::from(r.u8()?),
                _ => r.i32()? as f32,
            });
        }
        Ok(Value::Floats(floats))
    } else {
        let mut ints = Vec::with_capacity(layout.len());
        for &k in layout {
            ints.push(match k {
                b'u' => i64::from(r.u8()?),
                _ => i64::from(r.i32()?),
            });
        }
        Ok(Value::Ints(ints))
    }
}

fn looks_tagged(r: &Rd, names: &[String]) -> bool {
    let mut peek = r.clone();
    let (Ok(index), Ok(number)) = (peek.i32(), peek.i32()) else {
        return false;
    };
    index >= 0 && (index as usize) < names.len() && (0..0x10000).contains(&number)
}

fn read_struct(r: &mut Rd, names: &[String], name: &str) -> Result<Value> {
    if let Some(layout) = native(name) {
        return read_native(r, layout);
    }
    if name == "StringAssetReference" {
        return Ok(Value::Str(r.fstring()?));
    }
    if !looks_tagged(r, names) {
        return Err(bad(format!("struct {name} is not a property list")));
    }
    Ok(Value::Struct(name.to_owned(), read_props(r, names)?))
}

/// One element of an array or a map, without a tag.
fn read_element(r: &mut Rd, names: &[String], kind: &str, struct_name: &str) -> Result<Value> {
    Ok(match kind {
        "IntProperty" => Value::Int(i64::from(r.i32()?)),
        "UInt32Property" => Value::Int(i64::from(r.u32()?)),
        "FloatProperty" => Value::Float(r.f32()?),
        "NameProperty" | "EnumProperty" => Value::Name(read_name(r, names)?),
        "ObjectProperty" => Value::Object(r.i32()?),
        "StrProperty" | "AssetObjectProperty" => Value::Str(r.fstring()?),
        "Int8Property" => Value::Int(i64::from(r.u8()? as i8)),
        "Int16Property" => Value::Int(i64::from(r.i16()?)),
        "UInt16Property" => Value::Int(i64::from(r.u16()?)),
        "Int64Property" => Value::Int(r.i64()?),
        "UInt64Property" => Value::Int(r.u64()? as i64),
        "StructProperty" => read_struct(r, names, struct_name)?,
        _ => return Err(bad(format!("element kind {kind}"))),
    })
}

fn read_value(r: &mut Rd, names: &[String], t: &Tag) -> Result<Value> {
    Ok(match t.kind.as_str() {
        "BoolProperty" => Value::Bool(t.bool_val != 0),
        "ByteProperty" => {
            if t.size == 1 {
                Value::Byte(r.u8()?)
            } else {
                Value::Name(read_name(r, names)?)
            }
        }
        "StructProperty" => read_struct(r, names, &t.struct_name)?,
        "ArrayProperty" => {
            let count = r.count(0)?;
            if t.inner == "StructProperty" {
                let inner = read_tag(r, names)?.ok_or_else(|| bad("array without inner tag"))?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(read_struct(r, names, &inner.struct_name)?);
                }
                Value::Array(items)
            } else if t.inner == "ByteProperty" || t.inner == "BoolProperty" {
                if (t.size as usize).saturating_sub(4) == count {
                    Value::Raw(r.take(count)?.to_vec())
                } else {
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(Value::Name(read_name(r, names)?));
                    }
                    Value::Array(items)
                }
            } else {
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(read_element(r, names, &t.inner, "")?);
                }
                Value::Array(items)
            }
        }
        "MapProperty" => {
            let remove = r.count(0)?;
            for _ in 0..remove {
                read_map_part(r, names, &t.inner)?;
            }
            let count = r.count(0)?;
            let mut entries = Vec::with_capacity(count);
            for _ in 0..count {
                let key = read_map_part(r, names, &t.inner)?;
                let value = read_map_part(r, names, &t.value)?;
                entries.push((key, value));
            }
            Value::Map(entries)
        }
        kind => read_element(r, names, kind, "")?,
    })
}

/// A map's struct value is a property list; the tag does not name the struct.
fn read_map_part(r: &mut Rd, names: &[String], kind: &str) -> Result<Value> {
    if kind == "StructProperty" {
        Ok(Value::Struct(String::new(), read_props(r, names)?))
    } else {
        read_element(r, names, kind, "")
    }
}

/// Reads tags until `None`.
pub fn read_props(r: &mut Rd, names: &[String]) -> Result<Props> {
    let mut out = Vec::new();
    while let Some(t) = read_tag(r, names)? {
        if t.size < 0 || t.size as usize > r.left() {
            return Err(bad(format!(
                "property {} of size {} past the data",
                t.name, t.size
            )));
        }
        let start = r.p;
        let mut value = match read_value(r, names, &t) {
            Ok(v) if r.p == start + t.size as usize => v,
            _ => Value::Raw(Vec::new()),
        };
        if matches!(value, Value::Raw(ref raw) if raw.is_empty()) && t.size > 0 {
            r.p = start;
            value = Value::Raw(r.take(t.size as usize)?.to_vec());
        }
        out.push(Prop {
            name: t.name,
            index: t.index,
            struct_name: t.struct_name,
            value,
        });
    }
    Ok(Props(out))
}

/// The tagged properties at the start of export `i`, and where they end.
pub fn export_props(p: &Package, i: usize) -> Result<(Props, usize)> {
    let bytes = p.export_bytes(i)?;
    let mut r = Rd::new(bytes);
    let props = read_props(&mut r, &p.names)?;
    Ok((props, r.p))
}

/// A `UDataTable` export: its rows, name and properties.
pub fn data_table_rows(p: &Package, i: usize) -> Result<Vec<(String, Props)>> {
    let bytes = p.export_bytes(i)?;
    let mut r = Rd::new(bytes);
    read_props(&mut r, &p.names)?;
    r.i32()?;
    let count = r.count(8)?;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let name = read_name(&mut r, &p.names)?;
        rows.push((name, read_props(&mut r, &p.names)?));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::super::package::testing::Writer;
    use super::*;

    fn tag(w: &mut Writer, name: &str, kind: &str, size: i32) -> Vec<u8> {
        let mut t = w.fname(name);
        t.extend(w.fname(kind));
        t.extend(size.to_le_bytes());
        t.extend(0i32.to_le_bytes());
        t
    }

    fn props_bytes(w: &mut Writer) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(tag(w, "Speed", "FloatProperty", 4));
        b.push(0);
        b.extend(2.5f32.to_le_bytes());
        b.extend(tag(w, "Count", "IntProperty", 4));
        b.push(0);
        b.extend(7i32.to_le_bytes());
        b.extend(tag(w, "On", "BoolProperty", 0));
        b.push(1);
        b.push(0);
        b.extend(tag(w, "Tag", "NameProperty", 8));
        b.push(0);
        b.extend(w.fname("Value"));
        // A native struct.
        b.extend(tag(w, "At", "StructProperty", 12));
        b.extend(w.fname("Vector"));
        b.extend([0; 16]);
        b.push(0);
        for f in [1f32, 2., 3.] {
            b.extend(f.to_le_bytes());
        }
        // A float array.
        b.extend(tag(w, "List", "ArrayProperty", 12));
        b.extend(w.fname("FloatProperty"));
        b.push(0);
        b.extend(2i32.to_le_bytes());
        b.extend(4f32.to_le_bytes());
        b.extend(5f32.to_le_bytes());
        // An unknown kind is kept raw.
        b.extend(tag(w, "Odd", "TextProperty", 3));
        b.push(0);
        b.extend([9, 8, 7]);
        b.extend(w.fname("None"));
        b
    }

    #[test]
    fn tagged_values_are_read() {
        let mut w = Writer::default();
        let class = w.import_class("Thing");
        let body = props_bytes(&mut w);
        w.export(class, "Asset", body);
        let (head, uexp) = w.build();
        let p = Package::parse(&head, uexp, Vec::new()).unwrap();
        let (props, end) = export_props(&p, 0).unwrap();
        assert_eq!(end, p.export_bytes(0).unwrap().len());
        assert_eq!(props.f32("Speed"), Some(2.5));
        assert_eq!(props.int("Count"), Some(7));
        assert_eq!(props.bool("On"), Some(true));
        assert_eq!(props.str("Tag"), Some("Value"));
        assert_eq!(props.floats("At"), Some(&[1., 2., 3.][..]));
        assert_eq!(props.array("List").unwrap().len(), 2);
        assert_eq!(props.get("Odd"), Some(&Value::Raw(vec![9, 8, 7])));
    }

    #[test]
    fn data_table_rows_are_read() {
        let mut w = Writer::default();
        let class = w.import_class("DataTable");
        let mut b = w.fname("None");
        b.extend(0i32.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.extend(w.fname("Row"));
        b.extend(tag(&mut w, "Power", "IntProperty", 4));
        b.push(0);
        b.extend(3i32.to_le_bytes());
        b.extend(w.fname("None"));
        w.export(class, "Table", b);
        let (head, uexp) = w.build();
        let p = Package::parse(&head, uexp, Vec::new()).unwrap();
        let rows = data_table_rows(&p, 0).unwrap();
        assert_eq!(rows[0].0, "Row");
        assert_eq!(rows[0].1.int("Power"), Some(3));
    }
}
