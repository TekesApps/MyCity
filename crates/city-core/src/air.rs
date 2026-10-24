//! RUN36 single-channel CPU reference. Pure laboratory task, no business effects.
#[cfg(test)]
use crate::sha256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    mem::size_of,
};
mod resident;
mod slot;
pub use slot::{BackgroundStateSlot, StateSlotIdentity};
mod rows;
mod wire;
pub use rows::Rows;
mod workspace;
pub use workspace::{AirWorkspace, WorkspaceReport, WORKSPACE_BYTES};

pub const STRATEGY: &str = "run36-air-f32-v1";
pub const INPUT_BYTES: usize = 4 * 1024 * 1024;
pub const CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;
pub const OWNED_BYTES: usize = 2 * 1024 * 1024;
struct BoundedEncoding<'a, W> {
    inner: &'a mut W,
    bytes: usize,
}
impl<W: Write> Write for BoundedEncoding<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.saturating_add(bytes.len()) > CHECKPOINT_BYTES {
            return Err(io::Error::other("air_checkpoint_bytes"));
        }
        let n = self.inner.write(bytes)?;
        self.bytes += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
// Serialization emits many tiny writes. Batch them without owning an encoded State.
// A checkpoint digest nests its payload digest, so at most two 8KiB buffers coexist.
struct HashWriter {
    hash: Sha256,
    buffer: [u8; 8192],
    used: usize,
}
impl HashWriter {
    fn new() -> Self {
        Self {
            hash: Sha256::new(),
            buffer: [0; 8192],
            used: 0,
        }
    }
    fn drain(&mut self) {
        self.hash.update(&self.buffer[..self.used]);
        self.used = 0;
    }
    fn finish(mut self) -> sha2::digest::Output<Sha256> {
        self.drain();
        self.hash.finalize()
    }
}
impl Write for HashWriter {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len();
        while !bytes.is_empty() {
            if self.used == 0 && bytes.len() >= self.buffer.len() {
                self.hash.update(bytes);
                break;
            }
            let n = bytes.len().min(self.buffer.len() - self.used);
            self.buffer[self.used..self.used + n].copy_from_slice(&bytes[..n]);
            self.used += n;
            bytes = &bytes[n..];
            if self.used == self.buffer.len() {
                self.drain();
            }
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.drain();
        Ok(())
    }
}
fn payload_hash(state: &State) -> Result<String, String> {
    let mut hash = HashWriter::new();
    let mut bounded = BoundedEncoding {
        inner: &mut hash,
        bytes: 0,
    };
    wire::payload(&mut bounded, state)?;
    Ok(format!("{:x}", hash.finish()))
}
const MICRO: f32 = 1_000_000.0;
const MAX_MICRO: u64 = 1_000_000_000_000;
// Enforce row caps during decoding, before an untrusted checkpoint can allocate huge Vecs.
fn bounded<'de, D, T, const N: usize>(d: D) -> Result<Rows<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: crate::model_rows::Record + Deserialize<'de>,
{
    struct Decode<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: crate::model_rows::Record + Deserialize<'de>, const N: usize>
        serde::de::Visitor<'de> for Decode<T, N>
    {
        type Value = Rows<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {N} fixed air records")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Rows::fixed(N, T::inactive()).map_err(serde::de::Error::custom)?;
            for _ in 0..N {
                match seq.next_element()? {
                    Some(v) => out.write_next(v).map_err(serde::de::Error::custom)?,
                    None => return Ok(out),
                }
            }
            seq.next_element_seed(crate::bounded_decode::RejectExtra("air_rows"))?;
            Ok(out)
        }
    }
    d.deserialize_seq(Decode::<T, N>(std::marker::PhantomData))
}
fn source_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Rows<Source>, D::Error> {
    bounded::<D, Source, 20000>(d)
}
fn field_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Rows<u32>, D::Error> {
    bounded::<D, u32, 16384>(d)
}
fn integral_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Rows<u64>, D::Error> {
    bounded::<D, u64, 16384>(d)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: u32,
    pub cell: u32,
    pub start: u32,
    pub end: u32,
    pub rate_micro: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    pub schema: String,
    pub side: u16,
    pub target: u32,
    pub initial_micro: u64,
    /// North, south, east, west prescribed synthetic ghost values.
    pub boundary_micro: [Option<u64>; 4],
    pub boundary_until: u32,
    #[serde(deserialize_with = "source_rows")]
    pub sources: Rows<Source>,
}
impl crate::model_rows::Record for Source {
    fn inactive() -> Self {
        Self {
            id: 0,
            cell: 0,
            start: 0,
            end: 0,
            rate_micro: 0,
        }
    }
}
impl crate::model_rows::Record for u32 {
    fn inactive() -> Self {
        0
    }
}
impl crate::model_rows::Record for u64 {
    fn inactive() -> Self {
        0
    }
}
impl Input {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > INPUT_BYTES {
            return Err("air_input_bytes".into());
        }
        crate::bounded_decode::guard(bytes)?;
        let input: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        input.validate()?;
        Ok(input)
    }
    fn validate(&self) -> Result<(), String> {
        let n = usize::from(self.side).pow(2);
        if self.schema != STRATEGY
            || ![32, 64, 128].contains(&self.side)
            || self.target == 0
            || self.target > 30 * 86400
            || self.initial_micro > MAX_MICRO
            || self.boundary_until < self.target
            || self
                .boundary_micro
                .iter()
                .any(|b| b.is_none_or(|v| v > MAX_MICRO))
            || self.sources.len() > 20000
        {
            return Err("air_input_or_boundary".into());
        }
        for (i, s) in self.sources.iter().enumerate() {
            if s.id == 0
                || (i > 0 && self.sources[i - 1].id >= s.id)
                || s.cell as usize >= n
                || s.start >= s.end
                || s.end > self.target
                || s.rate_micro == 0
                || s.rate_micro > 1_000_000
            {
                return Err("air_source".into());
            }
        }
        Ok(())
    }
    pub fn identity(&self) -> String {
        let mut hash = HashWriter::new();
        serde_json::to_writer(&mut hash, self).expect("integer input");
        format!("{:x}", hash.finish())
    }
    pub fn push_source(&mut self, source: Source) -> Result<(), String> {
        if self.sources.fixed_shape(self.sources.capacity()) {
            self.sources.write_next(source)
        } else {
            self.sources.push_owned(source)
        }
    }
    pub fn clear_sources(&mut self) -> Result<(), String> {
        if self.sources.fixed_shape(self.sources.capacity()) {
            self.sources
                .reset(<Source as crate::model_rows::Record>::inactive())
        } else {
            self.sources.clear_owned()
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Sources,
    Cells,
}
/// Successful work actually executed by one page. Units remain distinct;
/// the segment-end integration pass is additional to cell updates.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageWork {
    pub sources: u32,
    pub cells: u32,
    pub integrated_cells: u32,
    pub segments: u32,
    pub pages: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub end: u32,
    pub phase: Phase,
    pub cursor: u32,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Totals {
    emitted_micro: u64,
    decay: u64,
    background: u64,
    boundary: u64,
    rounding: u64,
}
impl Totals {
    fn add(bits: &mut u64, value: f64) {
        *bits = (f64::from_bits(*bits) + value).to_bits();
    }
    fn merge(&mut self, t: &Self) {
        self.emitted_micro += t.emitted_micro;
        for (a, b) in [
            (&mut self.decay, t.decay),
            (&mut self.background, t.background),
            (&mut self.boundary, t.boundary),
            (&mut self.rounding, t.rounding),
        ] {
            Self::add(a, f64::from_bits(b));
        }
    }
    fn valid(&self) -> bool {
        [self.decay, self.background, self.boundary, self.rounding]
            .iter()
            .all(|b| {
                let v = f64::from_bits(*b);
                v.is_finite() && v.abs() <= 1e24
            })
            && self.emitted_micro <= 51_840_000_000_000_000
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, try_from = "UncheckedState")]
pub struct State {
    input: Input,
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    input_sha256: String,
    time: u32,
    #[serde(deserialize_with = "field_rows")]
    field: Rows<u32>,
    #[serde(deserialize_with = "field_rows")]
    scratch: Rows<u32>,
    #[serde(deserialize_with = "integral_rows")]
    integral: Rows<u64>,
    progress: Option<Progress>,
    totals: Totals,
    pending: Totals,
    steps: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedState {
    input: Input,
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    input_sha256: String,
    time: u32,
    #[serde(deserialize_with = "field_rows")]
    field: Rows<u32>,
    #[serde(deserialize_with = "field_rows")]
    scratch: Rows<u32>,
    #[serde(deserialize_with = "integral_rows")]
    integral: Rows<u64>,
    progress: Option<Progress>,
    totals: Totals,
    pending: Totals,
    steps: u32,
}
impl TryFrom<UncheckedState> for State {
    type Error = String;
    fn try_from(raw: UncheckedState) -> Result<Self, String> {
        let state = Self {
            input: raw.input,
            input_sha256: raw.input_sha256,
            time: raw.time,
            field: raw.field,
            scratch: raw.scratch,
            integral: raw.integral,
            progress: raw.progress,
            totals: raw.totals,
            pending: raw.pending,
            steps: raw.steps,
        };
        state.validate()?;
        Ok(state)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    schema: String,
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    sha256: String,
    payload: State,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub strategy: &'static str,
    pub input_sha256: String,
    pub time: u32,
    pub target: u32,
    pub complete: bool,
    pub progress: Option<Progress>,
    pub steps: u32,
    pub side: u16,
    pub field_sha256: String,
    pub integral_sha256: String,
    pub minimum: f32,
    pub maximum: f32,
    pub sum: f64,
    pub emitted_micro: u64,
    pub decay: f64,
    pub background: f64,
    pub boundary_exchange: f64,
    pub rounding_residual: f64,
    pub owned_bytes: usize,
}
impl State {
    pub fn new(input: Input) -> Result<Self, String> {
        input.validate()?;
        let n = usize::from(input.side).pow(2);
        let value = (input.initial_micro as f32 / MICRO).to_bits();
        let s = Self {
            input_sha256: input.identity(),
            input,
            time: 0,
            field: vec![value; n].into(),
            scratch: vec![0; n].into(),
            integral: vec![0; n].into(),
            progress: None,
            totals: Totals::default(),
            pending: Totals::default(),
            steps: 0,
        };
        s.validate()?;
        Ok(s)
    }
    /// Continue a completed interval without resetting its field, integral or ledger.
    /// New targets are explicit numerical cuts; arbitrary cuts need not be bit-equivalent.
    pub fn continue_from_boundary(&self, input: Input) -> Result<Self, String> {
        input.validate()?;
        if !self.complete()
            || input.target <= self.time
            || input.side != self.input.side
            || input.initial_micro != self.input.initial_micro
            || input.boundary_micro != self.input.boundary_micro
            || input.sources.iter().any(|s| s.start < self.time)
        {
            return Err("air_continuation_boundary".into());
        }
        let mut next = self.clone();
        next.input_sha256 = input.identity();
        next.input = input;
        next.validate()?;
        Ok(next)
    }
    pub fn time(&self) -> u32 {
        self.time
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn field_bits(&self) -> &[u32] {
        &self.field
    }
    pub fn integral_bits(&self) -> &[u64] {
        &self.integral
    }
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }
    pub fn complete(&self) -> bool {
        self.time == self.input.target && self.progress.is_none()
    }
    pub fn owned_bytes(&self) -> usize {
        size_of::<Self>()
            + self.input.schema.capacity()
            + self.input_sha256.capacity()
            + self.input.sources.capacity() * size_of::<Source>()
            + self.field.capacity() * 4
            + self.scratch.capacity() * 4
            + self.integral.capacity() * 8
    }
    fn end(&self) -> u32 {
        let mut end = ((self.time / 60 + 1) * 60).min(self.input.target);
        for s in &self.input.sources {
            for at in [s.start, s.end] {
                if at > self.time {
                    end = end.min(at);
                }
            }
        }
        end
    }
    fn validate(&self) -> Result<(), String> {
        self.input.validate()?;
        let n = usize::from(self.input.side).pow(2);
        if self.time > self.input.target
            || self.input_sha256 != self.input.identity()
            || self.field.len() != n
            || self.scratch.len() != n
            || self.integral.len() != n
            || self.owned_bytes() > OWNED_BYTES
            || !self.totals.valid()
            || !self.pending.valid()
            || self.steps > self.input.target
        {
            return Err("air_state".into());
        }
        if self.field.iter().chain(self.scratch.iter()).any(|b| {
            let v = f32::from_bits(*b);
            !v.is_finite() || !(0.0..=1e12).contains(&v)
        }) || self.integral.iter().any(|b| {
            let v = f64::from_bits(*b);
            !v.is_finite() || !(0.0..=1e20).contains(&v)
        }) {
            return Err("air_nonfinite_or_negative".into());
        }
        if let Some(p) = &self.progress {
            let cap = if p.phase == Phase::Sources {
                self.input.sources.len()
            } else {
                n
            };
            if p.end != self.end()
                || p.end <= self.time
                || p.cursor as usize > cap
                || self.time == self.input.target
            {
                return Err("air_cursor".into());
            }
        }
        Ok(())
    }
    /// Same exact page; observation is outside the measured compute interval.
    pub fn page_diagnosed(
        &mut self,
        source_limit: u32,
        cell_limit: u32,
        trace: &mut crate::diagnostics::Trace<'_>,
    ) -> Result<(), String> {
        self.page_counted_diagnosed(source_limit, cell_limit, trace).map(|_| ())
    }
    pub fn page_counted_diagnosed(
        &mut self,
        source_limit: u32,
        cell_limit: u32,
        trace: &mut crate::diagnostics::Trace<'_>,
    ) -> Result<PageWork, String> {
        use crate::diagnostics::{AirPage, Class};
        let Some(start) = trace.start() else {
            return self.page_counted(source_limit, cell_limit);
        };
        let (class, items) = if self.complete() {
            (Class::AirEmpty, 0)
        } else {
            let p = self.progress.as_ref();
            let cursor = p.map_or(0, |p| p.cursor as usize);
            if p.is_none_or(|p| p.phase == Phase::Sources) {
                (
                    Class::AirSources,
                    self.input
                        .sources
                        .len()
                        .saturating_sub(cursor)
                        .min(source_limit as usize) as u32,
                )
            } else {
                (
                    Class::AirCells,
                    self.field
                        .len()
                        .saturating_sub(cursor)
                        .min(cell_limit as usize) as u32,
                )
            }
        };
        let result = self.page_counted(source_limit, cell_limit);
        let nanos = trace.nanos(Some(start));
        trace.page(AirPage {
            role: trace.role,
            class: if items == 0 { Class::AirEmpty } else { class },
            nanos,
            items,
            source_limit,
            cell_limit,
            ok: result.is_ok(),
        });
        result
    }
    /// One source page OR one cell page. A completed segment publishes atomically in memory.
    pub fn page(&mut self, source_limit: u32, cell_limit: u32) -> Result<(), String> {
        self.page_counted(source_limit, cell_limit).map(|_| ())
    }
    /// Count only successful loop bodies, not requested quota or a progress cursor.
    pub fn page_counted(&mut self, source_limit: u32, cell_limit: u32) -> Result<PageWork, String> {
        let mut work = PageWork::default();
        if source_limit == 0 || source_limit > 8192 || cell_limit == 0 || cell_limit > 1024 {
            return Err("air_page_limit".into());
        }
        if self.complete() {
            return Ok(work);
        }
        work.pages = 1;
        if self.progress.is_none() {
            self.scratch.fill(0);
            self.pending = Totals::default();
            self.progress = Some(Progress {
                end: self.end(),
                phase: Phase::Sources,
                cursor: 0,
            });
        }
        let p = self.progress.as_ref().expect("started").clone();
        let dt = p.end - self.time;
        if p.phase == Phase::Sources {
            let end = (p.cursor as usize + source_limit as usize).min(self.input.sources.len());
            for s in &self.input.sources[p.cursor as usize..end] {
                let seconds = p.end.min(s.end).saturating_sub(self.time.max(s.start));
                let micro = u64::from(s.rate_micro)
                    .checked_mul(u64::from(seconds))
                    .ok_or("air_source_overflow")?;
                self.pending.emitted_micro = self
                    .pending
                    .emitted_micro
                    .checked_add(micro)
                    .ok_or("air_source_overflow")?;
                let i = s.cell as usize;
                let before = f32::from_bits(self.scratch[i]);
                let after = before + micro as f32 / MICRO;
                Totals::add(
                    &mut self.pending.rounding,
                    f64::from(after) - f64::from(before) - micro as f64 / 1_000_000.0,
                );
                self.scratch[i] = after.to_bits();
                work.sources += 1;
            }
            self.progress = Some(if end == self.input.sources.len() {
                Progress {
                    end: p.end,
                    phase: Phase::Cells,
                    cursor: 0,
                }
            } else {
                Progress {
                    cursor: end as u32,
                    ..p
                }
            });
            return Ok(work);
        }
        let h = dt as f32 / 60.0;
        let r = 0.02f32 * h;
        let d = 0.01f32 * h;
        let side = usize::from(self.input.side);
        let n = self.field.len();
        let end = (p.cursor as usize + cell_limit as usize).min(n);
        let ghosts = self
            .input
            .boundary_micro
            .map(|v| v.expect("validated") as f32 / MICRO);
        for i in p.cursor as usize..end {
            let y = i / side;
            let x = i % side;
            let old = f32::from_bits(self.field[i]);
            let north = if y == 0 {
                ghosts[0]
            } else {
                f32::from_bits(self.field[i - side])
            };
            let south = if y + 1 == side {
                ghosts[1]
            } else {
                f32::from_bits(self.field[i + side])
            };
            let east = if x + 1 == side {
                ghosts[2]
            } else {
                f32::from_bits(self.field[i + 1])
            };
            let west = if x == 0 {
                ghosts[3]
            } else {
                f32::from_bits(self.field[i - 1])
            };
            let neighbors = ((north + south) + east) + west;
            let mix = (1.0 - 4.0 * r) * old + r * neighbors;
            let source = f32::from_bits(self.scratch[i]);
            let next = ((1.0 - d) * mix + source) + d * 2.0;
            self.scratch[i] = next.to_bits();
            let exchange = [
                (y == 0, north),
                (y + 1 == side, south),
                (x + 1 == side, east),
                (x == 0, west),
            ]
            .iter()
            .filter(|(edge, _)| *edge)
            .map(|(_, g)| f64::from(r) * (f64::from(*g) - f64::from(old)))
            .sum::<f64>();
            Totals::add(&mut self.pending.boundary, (1.0 - f64::from(d)) * exchange);
            Totals::add(&mut self.pending.decay, f64::from(d) * f64::from(old));
            Totals::add(&mut self.pending.background, f64::from(d) * 2.0);
            let ideal = (1.0 - f64::from(d))
                * (f64::from(old)
                    + f64::from(r)
                        * (f64::from(north)
                            + f64::from(south)
                            + f64::from(east)
                            + f64::from(west)
                            - 4.0 * f64::from(old)))
                + f64::from(source)
                + f64::from(d) * 2.0;
            Totals::add(&mut self.pending.rounding, f64::from(next) - ideal);
            work.cells += 1;
        }
        if end < n {
            self.progress = Some(Progress {
                cursor: end as u32,
                ..p
            });
            return Ok(work);
        }
        for i in 0..n {
            let area = 0.5
                * (f64::from(f32::from_bits(self.field[i]))
                    + f64::from(f32::from_bits(self.scratch[i])))
                * f64::from(dt);
            self.integral[i] = (f64::from_bits(self.integral[i]) + area).to_bits();
            work.integrated_cells += 1;
        }
        std::mem::swap(&mut self.field, &mut self.scratch);
        self.totals.merge(&self.pending);
        self.time = p.end;
        self.steps += 1;
        self.progress = None;
        work.segments = 1;
        Ok(work)
    }
    /// Same immutable checkpoint wire format; stream into an unpublished bounded
    /// destination. An I/O failure may leave a prefix, never a valid publication.
    pub fn checkpoint_to(&self, writer: &mut impl Write) -> Result<usize, String> {
        self.validate()?;
        let hash = payload_hash(self)?;
        let mut bounded = BoundedEncoding {
            inner: writer,
            bytes: 0,
        };
        bounded
            .write_all(b"{\"schema\":")
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut bounded, STRATEGY).map_err(|e| e.to_string())?;
        bounded
            .write_all(b",\"sha256\":")
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut bounded, &hash).map_err(|e| e.to_string())?;
        bounded
            .write_all(b",\"payload\":")
            .map_err(|e| e.to_string())?;
        wire::payload(&mut bounded, self)?;
        bounded.write_all(b"}").map_err(|e| e.to_string())?;
        Ok(bounded.bytes)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        self.checkpoint_to(&mut bytes)?;
        Ok(bytes)
    }
    /// Canonical checkpoint byte count and SHA-256 without a full encoding Vec.
    /// This preserves checkpoint_to's validation, size bound and exact wire bytes.
    pub fn checkpoint_digest(&self) -> Result<(usize, [u8; 32]), String> {
        let mut hash = HashWriter::new();
        let bytes = self.checkpoint_to(&mut hash)?;
        Ok((bytes, hash.finish().into()))
    }
    /// Hex digest of the complete checkpoint wire without an encoded body Vec.
    pub fn checkpoint_sha256(&self) -> Result<String, String> {
        use std::fmt::Write as _;
        let mut text = String::with_capacity(64);
        for byte in self.checkpoint_digest()?.1 {
            write!(&mut text, "{byte:02x}").expect("String write");
        }
        Ok(text)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > CHECKPOINT_BYTES {
            return Err("air_checkpoint_bytes".into());
        }
        crate::bounded_decode::guard(bytes)?;
        let cp: Checkpoint = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if cp.schema != STRATEGY || cp.sha256 != payload_hash(&cp.payload)? {
            return Err("air_checkpoint_checksum".into());
        }
        cp.payload.validate()?;
        Ok(cp.payload)
    }
    pub fn restore_for(bytes: &[u8], input: &Input) -> Result<Self, String> {
        input.validate()?;
        let state = Self::restore(bytes)?;
        if state.input_sha256 != input.identity() {
            return Err("air_stale_input".into());
        }
        Ok(state)
    }
    pub fn report(&self) -> Report {
        // Hash the exact old little-endian wire with a fixed 4KiB scratch.
        // The old integral pass grew a 64KiB Vec to 128KiB while reporting.
        let mut buffer = [0u8; 4096];
        let mut field_hash = Sha256::new();
        for chunk in self.field.chunks(1024) {
            for (out, value) in buffer.chunks_exact_mut(4).zip(chunk) {
                out.copy_from_slice(&value.to_le_bytes());
            }
            field_hash.update(&buffer[..chunk.len() * 4]);
        }
        let field_sha256 = format!("{:x}", field_hash.finalize());
        let mut integral_hash = Sha256::new();
        for chunk in self.integral.chunks(512) {
            for (out, value) in buffer.chunks_exact_mut(8).zip(chunk) {
                out.copy_from_slice(&value.to_le_bytes());
            }
            integral_hash.update(&buffer[..chunk.len() * 8]);
        }
        let integral_sha256 = format!("{:x}", integral_hash.finalize());
        let values = self.field.iter().map(|b| f32::from_bits(*b));
        Report {
            strategy: STRATEGY,
            input_sha256: self.input_sha256.clone(),
            time: self.time,
            target: self.input.target,
            complete: self.complete(),
            progress: self.progress.clone(),
            steps: self.steps,
            side: self.input.side,
            field_sha256,
            integral_sha256,
            minimum: values.clone().fold(f32::INFINITY, f32::min),
            maximum: values.clone().fold(0.0, f32::max),
            sum: values.map(f64::from).sum(),
            emitted_micro: self.totals.emitted_micro,
            decay: f64::from_bits(self.totals.decay),
            background: f64::from_bits(self.totals.background),
            boundary_exchange: f64::from_bits(self.totals.boundary),
            rounding_residual: f64::from_bits(self.totals.rounding),
            owned_bytes: self.owned_bytes(),
        }
    }
}

impl crate::restore_into::Restore for Input {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        crate::restore_into::object!(d,self,Input,127u64,{
        schema,0=>Text,
        side,1=>Copy,
        target,2=>Copy,
        initial_micro,3=>Copy,
        boundary_micro,4=>Copy,
        boundary_until,5=>Copy,
        sources,6=>Into,
        })?;
        self.validate().map_err(serde::de::Error::custom)?;
        if self.side != 128 {
            return Err(serde::de::Error::custom("background_fixed_input"));
        }
        Ok(())
    }
}

