//! Small authoritative/background scratch images; no general 20k-source inflation.
use super::*;
use crate::model_rows::ResidentValue;
const CELLS: usize = 128 * 128;
fn empty_source() -> Source {
    Source {
        id: 0,
        cell: 0,
        start: 0,
        end: 0,
        rate_micro: 0,
    }
}
impl<T: Clone> Rows<T> {
    fn copy_fixed(&mut self, values: &[T], fill: T) -> Result<(), String> {
        if !self.fixed_shape(self.capacity()) || values.len() > self.capacity() {
            return Err("background_fixed_rows".into());
        }
        self.reset(fill)?;
        for v in values {
            self.write_next(v.clone())?;
        }
        Ok(())
    }
}
impl Input {
    pub(crate) fn background_source_allocation(&self) -> usize {
        self.sources.allocation()
    }

    pub(crate) fn background_empty() -> Self {
        Self {
            schema: STRATEGY.into(),
            side: 128,
            target: 1,
            initial_micro: 2_000_000,
            boundary_micro: [Some(2_000_000); 4],
            boundary_until: crate::background::END_LIMIT,
            sources: Rows::fixed(crate::background::SOURCE_LIMIT, empty_source())
                .expect("bounded background sources"),
        }
    }
    pub(crate) fn background_push(&mut self, source: Source) -> Result<(), String> {
        self.sources.write_next(source)
    }
    pub(crate) fn background_reset(&mut self) {
        self.sources
            .reset(empty_source())
            .expect("fixed background source shape");
        self.target = 1;
    }
    pub(crate) fn background_copy(&mut self, input: &Self) -> Result<(), String> {
        if input.schema != STRATEGY
            || input.side != 128
            || input.sources.len() > crate::background::SOURCE_LIMIT
        {
            return Err("background_fixed_input".into());
        }
        self.sources.copy_fixed(&input.sources, empty_source())?;
        self.schema.clone_from(&input.schema);
        self.side = input.side;
        self.target = input.target;
        self.initial_micro = input.initial_micro;
        self.boundary_micro = input.boundary_micro;
        self.boundary_until = input.boundary_until;
        Ok(())
    }
}
impl State {
    pub(crate) fn background_empty() -> Self {
        Self {
            input: Input::background_empty(),
            input_sha256: "0".repeat(64),
            time: 0,
            field: Rows::fixed(CELLS, 0).expect("bounded field"),
            scratch: Rows::fixed(CELLS, 0).expect("bounded scratch"),
            integral: Rows::fixed(CELLS, 0).expect("bounded integral"),
            progress: None,
            totals: Totals::default(),
            pending: Totals::default(),
            steps: 0,
        }
    }
    pub(crate) fn background_reset(&mut self) {
        self.input.background_reset();
        self.field.reset(0).expect("fixed field");
        self.scratch.reset(0).expect("fixed scratch");
        self.integral.reset(0).expect("fixed integral");
        self.time = 0;
        self.progress = None;
        self.totals = Totals::default();
        self.pending = Totals::default();
        self.steps = 0;
        self.input_sha256.clear();
        self.input_sha256.extend(std::iter::repeat('0').take(64));
    }
    pub(crate) fn background_copy(&mut self, source: &Self) -> Result<(), String> {
        if source.field.len() != CELLS
            || source.scratch.len() != CELLS
            || source.integral.len() != CELLS
            || source.input_sha256.len() != 64
        {
            return Err("background_fixed_state".into());
        }
        self.input.background_copy(&source.input)?;
        self.field.copy_fixed(&source.field, 0)?;
        self.scratch.copy_fixed(&source.scratch, 0)?;
        self.integral.copy_fixed(&source.integral, 0)?;
        self.input_sha256.clone_from(&source.input_sha256);
        self.time = source.time;
        self.progress = source.progress.clone();
        self.totals = source.totals.clone();
        self.pending = source.pending.clone();
        self.steps = source.steps;
        Ok(())
    }
    pub(crate) fn background_begin(
        &mut self,
        base: Option<&Self>,
        input: &Input,
    ) -> Result<(), String> {
        input.validate()?;
        if let Some(base) = base {
            if !base.complete()
                || input.target <= base.time
                || input.side != base.input.side
                || input.initial_micro != base.input.initial_micro
                || input.boundary_micro != base.input.boundary_micro
                || input.sources.iter().any(|s| s.start < base.time)
            {
                return Err("air_continuation_boundary".into());
            }
            self.background_copy(base)?;
        } else {
            self.background_reset();
            for _ in 0..CELLS {
                self.field
                    .write_next((input.initial_micro as f32 / MICRO).to_bits())?;
                self.scratch.write_next(0)?;
                self.integral.write_next(0)?;
            }
        }
        self.input.background_copy(input)?;
        self.input_sha256.clone_from(&input.identity());
        self.validate()
    }
    pub(crate) fn background_allocations(&self) -> [usize; 6] {
        let mut ids = [
            self.field.allocation(),
            self.scratch.allocation(),
            self.integral.allocation(),
            self.input.sources.allocation(),
            self.input.schema.as_ptr() as usize,
            self.input_sha256.as_ptr() as usize,
        ];
        ids.sort_unstable();
        ids
    }
}
impl ResidentValue for State {
    fn empty() -> Self {
        Self::background_empty()
    }
    fn reset(&mut self) {
        self.background_reset();
    }
}
