//! Finite, contract-derived facts. Physical delivery and receipt are committed together.
use crate::{
    economy::Fragment,
    exchange::{Exchange, Ledger, Status},
    transaction::TxError,
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
pub const NORMAL: usize = 4;
pub const TOTAL: usize = 12;
pub const RETAINED: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Delivery,
    Incident,
    External,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalReason {
    EmergencyRepair,
    ContractAward,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalFact {
    pub source: u32,
    pub event: u32,
    pub debtor: u32,
    pub creditor: u32,
    pub amount: Money,
    pub due: u64,
    pub reason: ExternalReason,
}
impl ExternalFact {
    pub fn operation_id(self) -> u64 {
        0xf000000000000000 | u64::from(self.event)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub sequence: u64,
    pub quote: u32,
    pub kind: Kind,
    pub received_at: Option<u64>,
    pub debtor: u32,
    pub creditor: u32,
    pub manager: u32,
    pub handoffs: u32,
    pub fee: Money,
    pub paid: Money,
    pub quantity: Quantity,
    pub invoice: Money,
    pub fragment: Option<Fragment>,
    pub external: Option<ExternalFact>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inbox {
    pub enabled: bool,
    pub facts: [Option<Fact>; TOTAL],
    pub retained: [Option<Fact>; RETAINED],
    pub external_next: u64,
    pub next_sequence: u64,
    pub incident_seen: [bool; 8],
    pub accrued: Money,
    pub paid: Money,
    pub closed: u64,
}
impl Default for Inbox {
    fn default() -> Self {
        Self {
            enabled: false,
            facts: [None; TOTAL],
            retained: [None; RETAINED],
            external_next: 1,
            next_sequence: 1,
            incident_seen: [false; 8],
            accrued: Money::ZERO,
            paid: Money::ZERO,
            closed: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Enable,
    External { fact: ExternalFact },
    Receive { quote: u32 },
    Incident { quote: u32 },
    Settle { sequence: u64 },
    Handoff { sequence: u64, manager: u32 },
    Archive { sequence: u64 },
}
fn plus(a: Money, b: Money) -> Result<Money, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
impl Inbox {
    pub fn all(&self) -> impl Iterator<Item = &Fact> {
        self.facts.iter().chain(self.retained.iter()).flatten()
    }
    fn all_mut(&mut self) -> impl Iterator<Item = &mut Fact> {
        self.facts
            .iter_mut()
            .chain(self.retained.iter_mut())
            .flatten()
    }
    pub fn admission_full(&self) -> bool {
        self.enabled && self.facts.iter().flatten().count() >= NORMAL
    }
    pub(crate) fn admit(&mut self, quote: u32, buyer: u32, carrier: u32) -> Result<(), TxError> {
        if !self.enabled {
            return Ok(());
        }
        if self.admission_full() {
            return Err(TxError::InboxFull);
        }
        let index = self
            .facts
            .iter()
            .position(Option::is_none)
            .ok_or(TxError::InboxFull)?;
        let sequence = self.next_sequence;
        self.next_sequence = sequence.checked_add(1).ok_or(TxError::Overflow)?;
        self.facts[index] = Some(Fact {
            sequence,
            quote,
            kind: Kind::Delivery,
            received_at: None,
            debtor: buyer,
            creditor: carrier,
            manager: carrier,
            handoffs: 0,
            fee: Money::ZERO,
            paid: Money::ZERO,
            quantity: Quantity::ZERO,
            invoice: Money::ZERO,
            fragment: None,
            external: None,
        });
        Ok(())
    }
    pub(crate) fn delivered(
        &mut self,
        quote: u32,
        at: u64,
        quantity: Quantity,
        invoice: Money,
        fragment: Fragment,
    ) -> Result<(), TxError> {
        if !self.enabled {
            return Ok(());
        }
        let f = self
            .facts
            .iter_mut()
            .flatten()
            .find(|f| f.quote == quote && f.kind == Kind::Delivery)
            .ok_or(TxError::InboxFull)?;
        if f.received_at.is_some() {
            return Err(TxError::InvalidOperation);
        }
        f.received_at = Some(at);
        f.quantity = quantity;
        f.invoice = invoice;
        f.fragment = Some(fragment);
        // Explicit laboratory handling contract, five cents per completed delivery.
        f.fee = Money::new(5).expect("fixed fee");
        self.accrued = plus(self.accrued, f.fee)?;
        Ok(())
    }
    pub(crate) fn validate(&self) -> Result<(), TxError> {
        let mut outstanding = Money::ZERO;
        for (i, f) in self.all().enumerate() {
            if f.sequence == 0
                || f.sequence >= self.next_sequence
                || f.paid > f.fee
                || self.all().take(i).any(|g| {
                    g.sequence == f.sequence
                        || (g.quote == f.quote
                            && g.kind == f.kind
                            && (f.kind != Kind::External || g.external == f.external))
                })
            {
                return Err(TxError::InvalidOperation);
            }
            outstanding = plus(
                outstanding,
                f.fee.checked_sub(f.paid).map_err(|_| TxError::Overflow)?,
            )?;
        }
        if plus(outstanding, self.paid)? != self.accrued {
            return Err(TxError::InvalidOperation);
        }
        Ok(())
    }
}

impl Exchange {
    pub(crate) fn inbox_command(
        &mut self,
        command: Command,
        time: u64,
        l: &mut impl Ledger,
    ) -> Result<(), TxError> {
        if command == Command::Enable {
            if self.inbox.enabled {
                return Ok(());
            }
            if self.quotes.iter().any(|q| q.status != Status::Pending) {
                return Err(TxError::InvalidOperation);
            }
            self.inbox.enabled = true;
            return Ok(());
        }
        if !self.inbox.enabled {
            return Err(TxError::InvalidOperation);
        }
        match command {
            Command::Enable => unreachable!(),
            Command::Receive { quote } => {
                // Canonical fact key is (scope, quote, Delivery), retained by the contract even
                // after its working slot is archived. Re-notification never accrues another fee.
                if !self
                    .quotes
                    .iter()
                    .any(|q| q.spec.id == quote && q.status == Status::Delivered)
                {
                    return Err(TxError::InvalidOperation);
                }
            }
            Command::Incident { quote } => {
                let qi = self
                    .quotes
                    .iter()
                    .position(|q| q.spec.id == quote)
                    .ok_or(TxError::InvalidOperation)?;
                if self.inbox.incident_seen[qi] {
                    return Ok(());
                }
                if !self
                    .shipments
                    .iter()
                    .flatten()
                    .any(|s| s.quote_index == qi && s.arrives_at.is_none())
                {
                    return Err(TxError::InvalidOperation);
                }
                let index = self
                    .inbox
                    .facts
                    .iter()
                    .position(Option::is_none)
                    .ok_or(TxError::InboxFull)?;
                let spec = self.quotes[qi].spec;
                let sequence = self.inbox.next_sequence;
                let fee = Money::new(25).expect("fixed incident liability");
                self.inbox.facts[index] = Some(Fact {
                    sequence,
                    quote,
                    kind: Kind::Incident,
                    received_at: Some(time),
                    debtor: spec.carrier,
                    creditor: spec.buyer,
                    manager: spec.carrier,
                    handoffs: 0,
                    fee,
                    paid: Money::ZERO,
                    quantity: Quantity::ZERO,
                    invoice: Money::ZERO,
                    fragment: None,
                    external: None,
                });
                self.inbox.next_sequence = sequence.checked_add(1).ok_or(TxError::Overflow)?;
                self.inbox.incident_seen[qi] = true;
                self.inbox.accrued = plus(self.inbox.accrued, fee)?;
            }
            Command::Settle { sequence } => {
                let f = self
                    .inbox
                    .all_mut()
                    .find(|f| f.sequence == sequence)
                    .ok_or(TxError::InvalidOperation)?;
                if f.received_at.is_none() {
                    return Err(TxError::InvalidOperation);
                }
                let free = l
                    .cash(f.debtor)?
                    .checked_sub(l.protection(f.debtor)?)
                    .map_err(|_| TxError::Protection)?;
                let due = f.fee.checked_sub(f.paid).map_err(|_| TxError::Overflow)?;
                let pay = std::cmp::min(free, due);
                l.pay(f.debtor, f.creditor, pay)?;
                f.paid = plus(f.paid, pay)?;
                self.inbox.paid = plus(self.inbox.paid, pay)?;
            }
            Command::Handoff { sequence, manager } => {
                if !self.accounts.iter().any(|a| a.account == manager) {
                    return Err(TxError::Unauthorized);
                }
                let i = self
                    .inbox
                    .facts
                    .iter()
                    .position(|f| f.is_some_and(|f| f.sequence == sequence))
                    .ok_or(TxError::InvalidOperation)?;
                let mut fact = self.inbox.facts[i].ok_or(TxError::InvalidOperation)?;
                if fact.manager != manager && fact.handoffs >= 1
                    || fact.manager == manager && fact.received_at.is_none()
                {
                    return Err(TxError::InvalidOperation);
                }
                if fact.manager != manager {
                    fact.manager = manager;
                    fact.handoffs = 1;
                }
                if fact.received_at.is_some() {
                    let dst = self
                        .inbox
                        .retained
                        .iter()
                        .position(Option::is_none)
                        .ok_or(TxError::InboxFull)?;
                    self.inbox.retained[dst] = Some(fact);
                    self.inbox.facts[i] = None;
                } else {
                    self.inbox.facts[i] = Some(fact);
                }
            }
            Command::Archive { sequence } => {
                let slot = self
                    .inbox
                    .facts
                    .iter_mut()
                    .chain(self.inbox.retained.iter_mut())
                    .find(|f| f.is_some_and(|f| f.sequence == sequence))
                    .ok_or(TxError::InvalidOperation)?;
                let f = slot.ok_or(TxError::InvalidOperation)?;
                if f.received_at.is_none() || f.paid != f.fee {
                    return Err(TxError::UnsettledResponsibility);
                }
                *slot = None;
                self.inbox.closed = self.inbox.closed.checked_add(1).ok_or(TxError::Overflow)?;
            }
            Command::External { fact } => {
                if fact.source != 1
                    || fact.event == 0
                    || u64::from(fact.event) != self.inbox.external_next
                    || fact.debtor == fact.creditor
                    || fact.amount == Money::ZERO
                    || !self.accounts.iter().any(|a| a.account == fact.debtor)
                    || !self.accounts.iter().any(|a| a.account == fact.creditor)
                {
                    return Err(TxError::InvalidOperation);
                }
                let reserved_incidents = self
                    .shipments
                    .iter()
                    .flatten()
                    .filter(|s| !self.inbox.incident_seen[s.quote_index])
                    .count();
                if self.inbox.facts.iter().filter(|f| f.is_none()).count() <= reserved_incidents {
                    return Err(TxError::InboxFull);
                }
                let i = self
                    .inbox
                    .facts
                    .iter()
                    .position(Option::is_none)
                    .ok_or(TxError::InboxFull)?;
                let sequence = self.inbox.next_sequence;
                self.inbox.next_sequence = sequence.checked_add(1).ok_or(TxError::Overflow)?;
                self.inbox.external_next += 1;
                self.inbox.facts[i] = Some(Fact {
                    sequence,
                    quote: 0,
                    kind: Kind::External,
                    received_at: Some(time),
                    debtor: fact.debtor,
                    creditor: fact.creditor,
                    manager: fact.creditor,
                    handoffs: 0,
                    fee: fact.amount,
                    paid: Money::ZERO,
                    quantity: Quantity::ZERO,
                    invoice: Money::ZERO,
                    fragment: None,
                    external: Some(fact),
                });
                self.inbox.accrued = plus(self.inbox.accrued, fact.amount)?;
            }
        }
        self.inbox.validate()
    }
}
