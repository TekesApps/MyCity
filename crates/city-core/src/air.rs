//! RUN36 single-channel CPU reference. Pure laboratory task, no business effects.
use crate::sha256;
use serde::{Deserialize, Serialize};
use std::mem::size_of;
pub const STRATEGY: &str = "run36-air-f32-v1";
pub const INPUT_BYTES: usize = 4 * 1024 * 1024;
pub const CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;
pub const OWNED_BYTES: usize = 2 * 1024 * 1024;
const MICRO: f32 = 1_000_000.0;
const MAX_MICRO: u64 = 1_000_000_000_000;
// Enforce row caps during decoding, before an untrusted checkpoint can allocate huge Vecs.
fn bounded<'de, D, T, const N: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Rows<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Rows<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {N} rows")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|n| n > N) {
                return Err(serde::de::Error::custom("air_rows"));
            }
            let mut rows = Vec::new();
            while let Some(value) = seq.next_element()? {
                if rows.len() == N {
                    return Err(serde::de::Error::custom("air_rows"));
                }
                if rows.len() == rows.capacity() {
                    rows.reserve_exact((N - rows.len()).min(256));
                }
                rows.push(value);
            }
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(Rows::<T, N>(std::marker::PhantomData))
}
fn source_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Source>, D::Error> {
    bounded::<D, Source, 20000>(d)
}
fn field_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    bounded::<D, u32, 16384>(d)
}
fn integral_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u64>, D::Error> {
    bounded::<D, u64, 16384>(d)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub schema: String,
    pub side: u16,
    pub target: u32,
    pub initial_micro: u64,
    /// North, south, east, west prescribed synthetic ghost values.
    pub boundary_micro: [Option<u64>; 4],
    pub boundary_until: u32,
    #[serde(deserialize_with = "source_rows")]
    pub sources: Vec<Source>,
}
impl Input {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > INPUT_BYTES {
            return Err("air_input_bytes".into());
        }
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
        sha256(&serde_json::to_vec(self).expect("integer input"))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Sources,
    Cells,
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
    input_sha256: String,
    time: u32,
    #[serde(deserialize_with = "field_rows")]
    field: Vec<u32>,
    #[serde(deserialize_with = "field_rows")]
    scratch: Vec<u32>,
    #[serde(deserialize_with = "integral_rows")]
    integral: Vec<u64>,
    progress: Option<Progress>,
    totals: Totals,
    pending: Totals,
    steps: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedState {
    input: Input,
    input_sha256: String,
    time: u32,
    #[serde(deserialize_with = "field_rows")]
    field: Vec<u32>,
    #[serde(deserialize_with = "field_rows")]
    scratch: Vec<u32>,
    #[serde(deserialize_with = "integral_rows")]
    integral: Vec<u64>,
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
    schema: String,
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
            field: vec![value; n],
            scratch: vec![0; n],
            integral: vec![0; n],
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
    /// One source page OR one cell page. A completed segment publishes atomically in memory.
    pub fn page(&mut self, source_limit: u32, cell_limit: u32) -> Result<(), String> {
        if source_limit == 0 || source_limit > 8192 || cell_limit == 0 || cell_limit > 1024 {
            return Err("air_page_limit".into());
        }
        if self.complete() {
            return Ok(());
        }
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
            return Ok(());
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
        }
        if end < n {
            self.progress = Some(Progress {
                cursor: end as u32,
                ..p
            });
            return Ok(());
        }
        for i in 0..n {
            let area = 0.5
                * (f64::from(f32::from_bits(self.field[i]))
                    + f64::from(f32::from_bits(self.scratch[i])))
                * f64::from(dt);
            self.integral[i] = (f64::from_bits(self.integral[i]) + area).to_bits();
        }
        std::mem::swap(&mut self.field, &mut self.scratch);
        self.totals.merge(&self.pending);
        self.time = p.end;
        self.steps += 1;
        self.progress = None;
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let payload = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let cp = Checkpoint {
            schema: STRATEGY.into(),
            sha256: sha256(&payload),
            payload: self.clone(),
        };
        let bytes = serde_json::to_vec(&cp).map_err(|e| e.to_string())?;
        if bytes.len() > CHECKPOINT_BYTES {
            return Err("air_checkpoint_bytes".into());
        }
        Ok(bytes)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > CHECKPOINT_BYTES {
            return Err("air_checkpoint_bytes".into());
        }
        let cp: Checkpoint = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if cp.schema != STRATEGY
            || cp.sha256 != sha256(&serde_json::to_vec(&cp.payload).map_err(|e| e.to_string())?)
        {
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
        let mut hash = Vec::with_capacity(self.field.len() * 4);
        for b in &self.field {
            hash.extend(b.to_le_bytes());
        }
        let field_sha256 = sha256(&hash);
        hash.clear();
        for b in &self.integral {
            hash.extend(b.to_le_bytes());
        }
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
            integral_sha256: sha256(&hash),
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
