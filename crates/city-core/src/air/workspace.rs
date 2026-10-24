//! Fixed, pre-touched heap backings. State/decoder stack and transient parser
//! allocations remain separate; this is not whole-worker/global64MiB acceptance.
use super::*;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::{
    fmt,
    io::Read,
    sync::atomic::{AtomicU64, Ordering},
};

pub const WORKSPACE_BYTES: usize = 1024 * 1024;
const CELLS: usize = 16384;
const SOURCES: usize = 20000;
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const FILL: u8 = 0x5d;
static NEXT: AtomicU64 = AtomicU64::new(1);
fn empty_source() -> Source {
    Source {
        id: 0,
        cell: 0,
        start: 0,
        end: 0,
        rate_micro: 0,
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct WorkspaceReport {
    pub workspace_instance: u64,
    pub payload_capacity_bytes: usize,
    pub typed_backing_bytes: usize,
    pub inactive_typed_bytes: usize,
    pub padding_bytes: usize,
    pub metadata_stack_bytes: usize,
    pub full_fill_scan_passed: bool,
    pub allocation_set_stable: bool,
    pub loaded: bool,
    pub active_sources: usize,
    pub active_cells: usize,
    pub completed_restores: u64,
    pub failed_restores: u64,
    pub pages: u64,
}

/// No Clone, Deref<State>, mutable Input or owning model object is exposed.
pub struct AirWorkspace {
    state: State,
    expected_sha: String,
    padding: Vec<u8>,
    allocations: [usize; 8],
    instance: u64,
    loaded: bool,
    restores: u64,
    failures: u64,
    pages: u64,
}
impl AirWorkspace {
    pub fn new() -> Result<Self, String> {
        Self::with_budget(WORKSPACE_BYTES)
    }
    pub fn with_budget(budget: usize) -> Result<Self, String> {
        let typed = SOURCES
            .checked_mul(size_of::<Source>())
            .and_then(|n| n.checked_add(CELLS * 16))
            .and_then(|n| n.checked_add(STRATEGY.len() + 128))
            .ok_or("air_workspace_capacity_overflow")?;
        if budget != WORKSPACE_BYTES || typed >= budget {
            return Err("air_workspace_budget_before_allocation".into());
        }
        let state = State {
            input: Input {
                schema: STRATEGY.into(),
                side: 32,
                target: 1,
                initial_micro: 0,
                boundary_micro: [Some(0); 4],
                boundary_until: 1,
                sources: Rows::fixed(SOURCES, empty_source())?,
            },
            input_sha256: ZERO_HASH.into(),
            time: 0,
            field: Rows::fixed(CELLS, 0)?,
            scratch: Rows::fixed(CELLS, 0)?,
            integral: Rows::fixed(CELLS, 0)?,
            progress: None,
            totals: Totals::default(),
            pending: Totals::default(),
            steps: 0,
        };
        let mut padding = Vec::new();
        padding
            .try_reserve_exact(budget - typed)
            .map_err(|e| e.to_string())?;
        if padding.capacity() != budget - typed {
            return Err("air_workspace_padding_capacity".into());
        }
        padding.resize(budget - typed, FILL);
        let mut out = Self {
            state,
            expected_sha: ZERO_HASH.into(),
            padding,
            allocations: [0; 8],
            instance: NEXT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| "air_workspace_counter")?,
            loaded: false,
            restores: 0,
            failures: 0,
            pages: 0,
        };
        out.allocations = out.allocation_set();
        out.report()?;
        Ok(out)
    }
    fn allocation_set(&self) -> [usize; 8] {
        let mut ids = [
            self.state.field.allocation(),
            self.state.scratch.allocation(),
            self.state.integral.allocation(),
            self.state.input.sources.allocation(),
            self.state.input.schema.as_ptr() as usize,
            self.state.input_sha256.as_ptr() as usize,
            self.expected_sha.as_ptr() as usize,
            self.padding.as_ptr() as usize,
        ];
        ids.sort_unstable();
        ids
    }
    fn backing_bytes(&self) -> usize {
        self.state.field.capacity() * 4
            + self.state.scratch.capacity() * 4
            + self.state.integral.capacity() * 8
            + self.state.input.sources.capacity() * size_of::<Source>()
            + self.state.input.schema.capacity()
            + self.state.input_sha256.capacity()
            + self.expected_sha.capacity()
    }
    pub fn report(&self) -> Result<WorkspaceReport, String> {
        if !self.state.field.fixed_shape(CELLS)
            || !self.state.scratch.fixed_shape(CELLS)
            || !self.state.integral.fixed_shape(CELLS)
            || !self.state.input.sources.fixed_shape(SOURCES)
            || self.state.input.schema.capacity() != STRATEGY.len()
            || self.state.input_sha256.capacity() != 64
            || self.expected_sha.capacity() != 64
            || self.padding.len() != self.padding.capacity()
            || self.backing_bytes() + self.padding.capacity() != WORKSPACE_BYTES
            || self.allocation_set() != self.allocations
        {
            return Err("air_workspace_backing_changed".into());
        }
        if std::hint::black_box(self.state.field.tail())
            .iter()
            .any(|v| *v != 0)
            || std::hint::black_box(self.state.scratch.tail())
                .iter()
                .any(|v| *v != 0)
            || std::hint::black_box(self.state.integral.tail())
                .iter()
                .any(|v| *v != 0)
            || std::hint::black_box(self.state.input.sources.tail())
                .iter()
                .any(|v| *v != empty_source())
            || std::hint::black_box(self.padding.as_slice())
                .iter()
                .any(|v| *v != FILL)
        {
            return Err("air_workspace_inactive_fill_changed".into());
        }
        if self.loaded {
            self.state.validate()?;
        }
        Ok(WorkspaceReport {
            workspace_instance: self.instance,
            payload_capacity_bytes: WORKSPACE_BYTES,
            typed_backing_bytes: self.backing_bytes(),
            inactive_typed_bytes: (CELLS - self.state.field.len()) * 4
                + (CELLS - self.state.scratch.len()) * 4
                + (CELLS - self.state.integral.len()) * 8
                + (SOURCES - self.state.input.sources.len()) * size_of::<Source>(),
            padding_bytes: self.padding.capacity(),
            metadata_stack_bytes: size_of::<Self>(),
            full_fill_scan_passed: true,
            allocation_set_stable: true,
            loaded: self.loaded,
            active_sources: self.state.input.sources.len(),
            active_cells: self.state.field.len(),
            completed_restores: self.restores,
            failed_restores: self.failures,
            pages: self.pages,
        })
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.loaded = false;
        self.state.field.reset(0)?;
        self.state.scratch.reset(0)?;
        self.state.integral.reset(0)?;
        self.state.input.sources.reset(empty_source())?;
        self.state.input.schema.clear();
        self.state.input.schema.push_str(STRATEGY);
        self.state.input_sha256.clear();
        self.state.input_sha256.push_str(ZERO_HASH);
        self.expected_sha.clear();
        self.expected_sha.push_str(ZERO_HASH);
        self.state.input.side = 32;
        self.state.input.target = 1;
        self.state.input.initial_micro = 0;
        self.state.input.boundary_micro = [Some(0); 4];
        self.state.input.boundary_until = 1;
        self.state.time = 0;
        self.state.progress = None;
        self.state.totals = Totals::default();
        self.state.pending = Totals::default();
        self.state.steps = 0;
        self.report()?;
        Ok(())
    }
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.reset()?;
        let result = (|| {
            if bytes.len() > CHECKPOINT_BYTES {
                return Err("air_checkpoint_bytes".into());
            }
            // Validate token/depth bounds before serde can accumulate escaped scratch.
            let mut guard = crate::fixed_restore::JsonReader::new(bytes);
            let mut scratch = [0u8; 4096];
            while guard.read(&mut scratch).map_err(|e| e.to_string())? != 0 {}
            let mut de = serde_json::Deserializer::from_slice(bytes);
            CheckpointSeed {
                state: &mut self.state,
                hash: &mut self.expected_sha,
            }
            .deserialize(&mut de)
            .map_err(|e| e.to_string())?;
            de.end().map_err(|e| e.to_string())?;
            self.state.validate()?;
            if payload_hash(&self.state)? != self.expected_sha {
                return Err("air_checkpoint_checksum".into());
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failures = self
                .failures
                .checked_add(1)
                .ok_or("air_workspace_counter")?;
            self.reset()?;
            return Err(error);
        }
        self.restores = self
            .restores
            .checked_add(1)
            .ok_or("air_workspace_counter")?;
        self.loaded = true;
        self.report()?;
        Ok(())
    }
    fn ready(&self) -> Result<(), String> {
        if self.loaded {
            Ok(())
        } else {
            Err("air_workspace_not_loaded".into())
        }
    }
    pub fn matches_input(&self, start: u32, end: u32, hash: &str) -> bool {
        self.loaded
            && self.state.time == start
            && self.state.input.target == end
            && self.state.input_sha256 == hash
            && self.state.progress.is_none()
            && !self.state.complete()
    }
    pub fn complete(&self) -> bool {
        self.loaded && self.state.complete()
    }
    pub fn progress(&self) -> Option<Progress> {
        if self.loaded {
            self.state.progress.clone()
        } else {
            None
        }
    }
    pub fn page(&mut self, sources: u32, cells: u32) -> Result<(), String> {
        self.page_counted(sources, cells).map(|_| ())
    }
    pub fn page_counted(&mut self, sources: u32, cells: u32) -> Result<PageWork, String> {
        self.ready()?;
        let result = self.state.page_counted(sources, cells);
        match result {
            Err(e) => { self.reset()?; Err(e) }
            Ok(work) => {
                self.pages = self.pages.checked_add(u64::from(work.pages)).ok_or("air_workspace_counter")?;
                Ok(work)
            }
        }
    }
    pub fn page_diagnosed(
        &mut self, sources: u32, cells: u32, trace: &mut crate::diagnostics::Trace<'_>,
    ) -> Result<(), String> {
        self.page_counted_diagnosed(sources, cells, trace).map(|_| ())
    }
    pub fn page_counted_diagnosed(
        &mut self, sources: u32, cells: u32, trace: &mut crate::diagnostics::Trace<'_>,
    ) -> Result<PageWork, String> {
        self.ready()?;
        let result = self.state.page_counted_diagnosed(sources, cells, trace);
        match result {
            Err(e) => { self.reset()?; Err(e) }
            Ok(work) => {
                self.pages = self.pages.checked_add(u64::from(work.pages)).ok_or("air_workspace_counter")?;
                Ok(work)
            }
        }
    }
    pub fn checkpoint_to(&mut self, writer: &mut impl Write) -> Result<usize, String> {
        self.ready()?;
        self.report()?;
        let result = self.state.checkpoint_to(writer);
        if result.is_err() {
            self.reset()?;
        }
        result
    }
}

fn mark<E: de::Error>(mask: &mut u16, bit: u16) -> Result<(), E> {
    if *mask & bit != 0 {
        return Err(E::custom("air_workspace_duplicate_field"));
    }
    *mask |= bit;
    Ok(())
}
struct HashSeed<'a>(&'a mut String);
impl<'de> DeserializeSeed<'de> for HashSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}
impl Visitor<'_> for HashSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("64 lowercase SHA-256 digits")
    }
    fn visit_str<E: de::Error>(self, s: &str) -> Result<(), E> {
        if s.len() != 64
            || self.0.capacity() != 64
            || !s
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(E::custom("air_workspace_hash"));
        }
        self.0.clear();
        self.0.push_str(s);
        Ok(())
    }
}
struct Schema;
impl<'de> DeserializeSeed<'de> for Schema {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}
impl Visitor<'_> for Schema {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(STRATEGY)
    }
    fn visit_str<E: de::Error>(self, s: &str) -> Result<(), E> {
        if s == STRATEGY {
            Ok(())
        } else {
            Err(E::custom("air_workspace_schema"))
        }
    }
}
struct RejectExtra;
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, _d: D) -> Result<(), D::Error> {
        Err(de::Error::custom("air_workspace_extra_row"))
    }
}
struct RowsSeed<'a, T>(&'a mut Rows<T>);
impl<'de, T: Deserialize<'de> + Clone> DeserializeSeed<'de> for RowsSeed<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T: Deserialize<'de> + Clone> Visitor<'de> for RowsSeed<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded fixed air rows")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        if a.size_hint().is_some_and(|n| n > self.0.capacity()) {
            return Err(de::Error::custom("air_workspace_extra_row"));
        }
        while self.0.len() < self.0.capacity() {
            match a.next_element()? {
                Some(v) => self.0.write_next(v).map_err(de::Error::custom)?,
                None => return Ok(()),
            }
        }
        a.next_element_seed(RejectExtra)?;
        Ok(())
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum InputKey {
    Schema,
    Side,
    Target,
    InitialMicro,
    BoundaryMicro,
    BoundaryUntil,
    Sources,
}
struct InputSeed<'a>(&'a mut Input);
impl<'de> DeserializeSeed<'de> for InputSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for InputSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("fixed air input")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut mask = 0;
        while let Some(k) = a.next_key::<InputKey>()? {
            mark(&mut mask, 1 << (k as u8))?;
            match k {
                InputKey::Schema => a.next_value_seed(Schema)?,
                InputKey::Side => self.0.side = a.next_value()?,
                InputKey::Target => self.0.target = a.next_value()?,
                InputKey::InitialMicro => self.0.initial_micro = a.next_value()?,
                InputKey::BoundaryMicro => self.0.boundary_micro = a.next_value()?,
                InputKey::BoundaryUntil => self.0.boundary_until = a.next_value()?,
                InputKey::Sources => a.next_value_seed(RowsSeed(&mut self.0.sources))?,
            }
        }
        if mask != 127 {
            return Err(de::Error::custom("air_workspace_input_fields"));
        }
        self.0.validate().map_err(de::Error::custom)
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum StateKey {
    Input,
    InputSha256,
    Time,
    Field,
    Scratch,
    Integral,
    Progress,
    Totals,
    Pending,
    Steps,
}
struct StateSeed<'a>(&'a mut State);
impl<'de> DeserializeSeed<'de> for StateSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for StateSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("fixed air state")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut mask = 0;
        while let Some(k) = a.next_key::<StateKey>()? {
            mark(&mut mask, 1 << (k as u8))?;
            match k {
                StateKey::Input => a.next_value_seed(InputSeed(&mut self.0.input))?,
                StateKey::InputSha256 => a.next_value_seed(HashSeed(&mut self.0.input_sha256))?,
                StateKey::Time => self.0.time = a.next_value()?,
                StateKey::Field => a.next_value_seed(RowsSeed(&mut self.0.field))?,
                StateKey::Scratch => a.next_value_seed(RowsSeed(&mut self.0.scratch))?,
                StateKey::Integral => a.next_value_seed(RowsSeed(&mut self.0.integral))?,
                StateKey::Progress => self.0.progress = a.next_value()?,
                StateKey::Totals => self.0.totals = a.next_value()?,
                StateKey::Pending => self.0.pending = a.next_value()?,
                StateKey::Steps => self.0.steps = a.next_value()?,
            }
        }
        if mask != 1023 {
            return Err(de::Error::custom("air_workspace_state_fields"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CheckpointKey {
    Schema,
    Sha256,
    Payload,
}
struct CheckpointSeed<'a> {
    state: &'a mut State,
    hash: &'a mut String,
}
impl<'de> DeserializeSeed<'de> for CheckpointSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for CheckpointSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("fixed air checkpoint")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut mask = 0;
        while let Some(k) = a.next_key::<CheckpointKey>()? {
            mark(&mut mask, 1 << (k as u8))?;
            match k {
                CheckpointKey::Schema => a.next_value_seed(Schema)?,
                CheckpointKey::Sha256 => a.next_value_seed(HashSeed(&mut *self.hash))?,
                CheckpointKey::Payload => a.next_value_seed(StateSeed(&mut *self.state))?,
            }
        }
        if mask != 7 {
            return Err(de::Error::custom("air_workspace_checkpoint_fields"));
        }
        Ok(())
    }
}
