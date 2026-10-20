//! Fixed helper control protocol storage. Model images/Engine work are separate.
use serde::{
    de::{self, DeserializeOwned, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    cell::Cell,
    fmt,
    fs::File,
    io::Read,
    ops::Deref,
    path::{Path, PathBuf},
};
pub const CONTROL_BYTES: usize = 8192;
pub const HELPER_CAP: usize = 256 * 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Text<N> {
    pub fn new(value: &str) -> Result<Self, String> {
        if value.len() > N {
            return Err("model_control_text_bound".into());
        }
        let mut out = Self {
            bytes: [0; N],
            len: value.len(),
        };
        out.bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(out)
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("validated UTF-8")
    }
}
impl<const N: usize> fmt::Debug for Text<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
impl<const N: usize> Serialize for Text<N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
impl<'de, const N: usize> Deserialize<'de> for Text<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<const N: usize>;
        impl<const N: usize> Visitor<'_> for V<N> {
            type Value = Text<N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "text at most {N} UTF-8 bytes")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Text::new(v).map_err(|_| E::custom("model_control_text_bound"))
            }
        }
        d.deserialize_str(V::<N>)
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub control_peak_owners: usize,
    pub path_peak_owners: usize,
    pub golden_peak_owners: usize,
    pub hash_peak_owners: usize,
}
#[derive(Clone, Copy, Default)]
struct Counts {
    control: usize,
    paths: usize,
    scratch: Option<Kind>,
    usage: Usage,
}
thread_local! { static COUNTS: Cell<Counts> = Cell::new(Counts::default()); }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Control,
    Path,
    Golden,
    Hash,
}
struct Lease(Kind);
impl Lease {
    fn new(kind: Kind) -> Result<Self, String> {
        COUNTS.with(|cell| {
            let mut c = cell.get();
            match kind {
                Kind::Control => {
                    if c.control == 3 {
                        return Err("model_control_owner_quota".into());
                    }
                    c.control += 1;
                    c.usage.control_peak_owners = c.usage.control_peak_owners.max(c.control);
                }
                Kind::Path => {
                    if c.paths == 3 {
                        return Err("model_path_owner_quota".into());
                    }
                    c.paths += 1;
                    c.usage.path_peak_owners = c.usage.path_peak_owners.max(c.paths);
                }
                Kind::Golden | Kind::Hash => {
                    if c.scratch.is_some() {
                        return Err("model_scratch_overlap".into());
                    }
                    c.scratch = Some(kind);
                    if kind == Kind::Golden {
                        c.usage.golden_peak_owners = 1;
                    } else {
                        c.usage.hash_peak_owners = 1;
                    }
                }
            }
            cell.set(c);
            Ok(Self(kind))
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        COUNTS.with(|cell| {
            let mut c = cell.get();
            match self.0 {
                Kind::Control => c.control -= 1,
                Kind::Path => c.paths -= 1,
                _ => c.scratch = None,
            }
            cell.set(c);
        });
    }
}
pub struct Scratch(Lease);
impl Scratch {
    pub fn golden() -> Result<Self, String> {
        Ok(Self(Lease::new(Kind::Golden)?))
    }
    pub fn hash() -> Result<Self, String> {
        Ok(Self(Lease::new(Kind::Hash)?))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = &self.0;
    }
}
pub struct Bytes {
    data: Vec<u8>,
    len: usize,
    _lease: Lease,
}
impl Bytes {
    pub fn read(path: &Path) -> Result<Self, String> {
        let lease = Lease::new(Kind::Control)?;
        let mut data = Vec::with_capacity(CONTROL_BYTES);
        if data.capacity() != CONTROL_BYTES {
            return Err("model_control_capacity".into());
        }
        data.resize(CONTROL_BYTES, 0);
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut len = 0;
        loop {
            if len == CONTROL_BYTES {
                let mut probe = [0u8];
                if file.read(&mut probe).map_err(|e| e.to_string())? != 0 {
                    return Err("model_control_file_bound".into());
                }
                break;
            }
            let n = file.read(&mut data[len..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            len += n;
        }
        Ok(Self {
            data,
            len,
            _lease: lease,
        })
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
}
pub fn parse<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = Bytes::read(path)?;
    // Do not copy serde's potentially input-sized error text into another String.
    decode(bytes.as_slice())
}
pub struct OwnedPath {
    path: PathBuf,
    _lease: Lease,
}
impl Deref for OwnedPath {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}
pub fn joined(dir: &Path, name: &str) -> Result<OwnedPath, String> {
    if dir
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .saturating_add(name.len())
        .saturating_add(1)
        > CONTROL_BYTES
    {
        return Err("model_control_path_bound".into());
    }
    let lease = Lease::new(Kind::Path)?;
    let mut path = PathBuf::with_capacity(CONTROL_BYTES);
    if path.capacity() != CONTROL_BYTES {
        return Err("model_control_path_capacity".into());
    }
    path.push(dir);
    path.push(name);
    if path.capacity() != CONTROL_BYTES {
        return Err("model_control_path_growth".into());
    }
    Ok(OwnedPath {
        path,
        _lease: lease,
    })
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Capacity {
    pub limit_bytes: usize,
    pub control_slots_bytes: usize,
    pub parser_error_scratch_bytes: usize,
    pub paths_and_metadata_bytes: usize,
    pub golden_or_hash_bytes: usize,
    pub exit_tail_bytes: usize,
    pub usage: Usage,
}
pub fn capacity() -> Capacity {
    Capacity {
        limit_bytes: HELPER_CAP,
        control_slots_bytes: 3 * CONTROL_BYTES,
        parser_error_scratch_bytes: 40 * 1024,
        paths_and_metadata_bytes: 64 * 1024,
        golden_or_hash_bytes: 64 * 1024,
        exit_tail_bytes: 64 * 1024,
        usage: COUNTS.with(|c| c.get().usage),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_rejects_before_copying_and_round_trips_unicode() {
        let t = Text::<6>::new("城市").unwrap();
        assert_eq!(serde_json::from_str::<Text<6>>("\"城市\"").unwrap(), t);
        assert!(serde_json::from_str::<Text<5>>("\"城市\"").is_err());
    }
    #[test]
    fn leases_reject_overlap_and_return_on_unwind() {
        let a = Lease::new(Kind::Control).unwrap();
        let b = Lease::new(Kind::Control).unwrap();
        let c = Lease::new(Kind::Control).unwrap();
        assert!(Lease::new(Kind::Control).is_err());
        drop(c);
        let d = Lease::new(Kind::Control).unwrap();
        drop((a, b, d));
        let g = Scratch::golden().unwrap();
        assert!(Scratch::hash().is_err());
        drop(g);
        drop(Scratch::hash().unwrap());
        assert!(std::panic::catch_unwind(|| {
            let _g = Scratch::golden().unwrap();
            panic!("release");
        })
        .is_err());
        drop(Scratch::hash().unwrap());
    }
    #[test]
    fn bill_is_exactly_256kib() {
        let b = capacity();
        assert_eq!(
            b.control_slots_bytes
                + b.parser_error_scratch_bytes
                + b.paths_and_metadata_bytes
                + b.golden_or_hash_bytes
                + b.exit_tail_bytes,
            b.limit_bytes
        );
    }
}

/// v2 control strings use unescaped UTF-8/hex. Reject escapes before serde can
/// allocate an unescape scratch Vec. Filesystem paths are hex encoded by prepare.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() > CONTROL_BYTES {
        return Err("model_control_file_bound".into());
    }
    if bytes.contains(&b'\\') {
        return Err("model_control_escape_rejected".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "model_control_json".into())
}
pub fn root_encode(path: &Path) -> Result<Text<2048>, String> {
    let data = path.to_str().ok_or("model_control_path_utf8")?.as_bytes();
    if data.len() > 1024 {
        return Err("model_control_root_bound".into());
    }
    let mut out = [0u8; 2048];
    let hex = b"0123456789abcdef";
    for (i, v) in data.iter().enumerate() {
        out[i * 2] = hex[(v >> 4) as usize];
        out[i * 2 + 1] = hex[(v & 15) as usize];
    }
    Text::new(std::str::from_utf8(&out[..data.len() * 2]).map_err(|_| "model_control_hex")?)
}
pub fn root_decode(value: &str) -> Result<Text<1024>, String> {
    if value.len() > 2048 || value.len() % 2 != 0 {
        return Err("model_control_root_hex".into());
    }
    let mut out = [0u8; 1024];
    fn digit(v: u8) -> Result<u8, String> {
        match v {
            b'0'..=b'9' => Ok(v - b'0'),
            b'a'..=b'f' => Ok(v - b'a' + 10),
            _ => Err("model_control_root_hex".into()),
        }
    }
    for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[i] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Text::new(std::str::from_utf8(&out[..value.len() / 2]).map_err(|_| "model_control_root_utf8")?)
}
#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn escapes_reject_before_serde_unescape_and_path_hex_preserves_names() {
        let text = br#""\u0061\u0061""#;
        assert!(decode::<Text<32>>(text)
            .unwrap_err()
            .contains("escape_rejected"));
        let path = Path::new("/tmp/城市\"with\\characters");
        assert_eq!(
            root_decode(root_encode(path).unwrap().as_str())
                .unwrap()
                .as_str(),
            path.to_str().unwrap()
        );
    }
    #[test]
    fn fourth_artifact_and_fourth_path_cannot_allocate_unbounded_metadata() {
        let result = br#"[null,null,null,null]"#;
        assert!(decode::<[Option<Text<4>>; 3]>(result).is_err());
        let a = joined(Path::new("/tmp"), "a").unwrap();
        let b = joined(Path::new("/tmp"), "b").unwrap();
        let c = joined(Path::new("/tmp"), "c").unwrap();
        assert!(joined(Path::new("/tmp"), "d").is_err());
        drop((a, b, c));
        assert_eq!(
            joined(Path::new("/tmp"), "e").unwrap().as_os_str(),
            "/tmp/e"
        );
        assert!(joined(Path::new("/tmp"), &"x".repeat(CONTROL_BYTES)).is_err());
    }
}

pub fn idle() -> bool {
    COUNTS.with(|c| {
        let c = c.get();
        c.control == 0 && c.paths == 0 && c.scratch.is_none()
    })
}
#[cfg(test)]
mod file_tests {
    use super::*;
    #[test]
    fn fixed_file_slots_refuse_fourth_and_oversized_inputs_and_return_on_error() {
        let path = std::env::temp_dir().join(format!("mycity-control-slot-{}", std::process::id()));
        std::fs::write(&path, b"{}").unwrap();
        let a = Bytes::read(&path).unwrap();
        let b = Bytes::read(&path).unwrap();
        let c = Bytes::read(&path).unwrap();
        assert!(Bytes::read(&path).is_err());
        drop((a, b, c));
        std::fs::write(&path, vec![b' '; CONTROL_BYTES + 1]).unwrap();
        assert!(Bytes::read(&path).is_err());
        assert!(idle());
        std::fs::remove_file(path).unwrap();
    }
}
