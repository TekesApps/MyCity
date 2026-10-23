//! Direct diagnostic decode into initialized native storage; no World authority.
use super::*;
use crate::{arena::LayoutInto, fixed_restore::ExactSlice};
use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
use std::fmt;

#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Key {
    Accounts,
    Inventory,
    AccountIndex,
    InventoryIndex,
    Claims,
    Payroll,
    Resources,
    Leases,
}

struct NativeInto<'a>(&'a mut NativePressure);
impl<'de> DeserializeSeed<'de> for NativeInto<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for NativeInto<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("existing initialized native diagnostic target")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let out = self.0;
        if [
            out.account_index.len(),
            out.inventory_index.len(),
            out.claims.len(),
            out.payroll.len(),
            out.resources.len(),
            out.leases.len(),
        ] != [40000, 110000, 180000, 31200, 8, 64]
        {
            return Err(de::Error::custom("native_inplace_vector_backing"));
        }
        let mut seen = 0u16;
        while let Some(key) = a.next_key::<Key>()? {
            let bit = 1 << key as u16;
            if seen & bit != 0 {
                return Err(de::Error::custom("native_inplace_duplicate"));
            }
            seen |= bit;
            match key {
                Key::Accounts => a.next_value_seed(LayoutInto::<_, _, 40000> {
                    target: &mut out.accounts,
                })?,
                Key::Inventory => a.next_value_seed(LayoutInto::<_, _, 110000> {
                    target: &mut out.inventory,
                })?,
                Key::AccountIndex => a.next_value_seed(ExactSlice {
                    target: &mut out.account_index,
                })?,
                Key::InventoryIndex => a.next_value_seed(ExactSlice {
                    target: &mut out.inventory_index,
                })?,
                Key::Claims => a.next_value_seed(ExactSlice {
                    target: &mut out.claims,
                })?,
                Key::Payroll => a.next_value_seed(ExactSlice {
                    target: &mut out.payroll,
                })?,
                Key::Resources => a.next_value_seed(ExactSlice {
                    target: &mut out.resources,
                })?,
                Key::Leases => a.next_value_seed(ExactSlice {
                    target: &mut out.leases,
                })?,
            }
        }
        if seen != 255 {
            return Err(de::Error::custom("native_inplace_missing"));
        }
        Ok(())
    }
}
impl NativePressure {
    /// Scan initialized diagnostic workspace storage; no business callbacks or
    /// extra authoritative records are produced by this scratch observation.
    pub fn touch_workspace_backing(&mut self) -> Result<u64, String> {
        let sum = self.scan(|_, _| {})?;
        Ok(sum
            .wrapping_add(self.accounts.touch_empty_free_backing()?)
            .wrapping_add(self.inventory.touch_empty_free_backing()?))
    }
    /// A partial error leaves the diagnostic target invalid; retry into the same
    /// backing is allowed. This never installs a gameplay World or authorizes it.
    pub fn restore_checkpoint_into(
        &mut self,
        reader: impl std::io::Read,
    ) -> Result<RestoreBound, String> {
        let bound = Self::fixed_restore_bound()?;
        if self.owned_capacity_bytes() != bound.decoded_owned_bytes {
            return Err("native_inplace_capacity".into());
        }
        let identities = self.backing_ids();
        let mut d =
            serde_json::Deserializer::from_reader(crate::fixed_restore::JsonReader::new(reader));
        NativeInto(self).deserialize(&mut d).map_err(fail)?;
        d.end().map_err(fail)?;
        if self.backing_ids() != identities {
            return Err("native_inplace_replaced".into());
        }
        for (i, h) in self.account_index.iter().enumerate() {
            if h.raw().index as usize != i
                || self.accounts.get(*h).map_err(fail)?.id as usize != i + 1
            {
                return Err("native_account_index".into());
            }
        }
        for (i, h) in self.inventory_index.iter().enumerate() {
            let stock = self.inventory.get(*h).map_err(fail)?;
            if h.raw().index as usize != i
                || stock.id as usize != i + 1
                || stock.owner != self.account_index[i % 40000]
            {
                return Err("native_stock_index".into());
            }
        }
        Ok(bound)
    }
    /// Address identities for runtime workspace/retry verification only.
    pub fn backing_ids(&self) -> [usize; 10] {
        let a = self.accounts.backing_ids();
        let i = self.inventory.backing_ids();
        [
            a[0],
            a[1],
            i[0],
            i[1],
            self.account_index.as_ptr() as usize,
            self.inventory_index.as_ptr() as usize,
            self.claims.as_ptr() as usize,
            self.payroll.as_ptr() as usize,
            self.resources.as_ptr() as usize,
            self.leases.as_ptr() as usize,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_native_target_reuses_all_ten_backings_after_errors_and_reordered_fields() {
        struct Reverse<'a>(&'a NativePressure);
        impl Serialize for Reverse<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let n = self.0;
                let mut m = s.serialize_map(Some(8))?;
                m.serialize_entry("leases", &n.leases)?;
                m.serialize_entry("resources", &n.resources)?;
                m.serialize_entry("payroll", &n.payroll)?;
                m.serialize_entry("claims", &n.claims)?;
                m.serialize_entry("inventory_index", &n.inventory_index)?;
                m.serialize_entry("account_index", &n.account_index)?;
                m.serialize_entry("inventory", &n.inventory)?;
                m.serialize_entry("accounts", &n.accounts)?;
                m.end()
            }
        }
        let original = NativePressure::jprod().unwrap();
        let canonical = serde_json::to_vec(&original).unwrap();
        let reversed = serde_json::to_vec(&Reverse(&original)).unwrap();
        let mut target = NativePressure::jprod().unwrap();
        const BASELINE_OWNED_BYTES: usize = 29_625_968;
        const ACCOUNT_COUNT: usize = 40_000;
        const STOCK_COUNT: usize = 110_000;
        const LEGACY_ACCOUNT_BYTES: usize = 24;
        const LEGACY_STOCK_BYTES: usize = 88;
        let account_record_bytes = target.accounts.layout().record_bytes;
        let stock_record_bytes = target.inventory.layout().record_bytes;
        assert_eq!(account_record_bytes, 32);
        assert_eq!(stock_record_bytes, 96);
        let provenance_row_growth = ACCOUNT_COUNT * (account_record_bytes - LEGACY_ACCOUNT_BYTES)
            + STOCK_COUNT * (stock_record_bytes - LEGACY_STOCK_BYTES);
        assert_eq!(provenance_row_growth, 1_200_000);
        assert_eq!(
            target.owned_capacity_bytes(),
            BASELINE_OWNED_BYTES + provenance_row_growth
        );
        assert_eq!(target.owned_capacity_bytes(), 30_825_968);
        assert_eq!(
            target.manifest().owned_capacity_bytes,
            target.owned_capacity_bytes()
        );
        let identities = target.backing_ids();
        let leases = serde_json::to_string(&original.leases).unwrap();
        for malformed in [
            "{}".to_string(),
            "{\"unknown\":0}".to_string(),
            format!("{{\"leases\":{leases},\"leases\":{leases}}}"),
            "{\"leases\":[]}".to_string(),
            "{\"accounts\":{\"free\":[{}]}}".to_string(),
        ] {
            assert!(target
                .restore_checkpoint_into(malformed.as_bytes())
                .is_err());
            assert_eq!(identities, target.backing_ids());
            let bound = target.restore_checkpoint_into(&reversed[..]).unwrap();
            assert_eq!(bound.decoded_owned_bytes, target.owned_capacity_bytes());
            assert_eq!(identities, target.backing_ids());
        }
        let mut with_tail = canonical.clone();
        with_tail.extend_from_slice(b" false");
        assert!(target.restore_checkpoint_into(&with_tail[..]).is_err());
        target.restore_checkpoint_into(&canonical[..]).unwrap();
        assert_eq!(identities, target.backing_ids());
        assert_eq!(serde_json::to_vec(&target).unwrap(), canonical);
    }
}
