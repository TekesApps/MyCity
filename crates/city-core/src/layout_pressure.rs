//! Diagnostic native storage only. This cannot be installed into an authoritative World.
//! Counts exceed gameplay caps deliberately; no business outcome is inferred from these scans.
mod restore_into;
use crate::{
    arena::{AccountDomain, Arena, Handle, InventoryDomain, Layout, Scope},
    assembly::{Account, Commodity, Role, Stock},
    timeline::{Claim, Cycle, Payroll, PayrollSpec},
    transaction::{Lease, Resource},
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
use std::{hint::black_box, mem::size_of};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePressure {
    #[serde(deserialize_with = "decode_accounts")]
    accounts: Arena<AccountDomain, Account>,
    #[serde(deserialize_with = "decode_inventory")]
    inventory: Arena<InventoryDomain, Stock>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 40000>")]
    account_index: Vec<Handle<AccountDomain>>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 110000>")]
    inventory_index: Vec<Handle<InventoryDomain>>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 180000>")]
    claims: Vec<Claim>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 31200>")]
    payroll: Vec<Payroll>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 8>")]
    resources: Vec<Resource>,
    #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 64>")]
    leases: Vec<Lease>,
}
fn decode_accounts<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Arena<AccountDomain, Account>, D::Error> {
    Arena::deserialize_layout::<D, 40000>(d)
}
fn decode_inventory<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Arena<InventoryDomain, Stock>, D::Error> {
    Arena::deserialize_layout::<D, 110000>(d)
}
#[derive(Serialize)]
pub struct Manifest {
    pub schema: &'static str,
    pub scope: &'static str,
    pub account_arena: Layout,
    pub inventory_arena: Layout,
    pub fields: Vec<(&'static str, usize)>,
    pub vectors: Vec<(&'static str, usize, usize, usize)>,
    pub owned_capacity_bytes: usize,
}
/// A conservative capacity proof for this fixed, hash-verified native fixture.
/// Includes the exact decoded graph, no buffer replacement, and 6MiB for
/// parser/IO/stack slack. This is not an allocator measurement of arbitrary JSON.
#[derive(Serialize)]
pub struct RestoreBound {
    pub decoded_owned_bytes: usize,
    pub largest_replacement_bytes: usize,
    pub parser_io_stack_slack_bytes: usize,
    pub conservative_transient_bound_bytes: usize,
}
fn fail(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
impl NativePressure {
    pub fn jprod() -> Result<Self, String> {
        let scope = Scope::new(0x5441, 1).map_err(fail)?;
        let mut accounts = Arena::new(scope, 40000, 8 * 1024 * 1024).map_err(fail)?;
        let mut inventory = Arena::new(scope, 110000, 32 * 1024 * 1024).map_err(fail)?;
        let mut account_index = Vec::with_capacity(40000);
        let mut inventory_index = Vec::with_capacity(110000);
        for id in 1..=40000 {
            account_index.push(
                accounts
                    .insert(Account {
                        id,
                        role: Role::Household,
                        cash: Money::new(i64::from(id)).map_err(fail)?,
                        protected: Money::ZERO,
                        opening_source_id: 0,
                    })
                    .map_err(fail)?,
            );
        }
        for id in 1..=110000 {
            inventory_index.push(
                inventory
                    .insert(Stock {
                        id,
                        owner: account_index[(id as usize - 1) % 40000],
                        commodity: Commodity::Raw,
                        quantity: Quantity::new(i64::from(id)).map_err(fail)?,
                        basis: Money::new(i64::from(id)).map_err(fail)?,
                        lot_id: id,
                        origin_quantity: Quantity::new(i64::from(id)).map_err(fail)?,
                        origin_basis: Money::new(i64::from(id)).map_err(fail)?,
                        offset: Quantity::ZERO,
                        opening_source_id: 0,
                        opening_basis: Money::ZERO,
                    })
                    .map_err(fail)?,
            );
        }
        let mut claims = Vec::with_capacity(180000);
        for id in 1..=180000 {
            claims.push(Claim {
                id,
                payer: 1,
                payee: 2,
                due: id,
                principal: Money::new(1).map_err(fail)?,
                paid: Money::ZERO,
                processed: false,
                source_ref: 0,
            });
        }
        let mut payroll = Vec::with_capacity(31200);
        for id in 1..=31200 {
            payroll.push(
                Payroll::new(
                    PayrollSpec {
                        id,
                        payer: 1,
                        payee: 2,
                        home_city: 1,
                        hourly_cents: Money::new(1).map_err(fail)?,
                        cycle: Cycle::Daily,
                        ends_at: 30 * 86400,
                    },
                    0,
                )
                .map_err(fail)?,
            );
        }
        let resources = (1..=8)
            .map(|id| Resource::new(id, 64).map_err(fail))
            .collect::<Result<Vec<_>, _>>()?;
        let leases = (1..=64)
            .map(|token| Lease {
                token,
                actor: 1,
                resource: 1,
                units: 1,
                occupied: false,
            })
            .collect();
        Ok(Self {
            accounts,
            inventory,
            account_index,
            inventory_index,
            claims,
            payroll,
            resources,
            leases,
        })
    }
    pub fn manifest(&self) -> Manifest {
        let vectors = vec![
            (
                "account_index",
                self.account_index.len(),
                self.account_index.capacity(),
                size_of::<Handle<AccountDomain>>(),
            ),
            (
                "inventory_index",
                self.inventory_index.len(),
                self.inventory_index.capacity(),
                size_of::<Handle<InventoryDomain>>(),
            ),
            (
                "claims",
                self.claims.len(),
                self.claims.capacity(),
                size_of::<Claim>(),
            ),
            (
                "payroll",
                self.payroll.len(),
                self.payroll.capacity(),
                size_of::<Payroll>(),
            ),
            (
                "resources",
                self.resources.len(),
                self.resources.capacity(),
                size_of::<Resource>(),
            ),
            (
                "leases",
                self.leases.len(),
                self.leases.capacity(),
                size_of::<Lease>(),
            ),
        ];
        let owned_capacity_bytes = size_of::<Self>() + self.accounts.owned_bytes()
            - size_of::<Arena<AccountDomain, Account>>()
            + self.inventory.owned_bytes()
            - size_of::<Arena<InventoryDomain, Stock>>()
            + vectors
                .iter()
                .map(|(_, _, capacity, width)| capacity * width)
                .sum::<usize>();
        Manifest {
            schema: "mycity-native-layout-v1",
            scope: "actual_types_synthetic_records_not_business_simulation",
            fields: vec![
                ("Account.id", std::mem::offset_of!(Account, id)),
                ("Account.role", std::mem::offset_of!(Account, role)),
                ("Account.cash", std::mem::offset_of!(Account, cash)),
                (
                    "Account.protected",
                    std::mem::offset_of!(Account, protected),
                ),
                (
                    "Account.opening_source_id",
                    std::mem::offset_of!(Account, opening_source_id),
                ),
                ("Stock.id", std::mem::offset_of!(Stock, id)),
                ("Stock.owner", std::mem::offset_of!(Stock, owner)),
                ("Stock.commodity", std::mem::offset_of!(Stock, commodity)),
                ("Stock.quantity", std::mem::offset_of!(Stock, quantity)),
                ("Stock.basis", std::mem::offset_of!(Stock, basis)),
                ("Stock.lot_id", std::mem::offset_of!(Stock, lot_id)),
                (
                    "Stock.origin_quantity",
                    std::mem::offset_of!(Stock, origin_quantity),
                ),
                (
                    "Stock.origin_basis",
                    std::mem::offset_of!(Stock, origin_basis),
                ),
                ("Stock.offset", std::mem::offset_of!(Stock, offset)),
                (
                    "Stock.opening_source_id",
                    std::mem::offset_of!(Stock, opening_source_id),
                ),
                (
                    "Stock.opening_basis",
                    std::mem::offset_of!(Stock, opening_basis),
                ),
            ],
            account_arena: self.accounts.layout(),
            inventory_arena: self.inventory.layout(),
            vectors,
            owned_capacity_bytes,
        }
    }
    /// Actual retained backing, without allocating a manifest/report tree.
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.accounts.owned_bytes() - size_of::<Arena<AccountDomain, Account>>()
            + self.inventory.owned_bytes()
            - size_of::<Arena<InventoryDomain, Stock>>()
            + self.account_index.capacity() * size_of::<Handle<AccountDomain>>()
            + self.inventory_index.capacity() * size_of::<Handle<InventoryDomain>>()
            + self.claims.capacity() * size_of::<Claim>()
            + self.payroll.capacity() * size_of::<Payroll>()
            + self.resources.capacity() * size_of::<Resource>()
            + self.leases.capacity() * size_of::<Lease>()
    }
    /// Scan ALL records and external indexes, with fixed 8192-record page boundaries.
    pub fn scan(&self, mut page: impl FnMut(usize, u64)) -> Result<u64, String> {
        let mut sum = 0u64;
        let mut emit = |count: usize, value: u64| {
            sum = sum.wrapping_add(value);
            page(count, black_box(value));
        };
        for entries in self.account_index.chunks(8192) {
            let mut value = 0u64;
            for &h in entries {
                let a = black_box(self.accounts.get(h).map_err(fail)?);
                value = value.wrapping_add(u64::from(a.id) ^ a.cash.value() as u64);
            }
            emit(entries.len(), value);
        }
        for entries in self.inventory_index.chunks(8192) {
            let mut value = 0u64;
            for &h in entries {
                let s = black_box(self.inventory.get(h).map_err(fail)?);
                value = value.wrapping_add(
                    u64::from(s.id)
                        ^ s.quantity.value() as u64
                        ^ s.basis.value() as u64
                        ^ u64::from(s.owner.raw().index),
                );
            }
            emit(entries.len(), value);
        }
        for entries in self.claims.chunks(8192) {
            emit(
                entries.len(),
                entries
                    .iter()
                    .map(|c| black_box(c).id.wrapping_add(c.principal.value() as u64))
                    .fold(0u64, u64::wrapping_add),
            );
        }
        for entries in self.payroll.chunks(8192) {
            emit(
                entries.len(),
                entries
                    .iter()
                    .map(|p| u64::from(black_box(p).spec.id) + p.spec.ends_at)
                    .fold(0u64, u64::wrapping_add),
            );
        }
        emit(
            self.resources.len(),
            self.resources
                .iter()
                .map(|r| u64::from(black_box(r).capacity))
                .sum(),
        );
        emit(
            self.leases.len(),
            self.leases.iter().map(|l| black_box(l).token).sum(),
        );
        Ok(black_box(sum))
    }
    /// Diagnostic snapshot only; never restores a World or authorizes gameplay state.
    pub fn restore_checkpoint(reader: impl std::io::Read) -> Result<Self, String> {
        Self::restore_checkpoint_profiled(reader).map(|(candidate, _)| candidate)
    }
    pub fn restore_checkpoint_profiled(
        reader: impl std::io::Read,
    ) -> Result<(Self, RestoreBound), String> {
        // Compute the whole simultaneous destination graph before reading input.
        // Every sequence below reserves exactly once, with no buffer replacement.
        let bound = Self::fixed_restore_bound()?;
        let mut decoder =
            serde_json::Deserializer::from_reader(crate::fixed_restore::JsonReader::new(reader));
        let restored = Self::deserialize(&mut decoder).map_err(fail)?;
        decoder.end().map_err(fail)?;
        if restored.manifest().owned_capacity_bytes != bound.decoded_owned_bytes {
            return Err("native_fixed_restore_capacity_changed".into());
        }
        for (i, h) in restored.account_index.iter().enumerate() {
            if h.raw().index as usize != i
                || restored.accounts.get(*h).map_err(fail)?.id as usize != i + 1
            {
                return Err("native_account_index".into());
            }
        }
        for (i, h) in restored.inventory_index.iter().enumerate() {
            let stock = restored.inventory.get(*h).map_err(fail)?;
            if h.raw().index as usize != i
                || stock.id as usize != i + 1
                || stock.owner != restored.account_index[i % 40000]
            {
                return Err("native_stock_index".into());
            }
        }
        if restored.manifest().owned_capacity_bytes > 64 * 1024 * 1024 {
            return Err("native_layout_budget".into());
        }
        Ok((restored, bound))
    }
    pub fn fixed_restore_bound() -> Result<RestoreBound, String> {
        let arrays = [
            (40000usize, size_of::<Handle<AccountDomain>>()),
            (110000, size_of::<Handle<InventoryDomain>>()),
            (180000, size_of::<Claim>()),
            (31200, size_of::<Payroll>()),
            (8, size_of::<Resource>()),
            (64, size_of::<Lease>()),
        ];
        let mut bytes = size_of::<Self>();
        for part in [
            Arena::<AccountDomain, Account>::layout_restore_bytes(40000),
            Arena::<InventoryDomain, Stock>::layout_restore_bytes(110000),
        ] {
            bytes = bytes
                .checked_add(part.ok_or("native_restore_overflow")?)
                .ok_or("native_restore_overflow")?;
        }
        for (count, width) in arrays {
            bytes = bytes
                .checked_add(count.checked_mul(width).ok_or("native_restore_overflow")?)
                .ok_or("native_restore_overflow")?;
        }
        let bound = RestoreBound {
            decoded_owned_bytes: bytes,
            largest_replacement_bytes: 0,
            parser_io_stack_slack_bytes: 6 * 1024 * 1024,
            conservative_transient_bound_bytes: bytes + 6 * 1024 * 1024,
        };
        if bound.conservative_transient_bound_bytes > crate::fixed_restore::RESTORE_BYTES {
            return Err("native_restore_work_bound_exceeded".into());
        }
        Ok(bound)
    }
    pub fn write_checkpoint(&self, writer: impl std::io::Write) -> Result<(), String> {
        serde_json::to_writer(writer, self).map_err(fail)
    }
}
