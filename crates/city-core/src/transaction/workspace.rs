//! Preallocated preparation tables, leased before use and returned on every exit.
use super::*;
use crate::model_rows::{Record, Resident, Rows};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};
type Image = Resident<crate::timeline::Timeline>;
type BackgroundImage = Resident<crate::background::Background>;
pub(super) type BackgroundLease = Lease<BackgroundImage>;
pub(super) type BackgroundWorkLease = Lease<crate::background::Work>;
pub(super) type Owner<T> = Arc<Mutex<Option<T>>>;
pub(super) type TimelineWorkspace = Owner<Image>;
pub(super) type TimelineLease = Lease<Image>;
pub(super) type AccountLease = Lease<DirtyAccounts>;
pub(super) type StockLease = Lease<DirtyStocks>;
pub(super) type ResourceLease = Lease<ResourceRows>;
pub(super) type CapacityLease = Lease<LeaseRows>;
pub(super) type SpatialPermissionOwner =
    Owner<crate::spatial_transport_permissions::SpatialPermissionState>;
pub(super) type SpatialPermissionLease =
    Lease<crate::spatial_transport_permissions::SpatialPermissionState>;
pub(super) struct PreparationWorkspace {
    pub timeline: TimelineWorkspace,
    pub background: Owner<BackgroundImage>,
    pub background_work: Owner<crate::background::Work>,
    pub accounts: Owner<DirtyAccounts>,
    pub stocks: Owner<DirtyStocks>,
    pub resources: Owner<ResourceRows>,
    pub leases: Owner<LeaseRows>,
    /// Fixed backing used for the next permission-state candidate. It is allocated only for
    /// worlds with the spatial-permission feature enabled.
    pub spatial_permissions: SpatialPermissionOwner,
}
impl PreparationWorkspace {
    pub(super) fn backing_ids(&self) -> Result<[usize; 28], String> {
        let t = self.timeline.lock().map_err(|_| "timeline_slot_poison")?;
        let b = self
            .background
            .lock()
            .map_err(|_| "background_slot_poison")?;
        let w = self
            .background_work
            .lock()
            .map_err(|_| "background_work_poison")?;
        let a = self.accounts.lock().map_err(|_| "accounts_slot_poison")?;
        let s = self.stocks.lock().map_err(|_| "stocks_slot_poison")?;
        let r = self.resources.lock().map_err(|_| "resources_slot_poison")?;
        let l = self.leases.lock().map_err(|_| "leases_slot_poison")?;
        let p = self
            .spatial_permissions
            .lock()
            .map_err(|_| "spatial_permissions_slot_poison")?;
        let t = t.as_ref().ok_or("timeline_slot_busy")?.backing();
        let b = b
            .as_ref()
            .ok_or("background_slot_busy")?
            .backing()
            .backing_ids();
        let w = w.as_ref().ok_or("background_work_busy")?.backing_ids();
        Ok([
            t.claims.allocation(),
            t.payroll.allocation(),
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            b[6],
            b[7],
            w[0],
            w[1],
            w[2],
            w[3],
            w[4],
            w[5],
            a.as_ref().ok_or("accounts_slot_busy")?.allocation(),
            s.as_ref().ok_or("stocks_slot_busy")?.allocation(),
            r.as_ref().ok_or("resources_slot_busy")?.allocation(),
            l.as_ref().ok_or("leases_slot_busy")?.allocation(),
            Arc::as_ptr(&self.timeline) as usize,
            Arc::as_ptr(&self.background) as usize,
            Arc::as_ptr(&self.background_work) as usize,
            Arc::as_ptr(&self.accounts) as usize,
            Arc::as_ptr(&self.stocks) as usize,
            Arc::as_ptr(&self.resources) as usize,
            Arc::as_ptr(&self.leases) as usize,
            p.as_ref().map_or(0, |state| state as *const _ as usize),
        ])
    }
}
/// Current per-Engine retained preparation storage, not just one active Prepared.
/// 256B/slot conservatively covers Arc counts/alignment; not allocator/RSS evidence.
pub(super) fn capacity_inventory() -> (usize, usize, usize) {
    fn slot<T>() -> usize {
        size_of::<Mutex<Option<T>>>() + 256
    }
    let controls = slot::<Image>()
        + slot::<BackgroundImage>()
        + slot::<crate::background::Work>()
        + slot::<DirtyAccounts>()
        + slot::<DirtyStocks>()
        + slot::<ResourceRows>()
        + slot::<LeaseRows>()
        + slot::<crate::spatial_transport_permissions::SpatialPermissionState>();
    let tables = crate::timeline::CLAIM_LIMIT * size_of::<crate::timeline::Claim>()
        + crate::timeline::PAYROLL_LIMIT * size_of::<crate::timeline::Payroll>()
        + crate::timeline::MAX_CLAIM_SOURCES * size_of::<crate::timeline::ClaimSourceRecord>()
        + crate::assembly::MAX_ACCOUNTS * size_of::<(Handle<AccountDomain>, Account)>()
        + crate::assembly::MAX_STOCKS * size_of::<(Handle<InventoryDomain>, Stock)>()
        + MAX_RESOURCES * size_of::<Resource>()
        + MAX_LEASES * size_of::<Option<super::Lease>>()
        + crate::spatial_transport_permissions::SpatialPermissionState::max_owned_capacity_bytes();
    let air_payload = 128 * 128 * 16
        + crate::background::SOURCE_LIMIT * size_of::<crate::air::Source>()
        + crate::air::STRATEGY.len()
        + 64;
    let background = air_payload
        + crate::background::SOURCE_LIMIT * size_of::<crate::air::Source>()
        + crate::air::STRATEGY.len();
    (tables + background, air_payload, controls)
}
pub(super) fn reserve_timeline() -> TimelineWorkspace {
    Arc::new(Mutex::new(Some(Image::empty())))
}
pub(super) fn reserve() -> Result<PreparationWorkspace, TxError> {
    fn rows<T: Record, const N: usize>() -> Result<Owner<Rows<T, N>>, TxError> {
        Ok(Arc::new(Mutex::new(Some(
            Rows::new().map_err(|_| TxError::WorkingLimit)?,
        ))))
    }
    Ok(PreparationWorkspace {
        timeline: reserve_timeline(),
        background: Arc::new(Mutex::new(Some(BackgroundImage::empty()))),
        background_work: Arc::new(Mutex::new(Some(crate::background::Work::new()))),
        accounts: rows()?,
        stocks: rows()?,
        resources: rows()?,
        leases: rows()?,
        spatial_permissions: Arc::new(Mutex::new(None)),
    })
}
pub(super) trait Reset {
    fn reset_storage(&mut self);
}
impl Reset for crate::background::Work {
    fn reset_storage(&mut self) {
        self.reset();
    }
}
impl Reset for BackgroundImage {
    fn reset_storage(&mut self) {
        self.deactivate();
    }
}
impl Reset for crate::spatial_transport_permissions::SpatialPermissionState {
    fn reset_storage(&mut self) {
        self.clear_preserving_capacity();
    }
}
impl Lease<BackgroundImage> {
    pub fn acquire(
        owner: &Owner<BackgroundImage>,
        source: &BackgroundImage,
    ) -> Result<Self, TxError> {
        let mut lease = Self::take(owner)?;
        lease.deref_mut().clone_from(source);
        Ok(lease)
    }
}
impl Reset for Image {
    fn reset_storage(&mut self) {
        self.deactivate();
    }
}
impl<T: Record, const N: usize> Reset for Rows<T, N> {
    fn reset_storage(&mut self) {
        self.clear();
    }
}
pub(super) struct Lease<T: Reset> {
    owner: Owner<T>,
    value: Option<T>,
}
impl<T: Reset> Lease<T> {
    pub fn take(owner: &Owner<T>) -> Result<Self, TxError> {
        let value = owner
            .try_lock()
            .map_err(|_| TxError::WorkingLimit)?
            .take()
            .ok_or(TxError::WorkingLimit)?;
        Ok(Self {
            owner: Arc::clone(owner),
            value: Some(value),
        })
    }
}
impl Lease<Image> {
    pub fn acquire(owner: &TimelineWorkspace, source: &Image) -> Result<Self, TxError> {
        let mut lease = Self::take(owner)?;
        lease.deref_mut().clone_from(source);
        Ok(lease)
    }
}
impl<T: Reset> Deref for Lease<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value.as_ref().expect("owned preparation lease")
    }
}
impl<T: Reset> DerefMut for Lease<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("owned preparation lease")
    }
}
impl<T: Reset> Drop for Lease<T> {
    fn drop(&mut self) {
        if let Some(mut value) = self.value.take() {
            value.reset_storage();
            let mut slot = self.owner.lock().unwrap_or_else(|p| p.into_inner());
            debug_assert!(slot.is_none());
            *slot = Some(value);
        }
    }
}
impl<'a, T: Record, const N: usize> IntoIterator for &'a Lease<Rows<T, N>> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Copy> Record for Option<T> {
    fn inactive() -> Self {
        None
    }
}
impl Record for Resource {
    fn inactive() -> Self {
        Self {
            id: 0,
            capacity: 0,
            reserved: 0,
            occupied: 0,
        }
    }
}
impl Record for HotJournalRecord {
    fn inactive() -> Self {
        Self {
            actor: 0,
            emergency_eligible: false,
            operation_id: 0,
            expected_revision: 0,
            payload_sha256: [0; 32],
            outcome: Err(TxError::InvalidOperation),
            actions: [None; MAX_ACTIONS],
        }
    }
}
impl Record for (Handle<AccountDomain>, Account) {
    fn inactive() -> Self {
        (
            Handle::inactive(),
            Account {
                id: 0,
                role: crate::assembly::Role::Treasury,
                cash: Money::ZERO,
                protected: Money::ZERO,
                opening_source_id: 0,
            },
        )
    }
}
impl Record for (Handle<InventoryDomain>, Stock) {
    fn inactive() -> Self {
        (
            Handle::inactive(),
            Stock {
                id: 0,
                owner: Handle::inactive(),
                commodity: crate::assembly::Commodity::Raw,
                quantity: Quantity::ZERO,
                basis: Money::ZERO,
                lot_id: 0,
                origin_quantity: Quantity::ZERO,
                origin_basis: Money::ZERO,
                offset: Quantity::ZERO,
                opening_source_id: 0,
                opening_basis: Money::ZERO,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> Engine {
        Engine::new(
            crate::assembly::assemble(include_bytes!("../../../../fixtures/m0-e0-opening.json"))
                .unwrap(),
            vec![],
        )
        .unwrap()
    }
    #[test]
    fn reentry_is_rejected_and_unwind_returns_same_initialized_rows() {
        let owner = reserve_timeline();
        let source = Image::empty();
        let id = owner
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .backing()
            .claims
            .allocation();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut lease = TimelineLease::acquire(&owner, &source).unwrap();
            lease.activate().reset_at(86400);
            assert!(matches!(
                TimelineLease::acquire(&owner, &source),
                Err(TxError::WorkingLimit)
            ));
            panic!("prepare callback unwind");
        }));
        assert!(failure.is_err());
        let lease = TimelineLease::acquire(&owner, &source).unwrap();
        assert_eq!(lease.backing().claims.allocation(), id);
        assert!(lease.backing().claims.full_fill_valid());
        assert!(!lease.is_some());
    }
    #[test]
    fn failed_or_unwound_hook_preserves_authority_then_commit_swaps_backing() {
        let mut e = engine();
        let before = e.state_hash();
        let bg_owner = Arc::clone(&e.preparation_workspace.as_ref().unwrap().background);
        let bg_authority = e.background.backing().allocation_set();
        let bg_scratch = bg_owner
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .backing()
            .allocation_set();
        let air_owner = Arc::clone(&e.preparation_workspace.as_ref().unwrap().background_work);
        {
            let lease = BackgroundWorkLease::take(&air_owner).unwrap();
            assert!(matches!(
                e.prepare_diagnosed(
                    Caller::System,
                    &Envelope {
                        world: 1,
                        branch: 1,
                        operation_id: 999,
                        expected_revision: 0,
                        actions: vec![Action::Timeline {
                            command: crate::timeline::Command::Enable
                        }]
                    },
                    None,
                    None,
                    &mut Trace::disabled()
                ),
                Err(TxError::WorkingLimit)
            ));
            drop(lease);
        }
        assert_eq!(e.background.backing().allocation_set(), bg_authority);
        let owner = Arc::clone(&e.preparation_workspace.as_ref().unwrap().timeline);
        let authority = e.timeline.backing().claims.allocation();
        let scratch = owner
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .backing()
            .claims
            .allocation();
        let env = Envelope {
            world: 1,
            branch: 1,
            operation_id: 1,
            expected_revision: 0,
            actions: vec![Action::Timeline {
                command: crate::timeline::Command::Enable,
            }],
        };
        assert_eq!(
            e.execute_recorded(Caller::System, env.clone(), |_, _| Err("durability")),
            Err("durability")
        );
        assert_eq!(e.state_hash(), before);
        assert!(e.journal.is_empty());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = e.execute_recorded::<()>(Caller::System, env.clone(), |_, _| {
                panic!("persist unwind")
            });
        }))
        .is_err());
        assert_eq!(e.state_hash(), before);
        assert!(e.journal.is_empty());
        assert_eq!(
            owner
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .backing()
                .claims
                .allocation(),
            scratch
        );
        e.execute(Caller::System, env).unwrap();
        assert!(e.timeline.is_some());
        assert_eq!(e.timeline.backing().claims.allocation(), scratch);
        assert_eq!(e.background.backing().allocation_set(), bg_scratch);
        assert_eq!(
            bg_owner
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .backing()
                .allocation_set(),
            bg_authority
        );
        assert!(BackgroundWorkLease::take(&air_owner).is_ok());
        let back = owner.lock().unwrap();
        let returned = back.as_ref().unwrap();
        assert_eq!(returned.backing().claims.allocation(), authority);
        assert!(returned.backing().claims.full_fill_valid());
        assert!(!returned.is_some());
    }
}
