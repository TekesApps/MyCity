//! Large output backing independent of the tiny, explicitly bounded source profile.
use super::*;
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TerrainWorkspaceIdentity {
    pub grid_backing: [usize; 2],
    pub decoder_backing: [usize; 4],
    pub grid_capacity_bytes: usize,
    pub decoder_capacity_bytes: usize,
}
pub struct TerrainWorkspace {
    heights: Vec<i64>,
    water: Vec<u8>,
    decoder: small_png::Decoder,
    manifest: Option<Manifest>,
    revision: u64,
}
impl TerrainWorkspace {
    pub fn new() -> Result<Self, Error> {
        let count = MapSize::Large.vertices().pow(2);
        let mut heights = Vec::new();
        heights.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut water = Vec::new();
        water.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        if heights.capacity() != count || water.capacity() != count {
            return Err(Error::Limit);
        }
        heights.resize(count, i64::MIN);
        water.resize(count, 0xa5);
        std::hint::black_box(heights.iter().fold(0i64, |a, b| a.wrapping_add(*b)));
        std::hint::black_box(water.iter().fold(0u64, |a, b| a + u64::from(*b)));
        heights.clear();
        water.clear();
        Ok(Self {
            heights,
            water,
            decoder: small_png::Decoder::new()?,
            manifest: None,
            revision: 0,
        })
    }
    pub fn identity(&self) -> TerrainWorkspaceIdentity {
        TerrainWorkspaceIdentity {
            grid_backing: [self.heights.as_ptr() as usize, self.water.as_ptr() as usize],
            decoder_backing: self.decoder.identity(),
            grid_capacity_bytes: self.heights.capacity() * 8 + self.water.capacity(),
            decoder_capacity_bytes: self.decoder.backing_bytes(),
        }
    }
    pub fn load(&mut self, m: Manifest, bytes: &[u8], revision: u64) -> Result<(), Error> {
        self.manifest = None;
        self.revision = 0;
        self.heights.clear();
        self.water.clear();
        if revision == 0 {
            return Err(Error::Input);
        }
        let raw = self.decoder.decode(bytes, &m)?;
        if let Err(e) = resample(&m, raw, &mut self.heights, &mut self.water) {
            self.heights.clear();
            self.water.clear();
            return Err(e);
        }
        self.manifest = Some(m);
        self.revision = revision;
        Ok(())
    }
    pub fn into_terrain(self) -> Result<(Terrain, DecoderReserve), Error> {
        let manifest = self.manifest.ok_or(Error::Stale)?;
        let terrain = Terrain {
            manifest,
            heights: self.heights,
            water: self.water,
            revision: self.revision,
        };
        if terrain.owned_capacity_bytes() > TERRAIN_RESIDENT_BUDGET_BYTES {
            return Err(Error::Limit);
        }
        Ok((terrain, DecoderReserve(self.decoder)))
    }
}
/// Keep the retained decoder backing alive with the role after import.
pub struct DecoderReserve(small_png::Decoder);
impl DecoderReserve {
    pub fn identity(&self) -> ([usize; 4], usize) {
        (self.0.identity(), self.0.backing_bytes())
    }
    /// Includes pointer headers as well as retained decoder/raw/filter heap backing.
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.0.backing_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        let input: crate::probe::Input = serde_json::from_slice(include_bytes!(
            "../../../../fixtures/m0-exchange-spatial.json"
        ))
        .unwrap();
        input.terrain
    }
    fn png(side: u32, bits: u8, filter: png::FilterType) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, side, side);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(if bits == 8 {
                png::BitDepth::Eight
            } else {
                png::BitDepth::Sixteen
            });
            enc.set_filter(filter);
            enc.set_compression(png::Compression::Best);
            let mut writer = enc.write_header().unwrap();
            let bytes = (0..side * side * u32::from(bits / 8))
                .map(|n| n.wrapping_mul(137).wrapping_add(n / 3) as u8)
                .collect::<Vec<_>>();
            writer.write_image_data(&bytes).unwrap();
        }
        out
    }
    #[test]
    fn both_depths_every_filter_and_all_output_sizes_match_general_decoder_and_reuse_large_backing()
    {
        let mut slot = TerrainWorkspace::new().unwrap();
        let identity = slot.identity();
        for side in [2, 16, 257] {
            for bits in [8, 16] {
                for filter in [
                    png::FilterType::NoFilter,
                    png::FilterType::Sub,
                    png::FilterType::Up,
                    png::FilterType::Avg,
                    png::FilterType::Paeth,
                ] {
                    let bytes = png(side, bits, filter);
                    let mut m = manifest();
                    m.bit_depth = bits;
                    m.width = side;
                    m.height = side;
                    m.source_sha256 = sha256(&bytes);
                    m.z_min_cm = -500;
                    m.z_max_cm = 500;
                    m.water = Water::WetRects {
                        rects: vec![PixelRect {
                            x0: 0,
                            y0: 0,
                            x1: side / 2,
                            y1: side / 2 + 1,
                        }],
                    };
                    for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
                        m.size = size;
                        let old = import(m.clone(), &bytes, 1, 0).unwrap();
                        slot.load(m.clone(), &bytes, 1).unwrap_or_else(|e| panic!("side={side} bits={bits} filter={filter:?} compressed={} error={e:?}", bytes.len()));
                        assert_eq!(slot.heights, old.heights);
                        assert_eq!(slot.water, old.water);
                        assert_eq!(slot.identity(), identity);
                        assert_eq!(slot.heights.capacity(), MapSize::Large.vertices().pow(2));
                    }
                }
            }
        }
        let (terrain, decoder) = slot.into_terrain().unwrap();
        assert_eq!(
            [
                terrain.heights.as_ptr() as usize,
                terrain.water.as_ptr() as usize
            ],
            identity.grid_backing
        );
        assert_eq!(
            decoder.identity(),
            (identity.decoder_backing, identity.decoder_capacity_bytes)
        );
    }
    #[test]
    fn original_frozen_207_byte_source_matches_general_import_for_all_output_maps() {
        let bytes = include_bytes!("../../../../fixtures/terrain/m0-flat16.png");
        let mut m = manifest();
        assert_eq!((m.width, m.height, m.bit_depth), (257, 257, 16));
        assert_eq!(bytes.len(), 207);
        let mut slot = TerrainWorkspace::new().unwrap();
        let id = slot.identity();
        for size in [MapSize::Small, MapSize::Medium, MapSize::Large] {
            m.size = size;
            let old = import(m.clone(), bytes, 1, 0).unwrap();
            slot.load(m.clone(), bytes, 1).unwrap();
            assert_eq!(slot.heights, old.heights);
            assert_eq!(slot.water, old.water);
            assert_eq!(slot.identity(), id);
        }
        let (new, decoder) = slot.into_terrain().unwrap();
        assert_eq!(
            new.state_hash(),
            import(m, bytes, 1, 0).unwrap().state_hash()
        );
        assert!(decoder.owned_capacity_bytes() > id.decoder_capacity_bytes);
    }
    #[test]
    fn high_entropy_source_over_profile_limit_is_rejected_without_changing_general_import() {
        let mut seed = 7u64;
        let raw = (0..257 * 257 * 2)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed as u8
            })
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut bytes, 257, 257);
            enc.set_depth(png::BitDepth::Sixteen);
            enc.set_color(png::ColorType::Grayscale);
            enc.write_header().unwrap().write_image_data(&raw).unwrap();
        }
        assert!(bytes.len() > small_png::SOURCE);
        let mut m = manifest();
        m.source_sha256 = sha256(&bytes);
        assert!(import(m.clone(), &bytes, 1, 0).is_ok());
        let mut slot = TerrainWorkspace::new().unwrap();
        let id = slot.identity();
        assert_eq!(slot.load(m, &bytes, 1), Err(Error::Limit));
        assert_eq!(slot.identity(), id);
        assert!(slot.into_terrain().is_err());
    }
    #[test]
    fn rejected_source_never_exposes_old_candidate_and_same_backing_accepts_next_legal_source() {
        let bytes = png(16, 8, png::FilterType::Paeth);
        let mut m = manifest();
        m.width = 16;
        m.height = 16;
        m.bit_depth = 8;
        m.source_sha256 = sha256(&bytes);
        let mut slot = TerrainWorkspace::new().unwrap();
        let id = slot.identity();
        slot.load(m.clone(), &bytes, 1).unwrap();
        let mut invalid = Vec::new();
        let mut crc = bytes.clone();
        crc[29] ^= 1;
        invalid.push(crc);
        invalid.push(bytes[..bytes.len() - 1].to_vec());
        invalid.push(vec![0; small_png::SOURCE + 1]);
        for bad in invalid {
            let mut bad_m = m.clone();
            bad_m.source_sha256 = sha256(&bad);
            assert!(slot.load(bad_m, &bad, 1).is_err());
            assert!(slot.manifest.is_none());
            assert_eq!(slot.identity(), id);
            slot.load(m.clone(), &bytes, 1).unwrap();
            assert_eq!(slot.identity(), id);
        }
        let large_source = png(258, 8, png::FilterType::NoFilter);
        let mut general = m.clone();
        general.width = 258;
        general.height = 258;
        general.source_sha256 = sha256(&large_source);
        assert!(import(general.clone(), &large_source, 1, 0).is_ok());
        assert!(slot.load(general, &large_source, 1).is_err());
        assert!(slot.manifest.is_none());
        assert_eq!(slot.identity(), id);
        slot.load(m, &bytes, 1).unwrap();
    }
}