impl crate::restore_into::Restore for State {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        crate::restore_into::object!(d,self,State,959u64,{
        input,0=>Into,
        input_sha256,1=>Text,
        time,2=>Copy,
        field,3=>Into,
        scratch,4=>Into,
        integral,5=>Into,
        progress,6=>Flat,
        totals,7=>Flat,
        pending,8=>Flat,
        steps,9=>Copy,
        })?;
        self.validate().map_err(serde::de::Error::custom)?;
        if self.input.side != 128
            || self.field.len() != 16384
            || self.scratch.len() != 16384
            || self.integral.len() != 16384
        {
            return Err(serde::de::Error::custom("background_fixed_state"));
        }
        Ok(())
    }
}

impl crate::restore_into::FlatValue for Option<Progress> {}
impl crate::restore_into::FlatValue for Totals {}

#[cfg(test)]
mod report_hash_tests {
    use super::*;
    #[test]
    fn fixed_buffer_hash_is_chunk_and_flush_independent() {
        for len in [0, 1, 8191, 8192, 8193, 32769] {
            let bytes: Vec<u8> = (0..len).map(|n| (n % 251) as u8).collect();
            for chunk in [1, 7, 8191, 8192, 8193, 65536] {
                let mut hash = HashWriter::new();
                hash.write_all(&[]).unwrap();
                for (i, part) in bytes.chunks(chunk).enumerate() {
                    hash.write_all(part).unwrap();
                    if i % 3 == 0 {
                        hash.flush().unwrap();
                    }
                }
                assert_eq!(hash.finish(), Sha256::digest(&bytes));
            }
        }
    }
    #[test]
    fn fixed_scratch_report_matches_the_previous_complete_wire_hashes() {
        for side in [32, 128] {
            let mut state = State::new(Input {
                schema: STRATEGY.into(),
                side,
                target: 120,
                initial_micro: 2_000_000,
                boundary_micro: [Some(2_000_000); 4],
                boundary_until: 120,
                sources: vec![].into(),
            })
            .unwrap();
            for (i, v) in state.field.iter_mut().enumerate() {
                *v = (2.0 + (i % 1234) as f32 * 0.000001).to_bits();
            }
            for (i, v) in state.integral.iter_mut().enumerate() {
                *v = (i as f64 / 7.0).to_bits();
            }
            let old_field: Vec<u8> = state.field.iter().flat_map(|v| v.to_le_bytes()).collect();
            let old_integral: Vec<u8> = state
                .integral
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let report = state.report();
            assert_eq!(report.field_sha256, sha256(&old_field));
            assert_eq!(report.integral_sha256, sha256(&old_integral));
        }
    }
}
