//! Two real, pre-touched worker byte slots. This is a local controlled owner,
//! not a claim that the cross-process 64MiB pool or State/serde is accounted.
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
pub const SLOT_BYTES: usize = crate::worker_cache::ASSET_BYTES;
pub const OWNER_BYTES: usize = 2 * SLOT_BYTES;
const FILL: u8 = 0xa5;
static NEXT_OWNER: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Input,
    Candidate,
}
impl Kind {
    fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::Candidate => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub process_pid: u32,
    pub process_instance_unix_ns: u128,
    pub run_epoch: u64,
    pub owner_instance: u64,
    pub job: u64,
    pub generation: u32,
    pub attempt: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Ticket {
    identity: Identity,
    slot: Kind,
    sequence: u64,
}
/// No owning storage, Clone, Arc or Vec escapes the scoped operation.
pub struct Bytes {
    data: Vec<u8>,
    used: usize,
    high_water: usize,
}
impl Bytes {
    fn new() -> Result<Self, String> {
        let mut data = Vec::new();
        data.try_reserve_exact(SLOT_BYTES)
            .map_err(|e| e.to_string())?;
        if data.capacity() != SLOT_BYTES {
            return Err("worker_slot_capacity".into());
        }
        data.resize(SLOT_BYTES, FILL);
        Ok(Self {
            data,
            used: 0,
            high_water: 0,
        })
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.used]
    }
    pub fn read_from(&mut self, mut source: impl Read) -> Result<usize, String> {
        self.used = 0;
        while self.used < SLOT_BYTES {
            match source.read(&mut self.data[self.used..]) {
                Ok(0) => return Ok(self.used),
                Ok(n) => {
                    self.used += n;
                    self.high_water = self.high_water.max(self.used);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e.to_string()),
            }
        }
        let mut extra = [0u8; 1];
        loop {
            match source.read(&mut extra) {
                Ok(0) => return Ok(self.used),
                Ok(_) => return Err("worker_slot_bytes".into()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    fn reset(&mut self) {
        self.data.fill(FILL);
        self.used = 0;
        self.high_water = 0;
    }
}
impl Write for Bytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.used.saturating_add(bytes.len()) > SLOT_BYTES {
            return Err(io::Error::other("worker_slot_bytes"));
        }
        self.data[self.used..self.used + bytes.len()].copy_from_slice(bytes);
        self.used += bytes.len();
        self.high_water = self.high_water.max(self.used);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Slot {
    bytes: Bytes,
    ticket: Option<Ticket>,
    claims: u64,
    returns: u64,
    maximum_payload_bytes: usize,
    poisoned: bool,
}
impl Slot {
    fn new() -> Result<Self, String> {
        Ok(Self {
            bytes: Bytes::new()?,
            ticket: None,
            claims: 0,
            returns: 0,
            maximum_payload_bytes: 0,
            poisoned: false,
        })
    }
    fn give_back(&mut self, ticket: Ticket) -> Result<(), String> {
        if self.ticket != Some(ticket) {
            return Err("worker_slot_stale_or_wrong_ticket".into());
        }
        self.maximum_payload_bytes = self.maximum_payload_bytes.max(self.bytes.high_water);
        self.bytes.reset();
        self.ticket = None;
        self.returns = self.returns.checked_add(1).ok_or("worker_slot_counter")?;
        Ok(())
    }
}
struct Loan<'a> {
    slot: &'a mut Slot,
    ticket: Ticket,
}
impl Drop for Loan<'_> {
    fn drop(&mut self) {
        if self.slot.give_back(self.ticket).is_err() {
            self.slot.poisoned = true;
        }
    }
}
#[derive(Clone, Copy, Serialize)]
struct Transition {
    ticket: Ticket,
    borrowed: bool,
    observed_ns: u64,
    work_succeeded: Option<bool>,
}
#[derive(Serialize)]
pub struct SlotReport {
    slot: Kind,
    capacity_bytes: usize,
    pub available_touched_bytes: usize,
    claims: u64,
    returns: u64,
    maximum_payload_bytes: usize,
}
#[derive(Serialize)]
pub struct Report<'a> {
    schema: &'static str,
    identity: Identity,
    pub owner_capacity_bytes: usize,
    pub available_touched_bytes: usize,
    lent_capacity_bytes: usize,
    controlled_metadata_charge_bytes: usize,
    pub full_balance_scan_passed: bool,
    slots: [SlotReport; 2],
    transitions: &'a [Option<Transition>],
    scope: &'static str,
}
pub struct Owner {
    identity: Identity,
    slots: [Slot; 2],
    next: u64,
    began: Instant,
    events: [Option<Transition>; 8],
    event_count: usize,
}
impl Owner {
    pub fn new(run_epoch: u64, job: u64, generation: u32, attempt: u8) -> Result<Self, String> {
        if run_epoch == 0 || job == 0 || generation == 0 || !(1..=3).contains(&attempt) {
            return Err("worker_slot_identity".into());
        }
        let owner_instance = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "worker_owner_counter")?;
        Ok(Self {
            identity: Identity {
                owner_instance,
                process_pid: std::process::id(),
                process_instance_unix_ns: crate::heap_meter::process_instance(),
                run_epoch,
                job,
                generation,
                attempt,
            },
            slots: [Slot::new()?, Slot::new()?],
            next: 1,
            began: Instant::now(),
            events: [None; 8],
            event_count: 0,
        })
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    /// Reuse both already-touched byte slots across jobs in the SAME process.
    /// A new identity/trace is published; no allocation is dropped or duplicated.
    pub fn rebind(
        &mut self,
        run_epoch: u64,
        job: u64,
        generation: u32,
        attempt: u8,
    ) -> Result<(), String> {
        if run_epoch == 0 || job == 0 || generation == 0 || !(1..=3).contains(&attempt) {
            return Err("worker_slot_identity".into());
        }
        self.check_balance()?;
        let owner_instance = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "worker_owner_counter")?;
        self.identity = Identity {
            process_pid: std::process::id(),
            process_instance_unix_ns: crate::heap_meter::process_instance(),
            run_epoch,
            owner_instance,
            job,
            generation,
            attempt,
        };
        for slot in &mut self.slots {
            slot.claims = 0;
            slot.returns = 0;
            slot.maximum_payload_bytes = 0;
        }
        self.next = 1;
        self.began = Instant::now();
        self.events = [None; 8];
        self.event_count = 0;
        Ok(())
    }
    pub fn metadata_charge_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
    fn event(&mut self, ticket: Ticket, borrowed: bool, work_succeeded: Option<bool>) {
        self.events[self.event_count] = Some(Transition {
            ticket,
            borrowed,
            observed_ns: self.began.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            work_succeeded,
        });
        self.event_count += 1;
    }
    /// The result type is independent of the loan lifetime, so a storage slice
    /// cannot escape. The loan returns and re-touches even during unwinding.
    /// ```compile_fail
    /// use mycity_headless::worker_slots::{Owner,Kind};
    /// let mut owner=Owner::new(1,1,1,1).unwrap();
    /// let escaped=owner.with_slot(Kind::Input,|bytes|Ok(bytes.as_slice())).unwrap();
    /// println!("{}",escaped.len());
    /// ```
    pub fn with_slot<T>(
        &mut self,
        kind: Kind,
        work: impl FnOnce(&mut Bytes) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.event_count + 2 > self.events.len() {
            return Err("worker_slot_trace_limit".into());
        }
        let next = self.next.checked_add(1).ok_or("worker_slot_counter")?;
        let ticket = Ticket {
            identity: self.identity,
            slot: kind,
            sequence: self.next,
        };
        let slot = &mut self.slots[kind.index()];
        if slot.poisoned || slot.ticket.is_some() {
            return Err("worker_slot_busy_or_poisoned".into());
        }
        slot.claims = slot.claims.checked_add(1).ok_or("worker_slot_counter")?;
        slot.ticket = Some(ticket);
        self.next = next;
        self.event(ticket, true, None);
        let result = {
            let loan = Loan {
                slot: &mut self.slots[kind.index()],
                ticket,
            };
            work(&mut loan.slot.bytes)
        };
        self.event(ticket, false, Some(result.is_ok()));
        self.check_balance()?;
        result
    }
    /// Two simultaneous fixed loans; neither slice can escape the closure.
    pub fn with_pair<T>(
        &mut self,
        work: impl FnOnce(&mut Bytes, &mut Bytes) -> Result<T, String>,
    ) -> Result<T, String> {
        self.check_balance()?;
        if self.event_count + 4 > self.events.len() {
            return Err("worker_slot_trace_limit".into());
        }
        let next = self.next.checked_add(2).ok_or("worker_slot_counter")?;
        let tickets = [Kind::Input, Kind::Candidate].map(|slot| Ticket {
            identity: self.identity,
            slot,
            sequence: self.next + slot.index() as u64,
        });
        for (slot, ticket) in self.slots.iter_mut().zip(tickets) {
            slot.claims = slot.claims.checked_add(1).ok_or("worker_slot_counter")?;
            slot.ticket = Some(ticket);
        }
        self.next = next;
        for t in tickets {
            self.event(t, true, None);
        }
        let result = {
            let [a, b] = &mut self.slots;
            let a = Loan {
                slot: a,
                ticket: tickets[0],
            };
            let b = Loan {
                slot: b,
                ticket: tickets[1],
            };
            work(&mut a.slot.bytes, &mut b.slot.bytes)
        };
        for t in tickets {
            self.event(t, false, Some(result.is_ok()));
        }
        self.check_balance()?;
        result
    }
    pub fn check_balance(&self) -> Result<(), String> {
        for slot in &self.slots {
            if slot.poisoned
                || slot.ticket.is_some()
                || slot.claims != slot.returns
                || slot.bytes.used != 0
                || slot.bytes.data.len() != SLOT_BYTES
                || slot.bytes.data.capacity() != SLOT_BYTES
                || !slot.bytes.data.iter().all(|b| *b == FILL)
            {
                return Err("worker_slot_unbalanced".into());
            }
        }
        Ok(())
    }
    pub fn report(&self) -> Result<Report<'_>, String> {
        self.check_balance()?;
        if self.event_count % 2 != 0 {
            return Err("worker_slot_incomplete_trace".into());
        }
        let mut live = [None; 2];
        for e in self.events[..self.event_count].iter().flatten() {
            let slot = &mut live[e.ticket.slot.index()];
            if e.borrowed {
                if slot.is_some() {
                    return Err("worker_slot_trace".into());
                }
                *slot = Some((e.ticket, e.observed_ns));
            } else {
                let (ticket, began) = slot.take().ok_or("worker_slot_trace")?;
                if ticket != e.ticket || e.observed_ns < began {
                    return Err("worker_slot_trace".into());
                }
            }
        }
        if live.iter().any(Option::is_some) {
            return Err("worker_slot_trace".into());
        }
        let slot_report = |kind: Kind| {
            let s = &self.slots[kind.index()];
            SlotReport {
                slot: kind,
                capacity_bytes: SLOT_BYTES,
                available_touched_bytes: SLOT_BYTES,
                claims: s.claims,
                returns: s.returns,
                maximum_payload_bytes: s.maximum_payload_bytes,
            }
        };
        Ok(Report { schema:"mycity-worker-slots-v1",identity:self.identity,owner_capacity_bytes:OWNER_BYTES,available_touched_bytes:OWNER_BYTES,lent_capacity_bytes:0,controlled_metadata_charge_bytes:self.metadata_charge_bytes(),full_balance_scan_passed:true,slots:[slot_report(Kind::Input),slot_report(Kind::Candidate)],transitions:&self.events[..self.event_count],scope:"local actual worker input/candidate capacity only; not State/serde/IPC/native or global64MiB closure" })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_capacity_prefix_and_one_byte_overflow_never_grow_or_publish_tail() {
        let mut owner = Owner::new(1, 1, 1, 1).unwrap();
        owner
            .with_slot(Kind::Input, |b| {
                b.read_from(io::Cursor::new(vec![9; SLOT_BYTES]))?;
                assert_eq!(b.as_slice().len(), SLOT_BYTES);
                assert!(b.as_slice().iter().all(|v| *v == 9));
                Ok(())
            })
            .unwrap();
        owner
            .with_slot(Kind::Candidate, |b| {
                b.write_all(&vec![7; 70000]).unwrap();
                b.write_all(&vec![7; 60000]).unwrap();
                b.write_all(&vec![7; SLOT_BYTES - 130000]).unwrap();
                assert!(b.write_all(&[1]).is_err());
                assert_eq!(b.data.capacity(), SLOT_BYTES);
                assert!(b.as_slice().iter().all(|v| *v == 7));
                Ok(())
            })
            .unwrap();
        owner.check_balance().unwrap();
        assert_eq!(owner.report().unwrap().owner_capacity_bytes, OWNER_BYTES);
    }
    #[test]
    fn read_failure_encoding_failure_and_unwind_restore_touched_balance() {
        let mut owner = Owner::new(1, 1, 1, 1).unwrap();
        assert!(owner
            .with_slot(Kind::Input, |b| b.read_from(io::Cursor::new(vec![
                0;
                SLOT_BYTES
                    + 1
            ])))
            .is_err());
        assert!(owner
            .with_slot::<()>(Kind::Candidate, |b| {
                b.write_all(b"unpublished prefix").unwrap();
                Err("writer_failed".into())
            })
            .is_err());
        owner.check_balance().unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = owner.with_slot::<()>(Kind::Input, |_| panic!("fault"));
        }))
        .is_err());
        owner.check_balance().unwrap();
        assert!(owner.report().is_err());
    }
    #[test]
    fn duplicate_wrong_slot_old_epoch_and_generation_cannot_refund_a_live_slot() {
        let mut owner = Owner::new(3, 7, 2, 1).unwrap();
        let ticket = Ticket {
            identity: owner.identity,
            slot: Kind::Input,
            sequence: 1,
        };
        owner.slots[0].ticket = Some(ticket);
        owner.slots[0].claims = 1;
        let mut bad = ticket;
        bad.identity.run_epoch -= 1;
        assert!(owner.slots[0].give_back(bad).is_err());
        bad = ticket;
        bad.identity.generation -= 1;
        assert!(owner.slots[0].give_back(bad).is_err());
        bad = ticket;
        bad.identity.owner_instance = u64::MAX;
        assert!(owner.slots[0].give_back(bad).is_err());
        assert!(owner.slots[1].give_back(ticket).is_err());
        assert_eq!(owner.slots[0].ticket, Some(ticket));
        owner.slots[0].give_back(ticket).unwrap();
        assert!(owner.slots[0].give_back(ticket).is_err());
        owner.check_balance().unwrap();
    }
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    #[test]
    fn rebind_reuses_the_same_two_allocations_and_rejects_busy_slots() {
        let mut o = Owner::new(1, 1, 1, 1).unwrap();
        let pointers = [
            o.slots[0].bytes.data.as_ptr(),
            o.slots[1].bytes.data.as_ptr(),
        ];
        let id = o.identity();
        o.with_slot(Kind::Input, |b| {
            b.write_all(b"actual bytes").map_err(|e| e.to_string())?;
            Ok(())
        })
        .unwrap();
        o.rebind(2, 3, 4, 2).unwrap();
        assert_ne!(o.identity().owner_instance, id.owner_instance);
        assert_eq!(o.identity().job, 3);
        assert_eq!(
            [
                o.slots[0].bytes.data.as_ptr(),
                o.slots[1].bytes.data.as_ptr()
            ],
            pointers
        );
        assert_eq!(o.slots[0].claims, 0);
        o.check_balance().unwrap();
        assert!(o.rebind(0, 3, 4, 2).is_err());
        o.slots[0].poisoned = true;
        assert!(o.rebind(2, 4, 4, 2).is_err());
    }
    #[test]
    fn repeated_reads_in_one_loan_preserve_payload_high_water_and_reset_backing() {
        let mut o = Owner::new(1, 1, 1, 1).unwrap();
        o.with_pair(|a, b| {
            a.read_from(io::repeat(7).take(SLOT_BYTES as u64))?;
            a.read_from(io::Cursor::new([8]))?;
            b.read_from(io::Cursor::new([1, 2]))?;
            b.read_from(io::Cursor::new([]))?;
            Ok(())
        })
        .unwrap();
        let r = o.report().unwrap();
        assert_eq!(r.slots[0].maximum_payload_bytes, SLOT_BYTES);
        assert_eq!(r.slots[1].maximum_payload_bytes, 2);
        o.check_balance().unwrap();
        o.with_slot(Kind::Input, |a| {
            a.read_from(io::Cursor::new([0]))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            o.report().unwrap().slots[0].maximum_payload_bytes,
            SLOT_BYTES
        );
    }
}
