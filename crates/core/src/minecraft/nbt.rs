//! Just enough NBT (Minecraft's binary tag format) to read `level.dat` and
//! read/write `servers.dat`.

use anyhow::{bail, Result};
use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// Element type id and items.
    List(u8, Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Tag::End => 0,
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn int(&self) -> Option<i64> {
        match *self {
            Tag::Byte(v) => Some(v as i64),
            Tag::Short(v) => Some(v as i64),
            Tag::Int(v) => Some(v as i64),
            Tag::Long(v) => Some(v),
            _ => None,
        }
    }
    pub fn list(&self) -> &[Tag] {
        match self {
            Tag::List(_, v) => v,
            _ => &[],
        }
    }
}

struct R<'a> {
    b: &'a [u8],
    i: usize,
    depth: u32,
}

impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.i + n > self.b.len() {
            bail!("NBT ends early");
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into()?))
    }
    fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.b.len() {
            bail!("bad NBT length");
        }
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String> {
        let n = self.i16()? as u16 as usize;
        // Java's "modified UTF-8" is plain UTF-8 for everything but NUL and
        // characters outside the BMP; lossy is fine for names.
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn payload(&mut self, id: u8) -> Result<Tag> {
        self.depth += 1;
        if self.depth > 64 {
            bail!("NBT nested too deep");
        }
        let t = match id {
            0 => Tag::End,
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(self.i16()?),
            3 => Tag::Int(self.i32()?),
            4 => Tag::Long(self.i64()?),
            5 => Tag::Float(f32::from_bits(self.i32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.i64()? as u64)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|b| *b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let el = self.u8()?;
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.payload(el)?);
                }
                Tag::List(el, v)
            }
            10 => {
                let mut v = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let k = self.string()?;
                    v.push((k, self.payload(t)?));
                }
                Tag::Compound(v)
            }
            11 => {
                let n = self.len()?;
                Tag::IntArray((0..n).map(|_| self.i32()).collect::<Result<_>>()?)
            }
            12 => {
                let n = self.len()?;
                Tag::LongArray((0..n).map(|_| self.i64()).collect::<Result<_>>()?)
            }
            _ => bail!("unknown NBT tag {id}"),
        };
        self.depth -= 1;
        Ok(t)
    }
}

/// Read a file's root tag, gzipped or not.
pub fn read(bytes: &[u8]) -> Result<Tag> {
    let raw;
    let b = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).take(64 << 20).read_to_end(&mut out)?;
        raw = out;
        &raw[..]
    } else {
        bytes
    };
    let mut r = R { b, i: 0, depth: 0 };
    let id = r.u8()?;
    if id != 10 {
        bail!("not an NBT file");
    }
    let _name = r.string()?;
    r.payload(10)
}

fn w_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    out.extend_from_slice(&(b.len().min(u16::MAX as usize) as u16).to_be_bytes());
    out.extend_from_slice(&b[..b.len().min(u16::MAX as usize)]);
}

fn w_payload(out: &mut Vec<u8>, t: &Tag) {
    match t {
        Tag::End => {}
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => w_str(out, s),
        Tag::List(el, v) => {
            out.push(if v.is_empty() { *el } else { v[0].id() });
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                w_payload(out, x);
            }
        }
        Tag::Compound(v) => {
            for (k, x) in v {
                out.push(x.id());
                w_str(out, k);
                w_payload(out, x);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

/// Write a root compound, uncompressed (as `servers.dat` is).
pub fn write(root: &Tag) -> Vec<u8> {
    let mut out = vec![10];
    w_str(&mut out, "");
    w_payload(&mut out, root);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips_and_reads_gzip() {
        let t = Tag::Compound(vec![
            ("servers".into(), Tag::List(10, vec![Tag::Compound(vec![("name".into(), Tag::String("Hypixel".into())), ("ip".into(), Tag::String("mc.hypixel.net".into()))])])),
            ("n".into(), Tag::Long(-5)),
            ("f".into(), Tag::Float(1.5)),
            ("a".into(), Tag::IntArray(vec![1, 2])),
        ]);
        let b = write(&t);
        assert_eq!(read(&b).unwrap(), t);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &b).unwrap();
        let g = read(&gz.finish().unwrap()).unwrap();
        assert_eq!(g.get("servers").unwrap().list()[0].get("ip").unwrap().str(), Some("mc.hypixel.net"));
        assert!(read(&[10, 0, 0, 1]).is_err());
        assert!(read(&[1, 2, 3]).is_err());
    }
}
