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
        assert_eq!(w.owned_bytes(), DECODE_BYTES);
        assert_eq!(w.native.owned_capacity_bytes(), 29_625_968);
        assert_eq!(w.tables.iter().map(Table::owned).sum::<usize>(), 32_144_000);
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
