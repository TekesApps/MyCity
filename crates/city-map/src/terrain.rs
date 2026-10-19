//! Raw grayscale samples, bilinear resampling, then one immutable triangulated surface.
use crate::{
    round_ratio, Error, MapSize, Point, Rect, GEOMETRY_BUDGET_BYTES, IMPORT_PEAK_BUDGET_BYTES,
    TERRAIN_RESIDENT_BUDGET_BYTES, TERRAIN_STEP_CM, Z_LIMIT_CM,
};
use mycity_core::sha256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read},
    mem::size_of,
};
pub const SOURCE_BYTE_LIMIT: u64 = 16 * 1024 * 1024;
pub const DECODER_BYTE_LIMIT: usize = 40 * 1024 * 1024;
pub const MAX_SOURCE_SIDE: u32 = 4097;
pub const MAX_MASK_RECTS: usize = 32;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}
impl PixelRect {
    fn validate(self, w: u32, h: u32) -> Result<(), Error> {
        if self.x0 >= self.x1 || self.y0 >= self.y1 || self.x1 > w || self.y1 > h {
            return Err(Error::Input);
        }
        Ok(())
    }
    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Validity {
    AllValid,
    InvalidRects { rects: Vec<PixelRect> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Water {
    AllDry,
    AllWet,
    WetRects { rects: Vec<PixelRect> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub size: MapSize,
    pub sample_semantics: String,
    pub row0: String,
    pub z_min_cm: i64,
    pub z_max_cm: i64,
    pub vertical_datum: String,
    pub source_resolution_cm: u32,
    pub source_accuracy_cm: Option<u32>,
    pub synthetic: bool,
    pub georeferencing: String,
    pub processing_version: String,
    pub license: String,
    pub validity: Validity,
    pub water: Water,
}
#[derive(Debug)]
pub struct Terrain {
    manifest: Manifest,
    heights: Vec<i64>,
    water: Vec<u8>,
    revision: u64,
}
impl Terrain {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn heights(&self) -> &[i64] {
        &self.heights
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.heights.capacity() * size_of::<i64>()
            + self.water.capacity()
            + self.manifest.schema.capacity()
            + self.manifest.source_sha256.capacity()
            + self.manifest.sample_semantics.capacity()
            + self.manifest.row0.capacity()
            + self.manifest.vertical_datum.capacity()
            + self.manifest.georeferencing.capacity()
            + self.manifest.processing_version.capacity()
            + self.manifest.license.capacity()
            + match &self.manifest.validity {
                Validity::AllValid => 0,
                Validity::InvalidRects { rects } => rects.capacity() * size_of::<PixelRect>(),
            }
            + match &self.manifest.water {
                Water::WetRects { rects } => rects.capacity() * size_of::<PixelRect>(),
                _ => 0,
            }
    }
    pub fn state_hash(&self) -> String {
        // Canonical order and fixed endian quantities; no geometry or source bytes omitted.
        let mut data = Sha256::new();
        data.update(serde_json::to_vec(&self.manifest).expect("finite manifest"));
        data.update(self.revision.to_le_bytes());
        for z in &self.heights {
            data.update(z.to_le_bytes());
        }
        data.update(&self.water);
        format!("{:x}", data.finalize())
    }
    pub fn height_cm(&self, local: Point) -> Result<i64, Error> {
        let side = self.manifest.size.side_cm();
        if local.x < 0 || local.y < 0 || local.x > side || local.y > side {
            return Err(Error::Bound);
        }
        let n = self.manifest.size.vertices();
        let x = (local.x / TERRAIN_STEP_CM).min(n as i64 - 2) as usize;
        let y = (local.y / TERRAIN_STEP_CM).min(n as i64 - 2) as usize;
        let dx = i128::from(local.x - x as i64 * TERRAIN_STEP_CM);
        let dy = i128::from(local.y - y as i64 * TERRAIN_STEP_CM);
        let s = i128::from(TERRAIN_STEP_CM);
        let sw = i128::from(self.heights[y * n + x]);
        let se = i128::from(self.heights[y * n + x + 1]);
        let nw = i128::from(self.heights[(y + 1) * n + x]);
        let ne = i128::from(self.heights[(y + 1) * n + x + 1]);
        // Fixed SW--NE diagonal. Not bilinear interpolation at runtime.
        let numerator = if dx >= dy {
            sw * (s - dx) + se * (dx - dy) + ne * dy
        } else {
            sw * (s - dy) + nw * (dy - dx) + ne * dx
        };
        round_ratio(numerator, s)
    }
    pub fn water_at_vertex(&self, x: usize, y: usize) -> Result<bool, Error> {
        let n = self.manifest.size.vertices();
        if x >= n || y >= n {
            return Err(Error::Bound);
        }
        Ok(self.water[y * n + x] != 0)
    }
    /// Conservative proof over all intersecting cells, including their vertices.
    /// Extra surrounding vertices may reject a valid near-boundary platform; never undercheck it.
    pub fn flat_platform(&self, rect: Rect, z_cm: i64) -> Result<(), Error> {
        rect.validate()?;
        let side = self.manifest.size.side_cm();
        if rect.min.x < 0 || rect.min.y < 0 || rect.max.x > side || rect.max.y > side {
            return Err(Error::Bound);
        }
        if let Water::WetRects { rects } = &self.manifest.water {
            // Keep narrow known water regions even when nearest-vertex resampling misses them.
            let sw = i64::from(self.manifest.width - 1);
            let sh = i64::from(self.manifest.height - 1);
            for r in rects {
                let x0 = i64::from(r.x0.saturating_sub(1)) * side / sw;
                let x1 = (i64::from(r.x1.min(self.manifest.width - 1)) * side + sw - 1) / sw;
                let y0 = (sh - i64::from(r.y1.min(self.manifest.height - 1))) * side / sh;
                let y1 = ((sh - i64::from(r.y0.saturating_sub(1))) * side + sh - 1) / sh;
                if rect.min.x <= x1 && rect.max.x >= x0 && rect.min.y <= y1 && rect.max.y >= y0 {
                    return Err(Error::NeedsDetailedValidation);
                }
            }
        }
        let n = self.manifest.size.vertices();
        let x0 = (rect.min.x / TERRAIN_STEP_CM) as usize;
        let y0 = (rect.min.y / TERRAIN_STEP_CM) as usize;
        let x1 = ((rect.max.x + TERRAIN_STEP_CM - 1) / TERRAIN_STEP_CM) as usize;
        let y1 = ((rect.max.y + TERRAIN_STEP_CM - 1) / TERRAIN_STEP_CM) as usize;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.heights[y * n + x] != z_cm || self.water[y * n + x] != 0 {
                    return Err(Error::NeedsDetailedValidation);
                }
            }
        }
        Ok(())
    }
}
fn validate_manifest(m: &Manifest) -> Result<(), Error> {
    if m.schema != "mycity-heightmap-v1"
        || !(2..=MAX_SOURCE_SIDE).contains(&m.width)
        || !(2..=MAX_SOURCE_SIDE).contains(&m.height)
        || m.width != m.height
        || ![8, 16].contains(&m.bit_depth)
        || m.z_min_cm < -Z_LIMIT_CM
        || m.z_max_cm > Z_LIMIT_CM
        || m.z_min_cm > m.z_max_cm
        || m.source_resolution_cm == 0
        || m.source_sha256.len() != 64
        || !m
            .source_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || m.vertical_datum.is_empty()
        || m.license.is_empty()
    {
        return Err(Error::Input);
    }
    if m.sample_semantics != "vertices"
        || m.row0 != "north"
        || m.georeferencing != "local_metric"
        || m.processing_version != "bilinear-cm-swne-v1"
    {
        return Err(Error::Unsupported);
    }
    for s in [
        &m.schema,
        &m.source_sha256,
        &m.sample_semantics,
        &m.row0,
        &m.vertical_datum,
        &m.georeferencing,
        &m.processing_version,
        &m.license,
    ] {
        if s.len() > 1024 {
            return Err(Error::Limit);
        }
    }
    if let Validity::InvalidRects { rects } = &m.validity {
        if rects.len() > MAX_MASK_RECTS {
            return Err(Error::Limit);
        }
        for r in rects {
            r.validate(m.width, m.height)?;
        }
        if !rects.is_empty() {
            return Err(Error::NoData);
        }
    }
    if let Water::WetRects { rects } = &m.water {
        if rects.len() > MAX_MASK_RECTS {
            return Err(Error::Limit);
        }
        for r in rects {
            r.validate(m.width, m.height)?;
        }
    }
    Ok(())
}
/// Structural allow-list before entering the decoder. Reject animation/transparency/metadata.
/// This narrow native format accepts only IHDR, consecutive IDAT, IEND; external conversion
/// is explicit. CRCs and compressed-stream validity are still checked by the PNG decoder.
fn preflight(bytes: &[u8], m: &Manifest) -> Result<(), Error> {
    if bytes.len() as u64 > SOURCE_BYTE_LIMIT {
        return Err(Error::Limit);
    }
    if bytes.get(..8) != Some(&b"\x89PNG\r\n\x1a\n"[..]) {
        return Err(Error::Png);
    }
    let mut pos = 8usize;
    let mut seen_header = false;
    let mut seen_data = false;
    while pos < bytes.len() {
        let header = bytes.get(pos..pos + 8).ok_or(Error::Png)?;
        let len = u32::from_be_bytes(header[..4].try_into().map_err(|_| Error::Png)?) as usize;
        let end = pos
            .checked_add(12)
            .and_then(|p| p.checked_add(len))
            .ok_or(Error::Limit)?;
        let chunk = bytes.get(pos + 8..end - 4).ok_or(Error::Png)?;
        if end > bytes.len() {
            return Err(Error::Png);
        }
        match &header[4..8] {
            b"IHDR" if !seen_header && pos == 8 && len == 13 => {
                let w = u32::from_be_bytes(chunk[..4].try_into().map_err(|_| Error::Png)?);
                let h = u32::from_be_bytes(chunk[4..8].try_into().map_err(|_| Error::Png)?);
                if w != m.width || h != m.height || chunk[8] != m.bit_depth {
                    return Err(Error::Input);
                }
                if chunk[9] != 0 || chunk[10] != 0 || chunk[11] != 0 || chunk[12] != 0 {
                    return Err(Error::Unsupported);
                }
                seen_header = true;
            }
            b"IDAT" if seen_header => {
                seen_data = true;
            }
            b"IEND" if seen_data && len == 0 && end == bytes.len() => return Ok(()),
            _ => return Err(Error::Unsupported),
        }
        pos = end;
    }
    Err(Error::Png)
}
// png's frame reader can ignore excess decompressed rows. Verify the complete zlib
// stream separately with a fixed output window, including checksum and trailing input.
fn strict_inflate(bytes: &[u8], m: &Manifest) -> Result<(), Error> {
    let mut idat = Vec::with_capacity(bytes.len());
    let mut pos = 8usize;
    while pos < bytes.len() {
        let len =
            u32::from_be_bytes(bytes[pos..pos + 4].try_into().map_err(|_| Error::Png)?) as usize;
        if &bytes[pos + 4..pos + 8] == b"IDAT" {
            idat.extend_from_slice(&bytes[pos + 8..pos + 8 + len]);
        }
        pos += len + 12;
    }
    let expected = (u64::from(m.width) * u64::from(m.bit_depth / 8) + 1) * u64::from(m.height);
    let mut decoder = flate2::read::ZlibDecoder::new(idat.as_slice());
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let remaining = usize::try_from((expected + 1 - total).min(buffer.len() as u64))
            .map_err(|_| Error::Limit)?;
        let count = decoder
            .read(&mut buffer[..remaining])
            .map_err(|_| Error::Png)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > expected {
            return Err(Error::Png);
        }
    }
    if total != expected || decoder.total_in() != idat.len() as u64 {
        return Err(Error::Png);
    }
    Ok(())
}
pub fn import(
    m: Manifest,
    bytes: &[u8],
    revision: u64,
    old_owned_bytes: usize,
) -> Result<Terrain, Error> {
    validate_manifest(&m)?;
    if revision == 0 {
        return Err(Error::Input);
    }
    preflight(bytes, &m)?;
    if sha256(bytes) != m.source_sha256 {
        return Err(Error::Hash);
    }
    let decoded = m.width as usize * m.height as usize * usize::from(m.bit_depth / 8);
    let count = m.size.vertices() * m.size.vertices();
    let peak = bytes
        .len()
        .checked_add(bytes.len())
        .and_then(|n| n.checked_add(decoded))
        .and_then(|n| n.checked_add(DECODER_BYTE_LIMIT))
        .and_then(|n| n.checked_add(count * 9))
        .and_then(|n| n.checked_add(old_owned_bytes))
        .and_then(|n| n.checked_add(2 * GEOMETRY_BUDGET_BYTES))
        .and_then(|n| n.checked_add(256 * 1024))
        .ok_or(Error::Overflow)?;
    if peak > IMPORT_PEAK_BUDGET_BYTES {
        return Err(Error::Limit);
    }
    strict_inflate(bytes, &m)?;
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: DECODER_BYTE_LIMIT,
        },
    );
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|_| Error::Png)?;
    if reader.output_buffer_size() != decoded {
        return Err(Error::Png);
    }
    let mut raw = vec![0; decoded];
    let info = reader.next_frame(&mut raw).map_err(|_| Error::Png)?;
    if info.buffer_size() != decoded {
        return Err(Error::Png);
    }
    reader.finish().map_err(|_| Error::Png)?;
    let n = m.size.vertices();
    let den = (n - 1) as u64;
    let max_gray = if m.bit_depth == 8 { 255i128 } else { 65535 };
    let gray = |x: usize, y: usize| -> i128 {
        let i = (y * m.width as usize + x) * usize::from(m.bit_depth / 8);
        if m.bit_depth == 8 {
            i128::from(raw[i])
        } else {
            i128::from(u16::from_be_bytes([raw[i], raw[i + 1]]))
        }
    };
    let mut heights = Vec::with_capacity(count);
    let mut water = Vec::with_capacity(count);
    for y in 0..n {
        for x in 0..n {
            // Internal grid row 0 is south; PNG row 0 is north.
            let sx = x as u64 * u64::from(m.width - 1);
            let sy = (n - 1 - y) as u64 * u64::from(m.height - 1);
            let ix = (sx / den) as usize;
            let iy = (sy / den) as usize;
            let jx = (ix + 1).min(m.width as usize - 1);
            let jy = (iy + 1).min(m.height as usize - 1);
            let fx = i128::from(sx % den);
            let fy = i128::from(sy % den);
            let d = i128::from(den);
            let value = gray(ix, iy) * (d - fx) * (d - fy)
                + gray(jx, iy) * fx * (d - fy)
                + gray(ix, jy) * (d - fx) * fy
                + gray(jx, jy) * fx * fy;
            let denominator = d * d * max_gray;
            let z = round_ratio(
                i128::from(m.z_min_cm) * denominator + value * i128::from(m.z_max_cm - m.z_min_cm),
                denominator,
            )?;
            heights.push(z);
            let wx = ((sx + den / 2) / den) as u32;
            let wy = ((sy + den / 2) / den) as u32;
            water.push(u8::from(match &m.water {
                Water::AllDry => false,
                Water::AllWet => true,
                Water::WetRects { rects } => rects.iter().any(|r| r.contains(wx, wy)),
            }));
        }
    }
    let terrain = Terrain {
        manifest: m,
        heights,
        water,
        revision,
    };
    if terrain.owned_capacity_bytes() > TERRAIN_RESIDENT_BUDGET_BYTES {
        return Err(Error::Limit);
    }
    Ok(terrain)
}
#[derive(Debug, Serialize)]
pub struct ImportBudget {
    pub input_bytes: usize,
    pub compressed_copy_allowance_bytes: usize,
    pub decoded_bytes: usize,
    pub decoder_limit_bytes: usize,
    pub target_payload_bytes: usize,
    pub old_owned_bytes: usize,
    pub geometry_pair_allowance_bytes: usize,
    pub scratch_allowance_bytes: usize,
    pub planned_peak_bytes: usize,
}
pub fn budget(
    m: &Manifest,
    input_bytes: usize,
    old_owned_bytes: usize,
) -> Result<ImportBudget, Error> {
    validate_manifest(m)?;
    let decoded = m.width as usize * m.height as usize * usize::from(m.bit_depth / 8);
    let target = m.size.vertices() * m.size.vertices() * 9;
    let peak = input_bytes
        .checked_add(input_bytes)
        .and_then(|n| n.checked_add(decoded))
        .and_then(|n| n.checked_add(DECODER_BYTE_LIMIT))
        .and_then(|n| n.checked_add(target))
        .and_then(|n| n.checked_add(old_owned_bytes))
        .and_then(|n| n.checked_add(2 * GEOMETRY_BUDGET_BYTES))
        .and_then(|n| n.checked_add(256 * 1024))
        .ok_or(Error::Overflow)?;
    Ok(ImportBudget {
        input_bytes,
        compressed_copy_allowance_bytes: input_bytes,
        decoded_bytes: decoded,
        decoder_limit_bytes: DECODER_BYTE_LIMIT,
        target_payload_bytes: target,
        old_owned_bytes,
        geometry_pair_allowance_bytes: 2 * GEOMETRY_BUDGET_BYTES,
        scratch_allowance_bytes: 256 * 1024,
        planned_peak_bytes: peak,
    })
}
