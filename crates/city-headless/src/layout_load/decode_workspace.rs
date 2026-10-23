//! Persistent, initialized diagnostic decode storage inside the original work budget.
use super::*;
pub(super) const DECODE_BYTES: usize = 64 * MIB;
pub(super) struct DecodeWorkspace {
    pub native: NativePressure,
    tables: [Table; 4],
    tail: Vec<u8>,
    pub valid: bool,
}
impl DecodeWorkspace {
    pub fn new(expected: &[DomainTable]) -> Result<Self, String> {
        let mut maxima = [0usize; 4];
        for domain in expected {
            let i = Self::kind(&domain.table);
            maxima[i] = maxima[i].max(domain.table.count());
        }
        let mut out = Self {
            native: NativePressure::jprod()?,
            tables: [
                Table::new(maxima[0], 64, 0, 1)?,
                Table::new(maxima[1], 128, 0, 1)?,
                Table::new(maxima[2], 256, 0, 1)?,
                Table::new(maxima[3], 4, 0, 1)?,
            ],
            tail: Vec::new(),
            valid: false,
        };
        let used = out.owned_bytes();
        let tail = DECODE_BYTES
            .checked_sub(used)
            .ok_or("layout_decode_workspace_budget")?;
        out.tail = vec![0xa5; tail];
        if out.owned_bytes() != DECODE_BYTES {
            return Err("layout_decode_workspace_capacity".into());
        }
        // Observe every initialized backing before returning; validity here is
        // not publication authority. A snapshot must decode/check all files.
        out.touch()?;
        Ok(out)
    }
    fn kind(table: &Table) -> usize {
        match table {
            Table::W64(_) => 0,
            Table::W128(_) => 1,
            Table::W256(_) => 2,
            Table::PathRefs(_) => 3,
        }
    }
    pub fn decode_table(&mut self, reader: impl Read, expected: &Table) -> Result<(), String> {
        bounded_table::decode_into(reader, expected, &mut self.tables[Self::kind(expected)])
    }
    /// Full cold restore publishes bytes read from disk into the existing
    /// primary backing only after hash/EOF/equality checks by the caller.
    pub fn copy_decoded_table(&self, target: &mut Table) -> Result<(), String> {
        let source = &self.tables[Self::kind(target)];
        macro_rules! copy {
            ($src:ident, $dst:ident) => {{
                let prefix = $src.get(..$dst.len()).ok_or("layout_copy_capacity")?;
                $dst.copy_from_slice(prefix);
            }};
        }
        match (source, target) {
            (Table::W64(src), Table::W64(dst)) => copy!(src, dst),
            (Table::W128(src), Table::W128(dst)) => copy!(src, dst),
            (Table::W256(src), Table::W256(dst)) => copy!(src, dst),
            (Table::PathRefs(src), Table::PathRefs(dst)) => copy!(src, dst),
            _ => return Err("layout_copy_kind".into()),
        }
        Ok(())
    }
    pub fn owned_bytes(&self) -> usize {
        size_of::<Self>() + self.native.owned_capacity_bytes() - size_of::<NativePressure>()
            + self.tables.iter().map(Table::owned).sum::<usize>()
            + self.tail.capacity()
    }
    pub fn touch(&mut self) -> Result<u64, String> {
        let mut sum = self.native.touch_workspace_backing()?;
        for table in &self.tables {
            sum = sum.wrapping_add(table.scan(|_, _| {}));
        }
        for chunk in self.tail.chunks(4096) {
            sum = sum.wrapping_add(u64::from(black_box(chunk[0])));
        }
        Ok(sum)
    }
    pub fn manifest(&self) -> Value {
        json!({"owned_bytes":self.owned_bytes(),"native_owned_bytes":self.native.owned_capacity_bytes(),
            "table_backings":self.tables.iter().map(|t|json!({"count":t.count(),"owned_bytes":t.owned()})).collect::<Vec<_>>(),
            "inline_header_bytes":size_of::<Self>(),"native_inline_header_bytes":size_of::<NativePressure>(),
            "initialized_tail_bytes":self.tail.capacity(),"parser_and_report":"original 4MiB metadata; 2MiB IO already charged there",
            "thread_stack":"original 2MiB native stack, included in physical process measurements",
            "disk_work_image":"128MiB canonical 0xa5, not a serialization of decoder heap"})
    }
    #[cfg(test)]
    fn identities(&self) -> ([usize; 10], [usize; 4], usize) {
        let ids = self.tables.each_ref().map(|table| match table {
            Table::W64(v) => v.as_ptr() as usize,
            Table::W128(v) => v.as_ptr() as usize,
            Table::W256(v) => v.as_ptr() as usize,
            Table::PathRefs(v) => v.as_ptr() as usize,
        });
        (self.native.backing_ids(), ids, self.tail.as_ptr() as usize)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_native_all_table_types_and_initialized_tail_fit_and_reuse() {
        let expected = tables().unwrap();
        let mut w = DecodeWorkspace::new(&expected).unwrap();
        eprintln!("workspace_manifest:{}", w.manifest());
        let id = w.identities();
        let native = w.native.manifest();

        // The persistent equity-source fields grow each native record by 8 B
        // under the pinned Rust ABI: Account 24 -> 32 B and Stock 88 -> 96 B.
        // Capacities and free-index backing stay frozen at the original counts.
        assert_eq!(
            (
                native.account_arena.record_bytes,
                native.account_arena.slot_bytes,
                native.account_arena.slots_capacity,
                native.account_arena.free_index_capacity,
                native.account_arena.live_records,
                native.account_arena.owned_capacity_bytes,
            ),
            (32, 40, 40_000, 40_000, 40_000, 1_760_080)
        );
        assert_eq!(
            (
                native.inventory_arena.record_bytes,
                native.inventory_arena.slot_bytes,
                native.inventory_arena.slots_capacity,
                native.inventory_arena.free_index_capacity,
                native.inventory_arena.live_records,
                native.inventory_arena.owned_capacity_bytes,
            ),
            (96, 104, 110_000, 110_000, 110_000, 11_880_080)
        );
        assert_eq!(
            native.account_arena.owned_capacity_bytes,
            native.account_arena.header_bytes
                + native.account_arena.slots_capacity * native.account_arena.slot_bytes
                + native.account_arena.free_index_capacity * size_of::<u32>()
        );
        assert_eq!(
            native.inventory_arena.owned_capacity_bytes,
            native.inventory_arena.header_bytes
                + native.inventory_arena.slots_capacity * native.inventory_arena.slot_bytes
                + native.inventory_arena.free_index_capacity * size_of::<u32>()
        );

        let expected_vectors = [
            ("account_index", 40_000, 40_000, 32),
            ("inventory_index", 110_000, 110_000, 32),
            ("claims", 180_000, 180_000, 48),
            ("payroll", 31_200, 31_200, 120),
            ("resources", 8, 8, 16),
            ("leases", 64, 64, 24),
        ];
        assert_eq!(native.vectors.as_slice(), expected_vectors.as_slice());

        // The unchanged stub workload remains 30k/160k/24k/900k records.
        let table_counts = w.tables.iter().map(Table::count).collect::<Vec<_>>();
        assert_eq!(table_counts.as_slice(), &[30_000, 160_000, 24_000, 900_000]);
        let table_owned = w.tables.iter().map(Table::owned).sum::<usize>();
        assert_eq!(table_owned, 32_144_000);

        // Rebuild the native owned-byte figure from the frozen arena capacities
        // and vector strides instead of treating the previous ABI total as a
        // current constant. The two Arena headers are already inside NativePressure.
        let vector_owned = native
            .vectors
            .iter()
            .map(|(_, _, capacity, stride)| capacity * stride)
            .sum::<usize>();
        let expected_native_owned = size_of::<NativePressure>()
            + native.account_arena.owned_capacity_bytes
            - native.account_arena.header_bytes
            + native.inventory_arena.owned_capacity_bytes
            - native.inventory_arena.header_bytes
            + vector_owned;
        assert_eq!(size_of::<NativePressure>(), 304);
        assert_eq!(expected_native_owned, 30_825_968);
        assert_eq!(expected_native_owned - 29_625_968, 1_200_000);
        assert_eq!(native.owned_capacity_bytes, expected_native_owned);
        assert_eq!(w.native.owned_capacity_bytes(), expected_native_owned);

        // Keep the original fixed 64 MiB workspace and 128 MiB work reserve.
        // Equity provenance consumes 1.2 MB more native backing; the initialized
        // tail shrinks by exactly that amount instead of expanding the budget.
        let expected_tail = DECODE_BYTES
            - (size_of::<DecodeWorkspace>() + expected_native_owned - size_of::<NativePressure>()
                + table_owned);
        assert_eq!(DECODE_BYTES, 64 * MIB);
        assert_eq!(WORK_PAGES * MIB, 128 * MIB);
        assert_eq!(COW_PAGES * MIB, 64 * MIB);
        assert_eq!(size_of::<DecodeWorkspace>(), 464);
        assert_eq!(expected_tail, 4_138_736);
        assert_eq!(5_338_736 - expected_tail, 1_200_000);
        assert_eq!(w.tail.capacity(), expected_tail);
        assert_eq!(w.owned_bytes(), DECODE_BYTES);
        assert!(!w.tail.is_empty());
        for domain in &expected {
            let bytes = serde_json::to_vec(&domain.table).unwrap();
            assert!(w.decode_table(&b"{}"[..], &domain.table).is_err());
            w.decode_table(&bytes[..], &domain.table).unwrap();
            assert_eq!(w.identities(), id);
            let mut primary = Table::new(domain.table.count(), domain.width, 0, 1).unwrap();
            w.copy_decoded_table(&mut primary).unwrap();
            assert!(primary == domain.table);
        }
        w.touch().unwrap();
        assert_eq!(w.identities(), id);
    }
}
