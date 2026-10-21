//! Explicit normal-role source profile; general PNG import remains independent.
//! All inflater state, IDAT and output storage is retained; inflate does not allocate.
use super::*;
use miniz_oxide::{
    inflate::stream::{inflate, InflateState},
    DataFormat, MZFlush, MZStatus,
};
pub(super) const SOURCE: usize = 64 * 1024;
pub(super) const SIDE: usize = 257;
const ROWS: usize = (SIDE * 2 + 1) * SIDE;
const RAW: usize = SIDE * SIDE * 2;
pub(super) struct Decoder {
    state: Box<InflateState>,
    idat: Vec<u8>,
    filtered: Vec<u8>,
    raw: Vec<u8>,
}
impl Decoder {
    pub fn new() -> Result<Self, Error> {
        let mut idat = Vec::new();
        idat.try_reserve_exact(SOURCE).map_err(|_| Error::Limit)?;
        if idat.capacity() != SOURCE {
            return Err(Error::Limit);
        }
        idat.resize(SOURCE, 0xa5);
        std::hint::black_box(idat.iter().fold(0u64, |a, b| a + u64::from(*b)));
        idat.clear();
        let initialized = |count| -> Result<Vec<u8>, Error> {
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(count).map_err(|_| Error::Limit)?;
            if bytes.capacity() != count {
                return Err(Error::Limit);
            }
            bytes.resize(count, 0xa5);
            std::hint::black_box(bytes.iter().fold(0u64, |a, b| a + u64::from(*b)));
            Ok(bytes)
        };
        Ok(Self {
            state: InflateState::new_boxed(DataFormat::Zlib),
            idat,
            filtered: initialized(ROWS + 1)?,
            raw: initialized(RAW)?,
        })
    }
    pub fn identity(&self) -> [usize; 4] {
        [
            self.idat.as_ptr() as usize,
            self.state.as_ref() as *const InflateState as usize,
            self.filtered.as_ptr() as usize,
            self.raw.as_ptr() as usize,
        ]
    }
    pub fn backing_bytes(&self) -> usize {
        self.idat.capacity()
            + size_of::<InflateState>()
            + self.filtered.capacity()
            + self.raw.capacity()
    }
    pub fn decode<'a>(&'a mut self, bytes: &[u8], m: &Manifest) -> Result<&'a [u8], Error> {
        // Check before any compressed data is processed; never fall back to the general decoder.
        if bytes.len() > SOURCE
            || m.width as usize > SIDE
            || m.height as usize > SIDE
            || ![8, 16].contains(&m.bit_depth)
        {
            return Err(Error::Limit);
        }
        validate_manifest(m)?;
        preflight(bytes, m)?;
        if sha256(bytes) != m.source_sha256 {
            return Err(Error::Hash);
        }
        self.idat.clear();
        self.raw.fill(0);
        self.filtered.fill(0);
        let mut pos = 8;
        while pos < bytes.len() {
            let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().map_err(|_| Error::Png)?)
                as usize;
            let end = pos + 12 + len;
            let expected =
                u32::from_be_bytes(bytes[end - 4..end].try_into().map_err(|_| Error::Png)?);
            if crc32(&bytes[pos + 4..end - 4]) != expected {
                return Err(Error::Png);
            }
            if &bytes[pos + 4..pos + 8] == b"IDAT" {
                if self.idat.len() + len > SOURCE {
                    return Err(Error::Limit);
                }
                self.idat.extend_from_slice(&bytes[pos + 8..end - 4]);
            }
            pos = end;
        }
        let bpp = usize::from(m.bit_depth / 8);
        let stride = m.width as usize * bpp;
        let expected = (stride + 1) * m.height as usize;
        self.state.reset(DataFormat::Zlib);
        let result = inflate(
            &mut self.state,
            &self.idat,
            &mut self.filtered[..expected + 1],
            MZFlush::Finish,
        );
        if result.status != Ok(MZStatus::StreamEnd)
            || result.bytes_consumed != self.idat.len()
            || result.bytes_written != expected
        {
            return Err(Error::Png);
        }
        for y in 0..m.height as usize {
            let row = y * (stride + 1);
            let filter = self.filtered[row];
            if filter > 4 {
                return Err(Error::Png);
            }
            for x in 0..stride {
                let at = y * stride + x;
                let a = if x >= bpp { self.raw[at - bpp] } else { 0 };
                let b = if y > 0 { self.raw[at - stride] } else { 0 };
                let c = if y > 0 && x >= bpp {
                    self.raw[at - stride - bpp]
                } else {
                    0
                };
                let predictor = match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                    4 => paeth(a, b, c),
                    _ => unreachable!(),
                };
                self.raw[at] = self.filtered[row + 1 + x].wrapping_add(predictor);
            }
        }
        Ok(&self.raw[..stride * m.height as usize])
    }
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i32::from(a) + i32::from(b) - i32::from(c);
    let da = (p - i32::from(a)).abs();
    let db = (p - i32::from(b)).abs();
    let dc = (p - i32::from(c)).abs();
    if da <= db && da <= dc {
        a
    } else if db <= dc {
        b
    } else {
        c
    }
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32.wrapping_mul(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    fn file(raw: &[u8], trailing_zlib: bool, split: bool) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(raw).unwrap();
        let mut z = z.finish().unwrap();
        if trailing_zlib {
            z.extend_from_slice(&z.clone());
        }
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        chunk(
            &mut out,
            b"IHDR",
            &[0, 0, 0, 16, 0, 0, 0, 16, 8, 0, 0, 0, 0],
        );
        if split {
            let at = z.len() / 2;
            chunk(&mut out, b"IDAT", &z[..at]);
            chunk(&mut out, b"IDAT", &z[at..]);
        } else {
            chunk(&mut out, b"IDAT", &z);
        }
        chunk(&mut out, b"IEND", &[]);
        out
    }
    #[test]
    fn compressed_row_overflow_concatenated_stream_invalid_filter_and_split_chunks() {
        let input: crate::probe::Input = serde_json::from_slice(include_bytes!(
            "../../../../fixtures/m0-exchange-spatial.json"
        ))
        .unwrap();
        let mut m = input.terrain;
        m.width = 16;
        m.height = 16;
        m.bit_depth = 8;
        let mut decoder = Decoder::new().unwrap();
        let id = decoder.identity();
        let valid = file(&[0; 272], false, true);
        m.source_sha256 = sha256(&valid);
        assert_eq!(decoder.decode(&valid, &m).unwrap(), &[0; 256]);
        let mut invalid_filter = [0; 272];
        invalid_filter[0] = 5;
        // A valid PNG CRC must not hide a corrupt zlib Adler checksum.
        let mut adler = file(&[0; 272], false, false);
        let idat_len = u32::from_be_bytes(adler[33..37].try_into().unwrap()) as usize;
        let end = 33 + 12 + idat_len;
        adler[end - 5] ^= 1;
        let crc = crc32(&adler[37..end - 4]);
        adler[end - 4..end].copy_from_slice(&crc.to_be_bytes());
        for bad in [
            file(&[0; 273], false, false),
            file(&[0; 271], false, false),
            file(&[0; 272], true, false),
            file(&invalid_filter, false, false),
            adler,
        ] {
            m.source_sha256 = sha256(&bad);
            assert!(decoder.decode(&bad, &m).is_err());
            assert_eq!(decoder.identity(), id);
            m.source_sha256 = sha256(&valid);
            decoder.decode(&valid, &m).unwrap();
        }
    }
}
