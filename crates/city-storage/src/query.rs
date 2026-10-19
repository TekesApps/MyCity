//! Snapshot-bound pagination. Querying does not grant mutation or advance time.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    Contracts,
    Fulfilment,
    Arrears,
    Capacity,
    Blocked,
    Recovery,
    Receipts,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    sequence: u64,
    generation: u64,
    state: String,
    query: Query,
    offset: usize,
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub schema: &'static str,
    pub sequence: u64,
    pub state_sha256: String,
    pub snapshot_sha256: String,
    pub rows: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}
impl Store {
    pub fn query(
        &mut self,
        query: Query,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page, Error> {
        self.engine()?;
        if limit == 0 || limit > 64 {
            return Err(Error::Invalid("query_page_limit"));
        }
        let state_hash = self.state_hash()?;
        let hash = sha256(&json(
            &serde_json::json!({"state":state_hash,"source":self.source_status()?,"capacity":self.capacity_status()?}),
        )?);
        let mut offset = 0;
        if let Some(c) = cursor {
            if c.len() > 1024 {
                return Err(Error::Invalid("query_cursor_limit"));
            }
            let c: Cursor = serde_json::from_str(c)?;
            if c.sequence != self.io.root.sequence
                || c.generation != self.io.root.generation
                || c.state != hash
                || c.query != query
            {
                return Err(Error::Invalid("query_snapshot_changed"));
            }
            offset = c.offset;
        }
        let view = self.engine.world().view();
        let mut rows = vec![];
        let total;
        if query == Query::Receipts {
            let (page, count) = self.receipt_rows(offset, limit)?;
            rows = page;
            total = count;
        } else {
            match query {
                Query::Contracts => {
                    if let Some(e) = view.exchange {
                        for q in e.quotes {
                            rows.push(
                                serde_json::json!({"type":"contract","key":q.spec.id,"value":q}),
                            );
                        }
                    }
                }
                Query::Fulfilment => {
                    if let Some(e) = view.exchange {
                        for ship in e.shipments.into_iter().flatten() {
                            rows.push(serde_json::json!({"type":"in_transit","quote":e.quotes[ship.quote_index].spec.id,"value":ship}));
                        }
                        for q in e
                            .quotes
                            .into_iter()
                            .filter(|q| q.status == mycity_core::exchange::Status::Delivered)
                        {
                            rows.push(serde_json::json!({"type":"physical_delivery","quote":q.spec.id,"value":q}));
                        }
                        for f in e.inbox.all() {
                            rows.push(serde_json::json!({"type":"fact_or_retained_claim","sequence":f.sequence,"value":f}));
                        }
                    }
                }
                Query::Arrears => {
                    if let Some(r) = view.region {
                        for o in r
                            .obligations
                            .into_iter()
                            .flatten()
                            .filter(|o| o.arrears.value() > 0)
                        {
                            rows.push(serde_json::json!({"type":"periodic_obligation","value":o}));
                        }
                    }
                    if let Some(e) = view.exchange {
                        for f in e.inbox.all().filter(|f| f.fee > f.paid) {
                            rows.push(serde_json::json!({"type":"fact_liability","outstanding_cents":f.fee.value()-f.paid.value(),"value":f}));
                        }
                    }
                }
                Query::Capacity => {
                    for r in self.engine.resources() {
                        rows.push(serde_json::json!({"type":"resource","value":r}));
                    }
                    for l in self.engine.leases().iter().flatten() {
                        rows.push(serde_json::json!({"type":"lease","value":l}));
                    }
                    if let Some(e) = view.exchange {
                        rows.push(serde_json::json!({"type":"fact_capacity","working_used":e.inbox.facts.iter().flatten().count(),"normal_limit":mycity_core::inbox::NORMAL,"working_limit":mycity_core::inbox::TOTAL,"retained_used":e.inbox.retained.iter().flatten().count(),"retained_limit":mycity_core::inbox::RETAINED}));
                    }
                }
                Query::Blocked => {
                    rows.push(
                        serde_json::json!({"type":"disk_capacity","value":self.capacity_status()?}),
                    );
                    rows.push(
                        serde_json::json!({"type":"source_sync","value":self.source_status()?}),
                    );
                    if let Some(e) = view.exchange {
                        if e.inbox.admission_full() {
                            rows.push(serde_json::json!({"type":"new_commitments_blocked","reason":"normal_fact_working_capacity_exhausted"}));
                        }
                        for ship in e
                            .shipments
                            .iter()
                            .flatten()
                            .filter(|s| s.arrives_at.is_none())
                        {
                            rows.push(serde_json::json!({"type":"paused_transport","quote":e.quotes[ship.quote_index].spec.id,"remaining_seconds":ship.remaining,"leases_retained":ship.tokens}));
                        }
                    }
                }
                Query::Recovery => {
                    rows.push(
                        serde_json::json!({"type":"checkpoint","value":self.checkpoint_status()?}),
                    );
                    rows.push(
                        serde_json::json!({"type":"source_sync","value":self.source_status()?}),
                    );
                    if let Some(r) = view.region {
                        rows.push(serde_json::json!({"type":"regional_alignment","active_city":r.active_city,"interaction_time":r.interaction_time,"cities":r.cities,"snapshots":r.snapshots}));
                    }
                }
                Query::Receipts => unreachable!(),
            }
            total = rows.len();
            if offset > total {
                return Err(Error::Invalid("query_offset"));
            }
            rows = rows.into_iter().skip(offset).take(limit).collect();
        }
        let end = offset
            .checked_add(rows.len())
            .ok_or(Error::Invalid("query_offset_overflow"))?;
        let next_cursor = if end < total {
            Some(
                String::from_utf8(json(&Cursor {
                    sequence: self.io.root.sequence,
                    generation: self.io.root.generation,
                    state: hash.clone(),
                    query,
                    offset: end,
                })?)
                .map_err(|_| Error::Invalid("query_cursor_encoding"))?,
            )
        } else {
            None
        };
        Ok(Page {
            schema: "mycity-m0-query-v1",
            sequence: self.io.root.sequence,
            state_sha256: state_hash,
            snapshot_sha256: hash,
            rows,
            next_cursor,
        })
    }
}
