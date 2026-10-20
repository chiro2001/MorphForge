//! Bounded big-endian Java NBT reader. Keeps raw source separately; no executable
//! Minecraft content is interpreted. Limits apply before allocating arrays.
use anyhow::{bail, Context, Result};
use std::{collections::BTreeMap, io::Read};
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_NODES: usize = 4_194_304;
const MAX_DEPTH: usize = 64;
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Tag>),
    Compound(BTreeMap<String, Tag>),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
}
impl Tag {
    pub fn get(&self, name: &str) -> Option<&Tag> {
        if let Self::Compound(map) = self {
            map.get(name)
        } else {
            None
        }
    }
    pub fn compound(&self) -> Result<&BTreeMap<String, Tag>> {
        if let Self::Compound(v) = self {
            Ok(v)
        } else {
            bail!("NBT需要Compound")
        }
    }
    pub fn list(&self) -> Result<&[Tag]> {
        if let Self::List(v) = self {
            Ok(v)
        } else {
            bail!("NBT需要List")
        }
    }
    pub fn string(&self) -> Result<&str> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            bail!("NBT需要String")
        }
    }
    pub fn int(&self) -> Result<i64> {
        match self {
            Self::Byte(v) => Ok(*v as i64),
            Self::Short(v) => Ok(*v as i64),
            Self::Int(v) => Ok(*v as i64),
            Self::Long(v) => Ok(*v),
            _ => bail!("NBT需要整数"),
        }
    }
    pub fn bytes(&self) -> Result<&[u8]> {
        if let Self::Bytes(v) = self {
            Ok(v)
        } else {
            bail!("NBT需要ByteArray")
        }
    }
    pub fn longs(&self) -> Result<&[i64]> {
        if let Self::Longs(v) = self {
            Ok(v)
        } else {
            bail!("NBT需要LongArray")
        }
    }
    pub fn vector(&self) -> Result<[i32; 3]> {
        let values: Vec<i64> = match self {
            Self::Ints(v) => v.iter().map(|n| *n as i64).collect(),
            Self::List(v) => v.iter().map(Self::int).collect::<Result<_>>()?,
            Self::Compound(v) => ["x", "y", "z"]
                .iter()
                .map(|k| v.get(*k).context("NBT坐标缺少分量")?.int())
                .collect::<Result<_>>()?,
            _ => bail!("NBT需要三维整数坐标"),
        };
        if values.len() != 3 {
            bail!("NBT坐标需要三个分量");
        }
        Ok([
            i32::try_from(values[0])?,
            i32::try_from(values[1])?,
            i32::try_from(values[2])?,
        ])
    }
    pub fn required(&self, name: &str) -> Result<&Tag> {
        self.get(name).with_context(|| format!("NBT缺少{name}"))
    }
}
struct Reader<'a> {
    data: &'a [u8],
    cursor: usize,
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.cursor.checked_add(n).context("NBT长度溢出")?;
        let data = self.data.get(self.cursor..end).context("NBT数据截断")?;
        self.cursor = end;
        Ok(data)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn int(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn length(&mut self, width: usize) -> Result<usize> {
        let length = usize::try_from(self.int()?).context("NBT数组长度不能为负")?;
        let bytes = length.checked_mul(width).context("NBT数组长度溢出")?;
        if bytes > MAX_BYTES || bytes > self.data.len() - self.cursor {
            bail!("NBT数组长度超过数据或保护值");
        }
        Ok(length)
    }
    fn string(&mut self) -> Result<String> {
        let size = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        modified_utf8(self.take(size)?)
    }
    fn value(&mut self, kind: u8, depth: usize) -> Result<Tag> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            bail!("NBT嵌套深度或节点数超过保护值");
        }
        self.nodes += 1;
        Ok(match kind {
            1 => Tag::Byte(self.byte()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.take(2)?.try_into()?)),
            3 => Tag::Int(self.int()?),
            4 => Tag::Long(i64::from_be_bytes(self.take(8)?.try_into()?)),
            5 => Tag::Float(f32::from_be_bytes(self.take(4)?.try_into()?)),
            6 => Tag::Double(f64::from_be_bytes(self.take(8)?.try_into()?)),
            7 => {
                let n = self.length(1)?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let element = self.byte()?;
                let n = self.length(1)?;
                if n > MAX_NODES - self.nodes || (element == 0 && n != 0) || element > 12 {
                    bail!("NBT List类型或长度无效");
                }
                let mut out = Vec::with_capacity(n.min(65536));
                for _ in 0..n {
                    out.push(self.value(element, depth + 1)?);
                }
                Tag::List(out)
            }
            10 => {
                let mut out = BTreeMap::new();
                loop {
                    let kind = self.byte()?;
                    if kind == 0 {
                        break;
                    }
                    let key = self.string()?;
                    let value = self.value(kind, depth + 1)?;
                    if out.insert(key, value).is_some() {
                        bail!("NBT包含重复字段");
                    }
                }
                Tag::Compound(out)
            }
            11 => {
                let n = self.length(4)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(self.int()?);
                }
                Tag::Ints(out)
            }
            12 => {
                let n = self.length(8)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(i64::from_be_bytes(self.take(8)?.try_into()?));
                }
                Tag::Longs(out)
            }
            _ => bail!("NBT未知标签类型{kind}"),
        })
    }
}
fn modified_utf8(bytes: &[u8]) -> Result<String> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.into());
    }
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let (code, width) = if b < 128 {
            (b as u32, 1)
        } else if b & 0xe0 == 0xc0 {
            let next = *bytes.get(i + 1).context("NBT UTF8截断")?;
            if next & 0xc0 != 0x80 {
                bail!("NBT UTF8无效");
            }
            let code = ((b & 31) as u32) << 6 | (next & 63) as u32;
            if code < 128 && !(b == 0xc0 && next == 0x80) {
                bail!("NBT UTF8过长编码");
            }
            (code, 2)
        } else if b & 0xf0 == 0xe0 {
            let next = bytes.get(i + 1..i + 3).context("NBT UTF8截断")?;
            if next.iter().any(|b| b & 0xc0 != 0x80) {
                bail!("NBT UTF8无效");
            }
            let code =
                ((b & 15) as u32) << 12 | ((next[0] & 63) as u32) << 6 | (next[1] & 63) as u32;
            if code < 2048 {
                bail!("NBT UTF8过长编码");
            }
            (code, 3)
        } else if b & 0xf8 == 0xf0 {
            let next = bytes.get(i + 1..i + 4).context("NBT UTF8截断")?;
            if next.iter().any(|b| b & 0xc0 != 0x80) {
                bail!("NBT UTF8无效");
            }
            let code = ((b & 7) as u32) << 18
                | ((next[0] & 63) as u32) << 12
                | ((next[1] & 63) as u32) << 6
                | (next[2] & 63) as u32;
            if !(0x10000..=0x10ffff).contains(&code) {
                bail!("NBT UTF8码点越界");
            }
            (code, 4)
        } else {
            bail!("NBT UTF8无效");
        };
        if code > 65535 {
            let value = code - 65536;
            units.push(0xd800 + (value >> 10) as u16);
            units.push(0xdc00 + (value & 1023) as u16);
        } else {
            units.push(code as u16);
        }
        i += width;
    }
    Ok(String::from_utf16(&units).context("NBT UTF16代理对无效")?)
}
pub fn read(bytes: &[u8]) -> Result<Tag> {
    if bytes.len() > 64 * 1024 * 1024 {
        bail!("建筑压缩文件超过64MiB");
    }
    let mut decompressed = Vec::new();
    let data = if bytes.starts_with(&[0x1f, 0x8b]) {
        flate2::read::MultiGzDecoder::new(bytes)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut decompressed)
            .context("建筑GZip无效")?;
        &decompressed[..]
    } else if bytes.len() > 2
        && bytes[0] & 15 == 8
        && u16::from_be_bytes([bytes[0], bytes[1]]) % 31 == 0
    {
        flate2::read::ZlibDecoder::new(bytes)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut decompressed)
            .context("建筑Zlib无效")?;
        &decompressed[..]
    } else {
        bytes
    };
    if data.len() > MAX_BYTES {
        bail!("建筑解压后超过128MiB");
    }
    let mut reader = Reader {
        data,
        cursor: 0,
        nodes: 0,
    };
    if reader.byte()? != 10 {
        bail!("需要Java NBT Compound建筑文件；暂不支持基岩mcstructure");
    }
    reader.string()?;
    let value = reader.value(10, 0)?;
    if reader.cursor != data.len() {
        bail!("NBT末尾存在额外数据");
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn big_endian_and_modified_utf8_are_exact() {
        let bytes = [10, 0, 0, 3, 0, 1, b'x', 0, 0, 1, 0, 0];
        assert_eq!(read(&bytes).unwrap().get("x").unwrap().int().unwrap(), 256);
        assert_eq!(modified_utf8(&[0xc0, 0x80]).unwrap(), "\0");
        assert_eq!(
            modified_utf8(&[0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80]).unwrap(),
            "😀"
        );
    }
    #[test]
    fn negative_lengths_truncation_and_depth_are_rejected() {
        assert!(read(&[10, 0, 0, 7, 0, 1, b'x', 255, 255, 255, 255]).is_err());
        assert!(read(&[10, 0, 0, 12, 0, 1, b'x', 0, 0, 0, 2, 0]).is_err());
        let mut bytes = vec![10, 0, 0];
        for _ in 0..70 {
            bytes.extend([10, 0, 0]);
        }
        bytes.extend([0; 71]);
        assert!(read(&bytes).is_err());
        assert!(modified_utf8(&[0xed, 0xa0, 0xbd]).is_err());
    }
    #[test]
    fn gzip_crc_is_checked_and_trailing_nbt_rejected() {
        use std::io::Write;
        let bytes = [10, 0, 0, 0];
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&bytes).unwrap();
        let mut gz = encoder.finish().unwrap();
        assert!(read(&gz).is_ok());
        let n = gz.len();
        gz[n - 5] ^= 1;
        assert!(read(&gz).is_err());
        assert!(read(&[10, 0, 0, 0, 0]).is_err());
    }
}
