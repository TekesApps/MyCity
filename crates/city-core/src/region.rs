//! Bounded snapshot/interaction laboratory. No implicit offscreen production.
use crate::units::Money;
use serde::{Deserialize, Serialize};

pub const MAX_CITIES: usize = 8;
pub const MAX_ROUTES: usize = 16;
pub const MAX_TRANSITS: usize = 16;
pub const MAX_OBLIGATIONS: usize = 16;
pub const MAX_ADVANCE_SECONDS: u32 = 3600;
pub const MAX_SHORTFALL: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct City {
    pub id: u32,
    pub development_elapsed: u64,
    pub population: u32,
    pub willing: u32,
    pub housing_capacity: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub distance_m: u32,
    pub travel_seconds: u32,
    pub capacity_people: u32,
}
/// Fixed-second experimental obligation, NOT Gregorian monthly payroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub id: u32,
    pub payer: u32,
    pub payee: u32,
    pub period_seconds: u32,
    pub amount: Money,
    pub next_due: Option<u64>,
    pub expires_at: u64,
    pub arrears: Money,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub schema: String,
    pub active_city: u32,
    pub cities: Vec<City>,
    pub routes: Vec<Route>,
    pub obligations: Vec<Obligation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub city: u32,
    pub version: u64,
    pub observed_at: u64,
    pub population: u32,
    pub applied_sequence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Transit {
    pub id: u64,
    pub from: u32,
    pub to: u32,
    pub requested: u32,
    pub actual: u32,
    pub shortfall: u32,
    pub arrives_at: u64,
    pub route_indices: [Option<usize>; MAX_CITIES],
    pub lease_tokens: [Option<u64>; MAX_CITIES],
    pub pause_remaining: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Region {
    pub interaction_time: u64,
    pub active_city: u32,
    pub sequence: u64,
    pub cities: [Option<City>; MAX_CITIES],
    pub snapshots: [Option<Snapshot>; MAX_CITIES],
    pub routes: [Option<Route>; MAX_ROUTES],
    pub transits: [Option<Transit>; MAX_TRANSITS],
    pub obligations: [Option<Obligation>; MAX_OBLIGATIONS],
    next_transit: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    Input,
    City,
    NoRoute,
    Capacity,
    Shortfall,
    Limit,
    Overflow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub city: u32,
    pub distance_m: u64,
    pub travel_seconds: u64,
}
impl Region {
    pub fn new(mut opening: Opening) -> Result<Self, Error> {
        if opening.schema != "mycity-m0-region-v1"
            || opening.cities.is_empty()
            || opening.cities.len() > MAX_CITIES
            || opening.routes.len() > MAX_ROUTES
            || opening.obligations.len() > MAX_OBLIGATIONS
        {
            return Err(Error::Input);
        }
        opening.cities.sort_by_key(|x| x.id);
        opening.routes.sort_by_key(|x| x.id);
        opening.obligations.sort_by_key(|x| x.id);
        if opening.cities.windows(2).any(|x| x[0].id == x[1].id)
            || opening.routes.windows(2).any(|x| x[0].id == x[1].id)
            || opening.obligations.windows(2).any(|x| x[0].id == x[1].id)
        {
            return Err(Error::Input);
        }
        let mut result = Self {
            interaction_time: 0,
            active_city: opening.active_city,
            sequence: 0,
            cities: [None; MAX_CITIES],
            snapshots: [None; MAX_CITIES],
            routes: [None; MAX_ROUTES],
            transits: [None; MAX_TRANSITS],
            obligations: [None; MAX_OBLIGATIONS],
            next_transit: 1,
        };
        for (i, city) in opening.cities.into_iter().enumerate() {
            if city.id == 0
                || city.willing > city.population
                || city.population > city.housing_capacity
            {
                return Err(Error::Input);
            }
            result.cities[i] = Some(city);
            result.snapshots[i] = Some(Snapshot {
                city: city.id,
                version: 1,
                observed_at: 0,
                population: city.population,
                applied_sequence: 0,
            });
        }
        result.index(result.active_city)?;
        for (i, route) in opening.routes.into_iter().enumerate() {
            result.index(route.from)?;
            result.index(route.to)?;
            if route.id == 0
                || route.from == route.to
                || route.distance_m == 0
                || route.travel_seconds == 0
                || route.capacity_people == 0
            {
                return Err(Error::Input);
            }
            result.routes[i] = Some(route);
        }
        for (i, obligation) in opening.obligations.into_iter().enumerate() {
            if obligation.id == 0
                || obligation.payer == 0
                || obligation.payee == 0
                || obligation.payer == obligation.payee
                || obligation.period_seconds == 0
                || obligation.amount == Money::ZERO
                || obligation.arrears != Money::ZERO
                || obligation
                    .next_due
                    .is_none_or(|t| t == 0 || t > obligation.expires_at)
            {
                return Err(Error::Input);
            }
            result.obligations[i] = Some(obligation);
        }
        Ok(result)
    }
    fn index(&self, id: u32) -> Result<usize, Error> {
        self.cities
            .iter()
            .position(|c| c.is_some_and(|c| c.id == id))
            .ok_or(Error::City)
    }
    pub fn city(&self, id: u32) -> Result<City, Error> {
        self.cities[self.index(id)?].ok_or(Error::City)
    }
    pub(crate) fn bump(&mut self) -> Result<(), Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Overflow)?;
        Ok(())
    }
    pub(crate) fn activate(&mut self, id: u32) -> Result<(), Error> {
        let target = self.index(id)?;
        if id == self.active_city {
            return Ok(());
        }
        self.bump()?;
        for i in [self.index(self.active_city)?, target] {
            let city = self.cities[i].ok_or(Error::City)?;
            let previous = self.snapshots[i].ok_or(Error::City)?;
            self.snapshots[i] = Some(Snapshot {
                city: city.id,
                version: previous.version.checked_add(1).ok_or(Error::Overflow)?,
                observed_at: self.interaction_time,
                population: city.population,
                applied_sequence: self.sequence,
            });
        }
        self.active_city = id;
        Ok(())
    }
    fn used(&self, route: usize) -> u32 {
        self.transits
            .iter()
            .flatten()
            .filter(|t| t.route_indices.contains(&Some(route)))
            .map(|t| t.actual)
            .sum()
    }
    /// Distance-only discovery; not the future full household preference/market solver.
    pub fn candidates(&self, from: u32) -> Result<Vec<Candidate>, Error> {
        self.index(from)?;
        let mut result = Vec::with_capacity(MAX_CITIES - 1);
        for c in self.cities.iter().flatten().filter(|c| c.id != from) {
            if let Ok((_, distance, seconds)) = self.path(from, c.id, 0) {
                result.push(Candidate {
                    city: c.id,
                    distance_m: distance,
                    travel_seconds: seconds,
                });
            }
        }
        result.sort_by_key(|c| (c.distance_m, c.city));
        Ok(result)
    }
    fn path(
        &self,
        from: u32,
        to: u32,
        people: u32,
    ) -> Result<([Option<usize>; MAX_CITIES], u64, u64), Error> {
        let start = self.index(from)?;
        let end = self.index(to)?;
        if start == end {
            return Err(Error::City);
        }
        let mut distances = [u64::MAX; MAX_CITIES];
        let mut times = [0u64; MAX_CITIES];
        let mut previous = [None; MAX_CITIES];
        let mut visited = [false; MAX_CITIES];
        distances[start] = 0;
        for _ in 0..MAX_CITIES {
            let Some(i) = (0..MAX_CITIES)
                .filter(|i| !visited[*i] && distances[*i] != u64::MAX)
                .min_by_key(|i| (distances[*i], *i))
            else {
                break;
            };
            visited[i] = true;
            let city = self.cities[i].ok_or(Error::City)?;
            for (ri, r) in self
                .routes
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.map(|r| (i, r)))
            {
                if r.from != city.id || r.capacity_people.saturating_sub(self.used(ri)) < people {
                    continue;
                }
                let next = self.index(r.to)?;
                let distance = distances[i]
                    .checked_add(u64::from(r.distance_m))
                    .ok_or(Error::Overflow)?;
                if distance < distances[next] {
                    distances[next] = distance;
                    times[next] = times[i]
                        .checked_add(u64::from(r.travel_seconds))
                        .ok_or(Error::Overflow)?;
                    previous[next] = Some((i, ri));
                }
            }
        }
        if distances[end] == u64::MAX {
            return Err(Error::NoRoute);
        }
        let mut path = [None; MAX_CITIES];
        let mut cursor = end;
        let mut count = 0;
        while cursor != start {
            let (parent, route) = previous[cursor].ok_or(Error::NoRoute)?;
            if count == MAX_CITIES {
                return Err(Error::Limit);
            }
            path[count] = Some(route);
            count += 1;
            cursor = parent;
        }
        Ok((path, distances[end], times[end]))
    }
    pub(crate) fn migrate(
        &mut self,
        from: u32,
        to: u32,
        requested: u32,
        tolerance: u32,
    ) -> Result<u32, Error> {
        if requested == 0 || tolerance > MAX_SHORTFALL {
            return Err(Error::Input);
        }
        let source = self.index(from)?;
        let dest = self.index(to)?;
        let mut a = self.cities[source].ok_or(Error::City)?;
        let b = self.cities[dest].ok_or(Error::City)?;
        let incoming: u32 = self
            .transits
            .iter()
            .flatten()
            .filter(|t| t.to == to)
            .map(|t| t.actual)
            .sum();
        let free = b
            .housing_capacity
            .checked_sub(b.population)
            .and_then(|v| v.checked_sub(incoming))
            .ok_or(Error::Capacity)?;
        let actual = requested.min(a.willing).min(free);
        if actual == 0 || requested - actual > tolerance {
            return Err(Error::Shortfall);
        }
        let (path, _, seconds) = self.path(from, to, actual)?;
        let slot = self
            .transits
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Limit)?;
        let arrives_at = self
            .interaction_time
            .checked_add(seconds)
            .ok_or(Error::Overflow)?;
        let id = self.next_transit;
        self.next_transit = id.checked_add(1).ok_or(Error::Overflow)?;
        self.bump()?;
        a.population = a.population.checked_sub(actual).ok_or(Error::Input)?;
        a.willing -= actual;
        self.cities[source] = Some(a);
        self.transits[slot] = Some(Transit {
            id,
            from,
            to,
            requested,
            actual,
            shortfall: requested - actual,
            arrives_at,
            route_indices: path,
            lease_tokens: [None; MAX_CITIES],
            pause_remaining: None,
        });
        Ok(actual)
    }
    pub(crate) fn next_step(&self, remaining: u32) -> u32 {
        self.obligations
            .iter()
            .flatten()
            .filter_map(|o| o.next_due)
            .chain(
                self.transits
                    .iter()
                    .flatten()
                    .filter(|t| t.pause_remaining.is_none())
                    .map(|t| t.arrives_at),
            )
            .filter_map(|t| t.checked_sub(self.interaction_time))
            .filter(|dt| *dt > 0 && *dt <= u64::from(remaining))
            .min()
            .map_or(remaining, |dt| dt as u32)
    }
    pub(crate) fn advance(&mut self, seconds: u32) -> Result<(), Error> {
        if seconds == 0 || seconds > MAX_ADVANCE_SECONDS {
            return Err(Error::Limit);
        }
        self.bump()?;
        self.interaction_time = self
            .interaction_time
            .checked_add(u64::from(seconds))
            .ok_or(Error::Overflow)?;
        let i = self.index(self.active_city)?;
        let mut active = self.cities[i].ok_or(Error::City)?;
        active.development_elapsed = active
            .development_elapsed
            .checked_add(u64::from(seconds))
            .ok_or(Error::Overflow)?;
        self.cities[i] = Some(active);
        for slot in 0..MAX_TRANSITS {
            if let Some(transit) = self.transits[slot]
                .filter(|t| t.pause_remaining.is_none() && t.arrives_at <= self.interaction_time)
            {
                let i = self.index(transit.to)?;
                let mut dest = self.cities[i].ok_or(Error::City)?;
                dest.population = dest
                    .population
                    .checked_add(transit.actual)
                    .ok_or(Error::Overflow)?;
                self.cities[i] = Some(dest);
                self.transits[slot] = None;
            }
        }
        for o in self.obligations.iter_mut().flatten() {
            if let Some(due) = o.next_due.filter(|t| *t <= self.interaction_time) {
                let end = self.interaction_time.min(o.expires_at);
                let count = (end - due) / u64::from(o.period_seconds) + 1;
                let increment = i128::from(o.amount.value()) * i128::from(count);
                o.arrears = o
                    .arrears
                    .checked_add(
                        Money::new(i64::try_from(increment).map_err(|_| Error::Overflow)?)
                            .map_err(|_| Error::Overflow)?,
                    )
                    .map_err(|_| Error::Overflow)?;
                let next = due.checked_add(
                    count
                        .checked_mul(u64::from(o.period_seconds))
                        .ok_or(Error::Overflow)?,
                );
                o.next_due = next.filter(|t| *t <= o.expires_at);
            }
        }
        Ok(())
    }
}
