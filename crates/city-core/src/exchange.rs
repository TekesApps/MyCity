//! Bounded regional fulfilment. One ledger, explicit metric links, no snapshot production.
use crate::{
    assembly::{Commodity, World},
    boot::{self, Boot},
    economy::Fragment,
    region::{Region, MAX_CITIES},
    transaction::TxError,
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
pub const MAX_SHIPMENTS: usize = 4;
pub const MAX_QUOTES: usize = 8;
pub const PATH: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Freight,
    Electricity,
    Water,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub city: u32,
    pub origin_x_cm: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountHome {
    pub account: u32,
    pub city: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockHome {
    pub stock: u32,
    pub city: u32,
    pub limit: Quantity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub kind: Kind,
    pub resource: u32,
    pub a_cm: [i64; 3],
    pub b_cm: [i64; 3],
    pub travel_seconds: u32,
    pub population_route: Option<u32>,
}
impl Edge {
    pub fn distance_m(self) -> u64 {
        self.a_cm[0].abs_diff(self.b_cm[0]) / 100
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteSpec {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub seller: u32,
    pub buyer: u32,
    pub carrier: u32,
    pub source: u32,
    pub destination: u32,
    pub kind: Kind,
    pub quantity: Quantity,
    pub unit_price: Money,
    pub expires_at: u64,
    pub max_distance_m: u64,
    pub edges: [Option<u32>; PATH],
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub schema: String,
    pub profile: String,
    pub home_city: u32,
    pub maps: [Placement; 3],
    pub accounts: [AccountHome; 4],
    pub stocks: [StockHome; 14],
    pub edges: [Edge; 6],
    pub quotes: [QuoteSpec; MAX_QUOTES],
    pub energy_loss_limit: Quantity,
    pub water_loss_limit: Quantity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Dispatch { quote: u32 },
    SetLink { edge: u32, open: bool },
    Cancel { quote: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Pending,
    Transit,
    Delivered,
    Canceled,
    Expired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub spec: QuoteSpec,
    pub status: Status,
    pub delivered: Quantity,
    pub invoice: Money,
    pub freight: Money,
    pub source_cost: Money,
    pub loss_cost: Money,
    pub source_fragment: Option<Fragment>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shipment {
    pub quote_index: usize,
    pub fragment: Fragment,
    pub net: Quantity,
    pub loss: Quantity,
    pub price: Money,
    pub freight: Money,
    pub loss_cost: Money,
    pub arrives_at: Option<u64>,
    pub remaining: u64,
    pub tokens: [Option<u64>; PATH],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Exchange {
    pub profile: crate::FrozenLabel,
    pub home_city: u32,
    pub maps: [Placement; 3],
    pub accounts: [AccountHome; 4],
    pub stocks: [StockHome; 14],
    pub edges: [Edge; 6],
    pub open: [bool; 6],
    pub quotes: [Quote; MAX_QUOTES],
    pub shipments: [Option<Shipment>; MAX_SHIPMENTS],
    pub energy_loss: Quantity,
    pub water_loss: Quantity,
    pub energy_loss_limit: Quantity,
    pub water_loss_limit: Quantity,
    pub loss_expense: Money,
    pub sales: Money,
    pub freight_income: Money,
    pub cogs: Money,
    pub initial_basis: Money,
    pub initial_raw: Quantity,
    pub inbox: crate::inbox::Inbox,
    pub link_faults: [u8; MAX_QUOTES],
    pub link_repairs: [u8; MAX_QUOTES],
    pub population_link_controls: [u8; 6],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    Input,
    State,
    Expired,
    Closed,
    Path,
    Distance,
    Storage,
    Slots,
    Invariant,
    Clock,
}
fn error(e: Error) -> TxError {
    TxError::Exchange(e)
}
fn q(v: i64) -> Quantity {
    Quantity::new(v).expect("profile quantity")
}
fn m(v: i64) -> Money {
    Money::new(v).expect("profile money")
}
fn add(a: Money, b: Money) -> Result<Money, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
fn qa(a: Quantity, b: Quantity) -> Result<Quantity, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
pub(crate) trait Ledger: boot::Ledger {
    fn protect(&mut self, id: u32, amount: Money) -> Result<(), TxError>;
    fn pay_protected(&mut self, from: u32, to: u32, amount: Money) -> Result<(), TxError>;
    fn protection(&mut self, id: u32) -> Result<Money, TxError>;
    fn claim_path(&mut self, requests: &[(u32, u32)]) -> Result<[Option<u64>; PATH], TxError>;
    fn release_path(&mut self, tokens: [Option<u64>; PATH]) -> Result<(), TxError>;
    fn owns_path(&self, requests: &[(u32, u32)], tokens: [Option<u64>; PATH]) -> bool;
}
impl Exchange {
    pub(crate) fn new(o: Opening, w: &World) -> Result<Self, String> {
        let r = w.region.ok_or("exchange_region_required")?;
        let b = w.boot.ok_or("exchange_boot_required")?;
        if o.schema != "mycity-m0-exchange-v1"
            || o.profile != "regional-fulfilment-v1"
            || w.economy.is_some()
            || w.stock_ids.len() != 14
            || w.account_ids.len() != 4
            || b.stocks != [1, 2, 3, 4, 5, 6]
            || b.resources != [101, 102]
            || r.cities.iter().flatten().count() != 3
        {
            return Err("exchange_profile".into());
        }
        if w.view().opening_sources.iter().any(|s| s.id >= 0xc0000000) {
            return Err("exchange_reserved_lot_namespace".into());
        }
        let mut maps = o.maps;
        maps.sort_by_key(|x| x.city);
        for (i, map) in maps.iter().enumerate() {
            r.city(map.city).map_err(|_| "city")?;
            if map.origin_x_cm < 0
                || map.origin_x_cm > 10000000
                || maps[..i]
                    .iter()
                    .any(|x| x.city == map.city || x.origin_x_cm.abs_diff(map.origin_x_cm) < 102400)
            {
                return Err("exchange_map_placement".into());
            }
        }
        if !maps.iter().any(|m| m.city == o.home_city) {
            return Err("exchange_home".into());
        }
        let mut accounts = o.accounts;
        accounts.sort_by_key(|a| a.account);
        for (i, a) in accounts.iter().enumerate() {
            w.account_handle(a.account)?;
            r.city(a.city).map_err(|_| "city")?;
            if i > 0 && accounts[i - 1].account == a.account {
                return Err("exchange_accounts".into());
            }
        }
        let mut stocks = o.stocks;
        stocks.sort_by_key(|s| s.stock);
        let mut basis = Money::ZERO;
        for (i, s) in stocks.iter().enumerate() {
            if s.stock != i as u32 + 1 {
                return Err("exchange_stock_ids".into());
            }
            let stock = w
                .stocks
                .get(w.stock_handle(s.stock)?)
                .map_err(|_| "stock")?;
            let owner = w.accounts.get(stock.owner).map_err(|_| "owner")?.id;
            if stock.quantity > s.limit
                || s.limit == Quantity::ZERO
                || !accounts
                    .iter()
                    .any(|a| a.account == owner && a.city == s.city)
            {
                return Err("exchange_custody".into());
            }
            basis = basis.checked_add(stock.basis).map_err(|_| "basis")?;
            if i < 6 && (s.city != o.home_city || s.limit != b.limits[i]) {
                return Err("exchange_boot_home".into());
            }
            if i >= 7 && (stock.quantity != Quantity::ZERO || stock.basis != Money::ZERO) {
                return Err("exchange_receivers_must_be_empty".into());
            }
            let kind = if i <= 5 {
                stock.commodity
            } else if i <= 11 {
                Commodity::Raw
            } else if i == 12 {
                Commodity::Electricity
            } else {
                Commodity::Water
            };
            if stock.commodity != kind {
                return Err("exchange_receiver_type".into());
            }
        }
        let mut edges = o.edges;
        edges.sort_by_key(|e| e.id);
        for (i, e) in edges.iter().enumerate() {
            let a = maps.iter().find(|m| m.city == e.from).ok_or("edge_from")?;
            let z = match e.kind {
                Kind::Freight => 0,
                Kind::Electricity => 500,
                Kind::Water => -300,
            };
            let b = maps.iter().find(|m| m.city == e.to).ok_or("edge_to")?;
            if e.id == 0
                || e.resource == 0
                || e.from == e.to
                || e.a_cm != [a.origin_x_cm + 51200, 51200, z]
                || e.b_cm != [b.origin_x_cm + 51200, 51200, z]
                || e.distance_m() != 1024
                || edges[..i]
                    .iter()
                    .any(|x| x.id == e.id || x.resource == e.resource)
            {
                return Err("exchange_metric_edge".into());
            }
            let typed = match e.kind {
                Kind::Freight => [201, 202].contains(&e.resource),
                Kind::Electricity => [101, 103].contains(&e.resource),
                Kind::Water => [102, 104].contains(&e.resource),
            };
            if !typed
                || (e.resource == 101 && e.from != o.home_city)
                || (e.resource == 102 && e.from != o.home_city)
                || (e.from == o.home_city && e.kind == Kind::Electricity && e.resource != 101)
                || (e.from == o.home_city && e.kind == Kind::Water && e.resource != 102)
            {
                return Err("exchange_resource_domain".into());
            }
            let seconds = match e.kind {
                Kind::Freight => 60,
                Kind::Electricity => 1,
                Kind::Water => 512,
            };
            if e.travel_seconds != seconds {
                return Err("exchange_link_time".into());
            }
            if e.kind == Kind::Freight {
                let route = r
                    .routes
                    .iter()
                    .flatten()
                    .find(|x| Some(x.id) == e.population_route)
                    .ok_or("exchange_population_route")?;
                if route.from != e.from
                    || route.to != e.to
                    || u64::from(route.distance_m) != e.distance_m()
                    || route.travel_seconds != e.travel_seconds
                {
                    return Err("exchange_route_geometry".into());
                }
            } else if e.population_route.is_some() {
                return Err("exchange_typed_link".into());
            }
        }
        if r.routes
            .iter()
            .flatten()
            .any(|route| !edges.iter().any(|e| e.population_route == Some(route.id)))
        {
            return Err("exchange_unbound_population_route".into());
        }
        let initial_raw = w
            .stocks
            .get(w.stock_handle(7)?)
            .map_err(|_| "raw")?
            .quantity;
        let mut specs = o.quotes;
        specs.sort_by_key(|s| s.id);
        let quotes = specs.map(|s| Quote {
            spec: s,
            status: Status::Pending,
            delivered: Quantity::ZERO,
            invoice: Money::ZERO,
            freight: Money::ZERO,
            source_cost: Money::ZERO,
            loss_cost: Money::ZERO,
            source_fragment: None,
        });
        let result = Self {
            profile: crate::FrozenLabel("regional-fulfilment-v1"),
            home_city: o.home_city,
            maps,
            accounts,
            stocks,
            edges,
            open: [true; 6],
            quotes,
            shipments: [None; MAX_SHIPMENTS],
            energy_loss: Quantity::ZERO,
            water_loss: Quantity::ZERO,
            energy_loss_limit: o.energy_loss_limit,
            water_loss_limit: o.water_loss_limit,
            loss_expense: Money::ZERO,
            sales: Money::ZERO,
            freight_income: Money::ZERO,
            cogs: Money::ZERO,
            initial_basis: basis,
            initial_raw,
            inbox: crate::inbox::Inbox::default(),
            link_faults: [0; MAX_QUOTES],
            link_repairs: [0; MAX_QUOTES],
            population_link_controls: [0; 6],
        };
        for (i, quote) in result.quotes.iter().enumerate() {
            let s = quote.spec;
            if s.id == 0
                || s.id >= 0x10000000
                || s.quantity == Quantity::ZERO
                || s.quantity.value() > 1000000
                || s.unit_price == Money::ZERO
                || s.expires_at == 0
                || s.max_distance_m == 0
                || result.quotes[..i].iter().any(|q| q.spec.id == s.id)
                || s.seller == s.buyer
            {
                return Err("exchange_quote".into());
            }
            for (id, city) in [(s.seller, s.from), (s.buyer, s.to)] {
                if !result
                    .accounts
                    .iter()
                    .any(|a| a.account == id && a.city == city)
                {
                    return Err("exchange_quote_home".into());
                }
            }
            w.account_handle(s.carrier)?;
            for (id, city, owner) in [(s.source, s.from, s.seller), (s.destination, s.to, s.buyer)]
            {
                let h = result
                    .stocks
                    .iter()
                    .find(|x| x.stock == id && x.city == city)
                    .ok_or("quote_custody")?;
                let stock = w
                    .stocks
                    .get(w.stock_handle(h.stock)?)
                    .map_err(|_| "stock")?;
                if w.accounts.get(stock.owner).map_err(|_| "owner")?.id != owner {
                    return Err("quote_owner".into());
                }
            }
            let (source, dest) = match s.kind {
                Kind::Freight => (s.source == 7, (8..=12).contains(&s.destination)),
                Kind::Electricity => (s.source == 2, s.destination == 13),
                Kind::Water => (s.source == 3, s.destination == 14),
            };
            if !source || !dest {
                return Err("quote_source_type".into());
            }
            result.path(s).map_err(|_| "quote_path")?;
        }
        Ok(result)
    }
    fn path(&self, s: QuoteSpec) -> Result<Vec<usize>, TxError> {
        let mut path = Vec::with_capacity(PATH);
        let mut city = s.from;
        let mut ended = false;
        let mut visited = vec![city];
        for id in s.edges {
            match id {
                None => ended = true,
                Some(id) => {
                    if ended {
                        return Err(error(Error::Path));
                    }
                    let i = self
                        .edges
                        .iter()
                        .position(|e| e.id == id)
                        .ok_or(error(Error::Path))?;
                    let e = self.edges[i];
                    if e.from != city || e.kind != s.kind || visited.contains(&e.to) {
                        return Err(error(Error::Path));
                    }
                    path.push(i);
                    city = e.to;
                    visited.push(city);
                }
            }
        }
        if path.is_empty() || city != s.to {
            return Err(error(Error::Path));
        }
        Ok(path)
    }
    fn requests(&self, s: QuoteSpec) -> Result<Vec<(u32, u32)>, TxError> {
        let units = if s.kind == Kind::Freight {
            u32::try_from((s.quantity.value() + 999) / 1000 * 20).map_err(|_| TxError::Overflow)?
        } else {
            1
        };
        let mut result = self
            .path(s)?
            .into_iter()
            .map(|i| (self.edges[i].resource, units))
            .collect::<Vec<_>>();
        result.sort();
        Ok(result)
    }
    pub(crate) fn migration_requests(
        &self,
        r: &Region,
        indices: [Option<usize>; MAX_CITIES],
        people: u32,
    ) -> Result<Vec<(u32, u32)>, TxError> {
        self.migration_requests_inner(r, indices, people, false)
    }
    fn migration_requests_inner(
        &self,
        r: &Region,
        indices: [Option<usize>; MAX_CITIES],
        people: u32,
        ignore_closed: bool,
    ) -> Result<Vec<(u32, u32)>, TxError> {
        let mut out = Vec::with_capacity(PATH);
        for i in indices.into_iter().flatten() {
            let route = r.routes[i].ok_or(error(Error::Path))?;
            let edge = self
                .edges
                .iter()
                .find(|e| e.population_route == Some(route.id))
                .ok_or(error(Error::Path))?;
            if !ignore_closed
                && !self.open[self
                    .edges
                    .iter()
                    .position(|x| x.id == edge.id)
                    .ok_or(error(Error::Path))?]
            {
                return Err(error(Error::Closed));
            }
            out.push((edge.resource, people));
        }
        out.sort();
        Ok(out)
    }
    pub fn required_protection(&self, id: u32) -> Money {
        self.shipments
            .iter()
            .flatten()
            .filter(|s| self.quotes[s.quote_index].spec.buyer == id)
            .fold(Money::ZERO, |sum, s| {
                sum.checked_add(s.price).expect("validated protection sum")
            })
    }
    pub(crate) fn command(
        &mut self,
        c: Command,
        r: &mut Region,
        b: &mut Boot,
        l: &mut impl Ledger,
    ) -> Result<(), TxError> {
        match c {
            Command::Cancel { quote } => {
                let q = self
                    .quotes
                    .iter_mut()
                    .find(|q| q.spec.id == quote)
                    .ok_or(error(Error::Input))?;
                if q.status != Status::Pending {
                    return Err(error(Error::State));
                }
                q.status = Status::Canceled;
                r.bump().map_err(TxError::Region)?;
            }
            Command::SetLink { edge, open } => {
                let i = self
                    .edges
                    .iter()
                    .position(|e| e.id == edge)
                    .ok_or(error(Error::Path))?;
                if self.open[i] != open {
                    for ship in self.shipments.iter().flatten() {
                        if self.quotes[ship.quote_index]
                            .spec
                            .edges
                            .contains(&Some(edge))
                        {
                            if open {
                                self.link_repairs[ship.quote_index] |= 1 << i;
                            } else {
                                self.link_faults[ship.quote_index] |= 1 << i;
                            }
                        }
                    }
                    if r.transits.iter().flatten().any(|t| {
                        t.route_indices.iter().flatten().any(|ri| {
                            r.routes[*ri].is_some_and(|route| {
                                self.edges[i].population_route == Some(route.id)
                            })
                        })
                    }) {
                        self.population_link_controls[i] |= if open { 2 } else { 1 };
                    }
                }
                self.open[i] = open;
                for si in 0..MAX_SHIPMENTS {
                    if let Some(mut ship) = self.shipments[si] {
                        let path = self.path(self.quotes[ship.quote_index].spec)?;
                        let clear = path.iter().all(|i| self.open[*i]);
                        if !clear {
                            if let Some(arrival) = ship.arrives_at {
                                ship.remaining = arrival
                                    .checked_sub(r.interaction_time)
                                    .ok_or(error(Error::Clock))?;
                                ship.arrives_at = None;
                            }
                        } else if ship.arrives_at.is_none() {
                            ship.arrives_at = Some(
                                r.interaction_time
                                    .checked_add(ship.remaining)
                                    .ok_or(TxError::Overflow)?,
                            );
                        }
                        self.shipments[si] = Some(ship);
                    }
                }
                for si in 0..crate::region::MAX_TRANSITS {
                    if let Some(mut transit) = r.transits[si] {
                        let requests = self.migration_requests_ignoring_closure(
                            r,
                            transit.route_indices,
                            transit.actual,
                        )?;
                        let clear = requests.iter().all(|(id, _)| {
                            self.edges
                                .iter()
                                .enumerate()
                                .any(|(i, e)| e.resource == *id && self.open[i])
                        });
                        if !clear && transit.pause_remaining.is_none() {
                            transit.pause_remaining = Some(
                                transit
                                    .arrives_at
                                    .checked_sub(r.interaction_time)
                                    .ok_or(error(Error::Clock))?,
                            );
                        } else if clear {
                            if let Some(left) = transit.pause_remaining {
                                transit.arrives_at = r
                                    .interaction_time
                                    .checked_add(left)
                                    .ok_or(TxError::Overflow)?;
                                transit.pause_remaining = None;
                            }
                        }
                        r.transits[si] = Some(transit);
                    }
                }
                r.bump().map_err(TxError::Region)?;
            }
            Command::Dispatch { quote } => {
                let qi = self
                    .quotes
                    .iter()
                    .position(|q| q.spec.id == quote)
                    .ok_or(error(Error::Input))?;
                let spec = self.quotes[qi].spec;
                self.inbox.admit(spec.id, spec.buyer, spec.carrier)?;
                if r.interaction_time >= spec.expires_at {
                    return Err(error(Error::Expired));
                }
                if self.quotes[qi].status != Status::Pending {
                    return Err(error(Error::State));
                }
                let slot = self
                    .shipments
                    .iter()
                    .position(Option::is_none)
                    .ok_or(error(Error::Slots))?;
                let path = self.path(spec)?;
                if path.iter().any(|i| !self.open[*i]) {
                    return Err(error(Error::Closed));
                }
                let distance: u64 = path.iter().map(|i| self.edges[*i].distance_m()).sum();
                if distance > spec.max_distance_m || spec.kind != Kind::Freight && distance > 2048 {
                    return Err(error(Error::Distance));
                }
                let mut net = spec.quantity;
                if spec.kind != Kind::Freight {
                    for _ in &path {
                        net = q(i64::try_from(i128::from(net.value()) * 990 / 1000)
                            .map_err(|_| TxError::Overflow)?);
                    }
                    let amount = spec.quantity.value();
                    let limit = if spec.kind == Kind::Electricity {
                        400000
                    } else {
                        600000
                    };
                    if amount > limit {
                        return Err(error(Error::Input));
                    }
                }
                if net == Quantity::ZERO {
                    return Err(error(Error::Input));
                }
                let loss = spec
                    .quantity
                    .checked_sub(net)
                    .map_err(|_| error(Error::Invariant))?;
                let receiver = self
                    .stocks
                    .iter()
                    .find(|s| s.stock == spec.destination)
                    .ok_or(error(Error::Input))?;
                let dst = l.stock(spec.destination)?;
                let mut projected = qa(dst.quantity, net)?;
                for s in self.shipments.iter().flatten() {
                    if self.quotes[s.quote_index].spec.destination == spec.destination {
                        if spec.kind == Kind::Freight {
                            return Err(error(Error::Storage));
                        }
                        projected = qa(projected, s.net)?;
                    }
                }
                if projected > receiver.limit
                    || spec.kind == Kind::Freight && dst.quantity != Quantity::ZERO
                {
                    return Err(error(Error::Storage));
                }
                let current_loss = if spec.kind == Kind::Electricity {
                    self.energy_loss
                } else {
                    self.water_loss
                };
                if spec.kind != Kind::Freight {
                    let mut projected = qa(current_loss, loss)?;
                    for s in self
                        .shipments
                        .iter()
                        .flatten()
                        .filter(|s| self.quotes[s.quote_index].spec.kind == spec.kind)
                    {
                        projected = qa(projected, s.loss)?;
                    }
                    let limit = if spec.kind == Kind::Electricity {
                        self.energy_loss_limit
                    } else {
                        self.water_loss_limit
                    };
                    if projected > limit {
                        return Err(error(Error::Storage));
                    }
                }
                let price = Money::new(
                    i64::try_from(
                        i128::from(net.value()) * i128::from(spec.unit_price.value()) / 1000,
                    )
                    .map_err(|_| TxError::Overflow)?,
                )
                .map_err(|_| TxError::Overflow)?;
                if price == Money::ZERO {
                    return Err(error(Error::Input));
                }
                let freight = m(path.len() as i64 * 10);
                let tokens = l.claim_path(&self.requests(spec)?)?;
                l.pay(spec.buyer, spec.carrier, freight)?;
                l.protect(spec.buyer, price)?;
                let source = l.stock(spec.source)?;
                let basis = if spec.kind == Kind::Freight {
                    take_frozen(l, spec.source, spec.quantity)?
                } else {
                    b.export(
                        if spec.kind == Kind::Electricity { 0 } else { 1 },
                        spec.quantity,
                        l,
                    )?
                };
                let loss_cost = Money::new(
                    i64::try_from(
                        i128::from(basis.value()) * i128::from(loss.value())
                            / i128::from(spec.quantity.value()),
                    )
                    .map_err(|_| TxError::Overflow)?,
                )
                .map_err(|_| TxError::Overflow)?;
                let time: u64 = path
                    .iter()
                    .map(|i| u64::from(self.edges[*i].travel_seconds))
                    .sum::<u64>()
                    + if spec.kind == Kind::Freight { 0 } else { 60 };
                self.shipments[slot] = Some(Shipment {
                    quote_index: qi,
                    fragment: Fragment {
                        lot_id: source.lot_id,
                        origin_quantity: source.origin_quantity,
                        origin_basis: source.origin_basis,
                        offset: source.offset,
                        quantity: spec.quantity,
                        basis,
                    },
                    net,
                    loss,
                    price,
                    freight,
                    loss_cost,
                    arrives_at: Some(
                        r.interaction_time
                            .checked_add(time)
                            .ok_or(TxError::Overflow)?,
                    ),
                    remaining: time,
                    tokens,
                });
                self.freight_income = add(self.freight_income, freight)?;
                self.quotes[qi].status = Status::Transit;
                r.bump().map_err(TxError::Region)?;
            }
        }
        Ok(())
    }
    pub(crate) fn next_boundary(&self, now: u64) -> Option<u64> {
        self.shipments
            .iter()
            .flatten()
            .filter_map(|s| s.arrives_at)
            .chain(
                self.quotes
                    .iter()
                    .filter(|q| q.status == Status::Pending)
                    .map(|q| q.spec.expires_at),
            )
            .filter(|t| *t > now)
            .min()
    }
    pub(crate) fn arrive(&mut self, r: &mut Region, l: &mut impl Ledger) -> Result<(), TxError> {
        let now = r.interaction_time;
        for q in &mut self.quotes {
            if q.status == Status::Pending && q.spec.expires_at <= now {
                q.status = Status::Expired;
            }
        }
        for i in 0..MAX_SHIPMENTS {
            if let Some(ship) = self.shipments[i].filter(|s| s.arrives_at.is_some_and(|t| t <= now))
            {
                let spec = self.quotes[ship.quote_index].spec;
                l.pay_protected(spec.buyer, spec.seller, ship.price)?;
                let mut dst = l.stock(spec.destination)?;
                let acquisition = add(ship.price, ship.freight)?;
                if spec.kind == Kind::Freight && dst.quantity != Quantity::ZERO {
                    return Err(error(Error::Storage));
                }
                dst.quantity = qa(dst.quantity, ship.net)?;
                dst.basis = add(dst.basis, acquisition)?;
                dst.origin_quantity = dst.quantity;
                dst.origin_basis = dst.basis;
                dst.offset = Quantity::ZERO;
                dst.lot_id = 0xc0000000 + spec.id;
                l.put(spec.destination, dst)?;
                if spec.kind == Kind::Electricity {
                    self.energy_loss = qa(self.energy_loss, ship.loss)?;
                } else if spec.kind == Kind::Water {
                    self.water_loss = qa(self.water_loss, ship.loss)?;
                }
                self.loss_expense = add(self.loss_expense, ship.loss_cost)?;
                self.sales = add(self.sales, ship.price)?;
                self.cogs = add(
                    self.cogs,
                    ship.fragment
                        .basis
                        .checked_sub(ship.loss_cost)
                        .map_err(|_| error(Error::Invariant))?,
                )?;
                let q = &mut self.quotes[ship.quote_index];
                q.status = Status::Delivered;
                q.delivered = ship.net;
                q.invoice = ship.price;
                q.freight = ship.freight;
                q.source_cost = ship.fragment.basis;
                q.source_fragment = Some(ship.fragment);
                q.loss_cost = ship.loss_cost;
                self.inbox
                    .delivered(spec.id, now, ship.net, ship.price, ship.fragment)?;
                l.release_path(ship.tokens)?;
                self.shipments[i] = None;
                r.bump().map_err(TxError::Region)?;
            }
        }
        Ok(())
    }
    pub(crate) fn validate(
        &self,
        r: &Region,
        b: &Boot,
        l: &mut impl Ledger,
    ) -> Result<(), TxError> {
        self.inbox.validate()?;
        if b.time != r.interaction_time {
            return Err(error(Error::Clock));
        }
        let mut basis = add(b.failed_expense, self.loss_expense)?;
        let mut raw = Quantity::ZERO;
        for s in self.stocks {
            let stock = l.stock(s.stock)?;
            if stock.quantity > s.limit {
                return Err(error(Error::Storage));
            }
            basis = add(basis, stock.basis)?;
            if (7..=12).contains(&s.stock) {
                raw = qa(raw, stock.quantity)?;
            }
        }
        for j in b.jobs {
            basis = add(basis, j.cost)?;
        }
        let mut e = self.energy_loss;
        let mut water = self.water_loss;
        for s in self.shipments.iter().flatten() {
            let q = self.quotes[s.quote_index];
            if q.status != Status::Transit || !l.owns_path(&self.requests(q.spec)?, s.tokens) {
                return Err(error(Error::Invariant));
            }
            basis = add(basis, add(s.fragment.basis, s.freight)?)?;
            match q.spec.kind {
                Kind::Freight => raw = qa(raw, s.fragment.quantity)?,
                Kind::Electricity => e = qa(e, s.fragment.quantity)?,
                Kind::Water => water = qa(water, s.fragment.quantity)?,
            }
        }
        e = qa(e, l.stock(13)?.quantity)?;
        water = qa(water, l.stock(14)?.quantity)?;
        if e != b.exports[0]
            || water != b.exports[1]
            || raw != self.initial_raw
            || i128::from(basis.value())
                != i128::from(self.initial_basis.value())
                    + i128::from(b.fees.value())
                    + i128::from(self.freight_income.value())
                    + i128::from(self.sales.value())
                    - i128::from(self.cogs.value())
        {
            return Err(error(Error::Invariant));
        }
        for a in self.accounts {
            if l.protection(a.account)? < self.required_protection(a.account) {
                return Err(TxError::Protection);
            }
        }
        for t in r.transits.iter().flatten() {
            let requests =
                self.migration_requests_ignoring_closure(r, t.route_indices, t.actual)?;
            if !l.owns_path(&requests, t.lease_tokens) {
                return Err(error(Error::Invariant));
            }
        }
        Ok(())
    }
    fn migration_requests_ignoring_closure(
        &self,
        r: &Region,
        indices: [Option<usize>; MAX_CITIES],
        people: u32,
    ) -> Result<Vec<(u32, u32)>, TxError> {
        self.migration_requests_inner(r, indices, people, true)
    }
}
fn take_frozen(l: &mut impl Ledger, id: u32, quantity: Quantity) -> Result<Money, TxError> {
    let mut s = l.stock(id)?;
    let end = qa(s.offset, quantity)?;
    if quantity > s.quantity || s.origin_quantity == Quantity::ZERO {
        return Err(TxError::Stock);
    }
    let denominator = i128::from(s.origin_quantity.value());
    let cost = Money::new(
        i64::try_from(
            i128::from(s.origin_basis.value()) * i128::from(end.value()) / denominator
                - i128::from(s.origin_basis.value()) * i128::from(s.offset.value()) / denominator,
        )
        .map_err(|_| TxError::Overflow)?,
    )
    .map_err(|_| TxError::Overflow)?;
    s.quantity = s
        .quantity
        .checked_sub(quantity)
        .map_err(|_| TxError::Stock)?;
    s.basis = s.basis.checked_sub(cost).map_err(|_| TxError::Stock)?;
    s.offset = end;
    l.put(id, s)?;
    Ok(cost)
}
