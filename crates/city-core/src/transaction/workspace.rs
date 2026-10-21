//! Preallocated preparation tables, leased before use and returned on every exit.
use super::*;
use crate::model_rows::{Record, Resident, Rows};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};
type Image = Resident<crate::timeline::Timeline>;
type Owner<T> = Arc<Mutex<Option<T>>>;
pub(super) type TimelineWorkspace = Owner<Image>;
pub(super) type TimelineLease = Lease<Image>;
pub(super) type AccountLease = Lease<DirtyAccounts>;
pub(super) type StockLease = Lease<DirtyStocks>;
pub(super) type ResourceLease = Lease<ResourceRows>;
pub(super) type CapacityLease = Lease<LeaseRows>;
pub(super) struct PreparationWorkspace {
    pub timeline: TimelineWorkspace,
    pub accounts: Owner<DirtyAccounts>,
    pub stocks: Owner<DirtyStocks>,
    pub resources: Owner<ResourceRows>,
    pub leases: Owner<LeaseRows>,
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
        accounts: rows()?,
        stocks: rows()?,
        resources: rows()?,
        leases: rows()?,
    })
}
pub(super) trait Reset {
    fn reset_storage(&mut self);
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
impl Record for JournalRecord {
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
        let back = owner.lock().unwrap();
        let returned = back.as_ref().unwrap();
        assert_eq!(returned.backing().claims.allocation(), authority);
        assert!(returned.backing().claims.full_fill_valid());
        assert!(!returned.is_some());
    }
}
