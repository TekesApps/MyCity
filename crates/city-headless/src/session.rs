//! Bounded, synchronous laboratory control. No new business scheduler or remote authority.
use mycity_core::transaction::{Action, Caller, Envelope};
use mycity_storage::{Error, Query, Store};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Read, Write},
    path::Path,
    time::{Duration, Instant},
};
const LINE_LIMIT: usize = 32768;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    request: Command,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Status,
    Pause,
    Resume,
    Speed {
        day_wall_ms: u32,
    },
    Submit {
        envelope: Envelope,
    },
    Step {
        envelope: Envelope,
    },
    Run {
        envelope: Envelope,
    },
    Receipt {
        operation_id: u64,
    },
    Query {
        section: Query,
        limit: usize,
        cursor: Option<String>,
    },
}
fn storage_error(e: Error) -> Value {
    match e {
        Error::Io(_) | Error::RecoveryRequired => {
            json!({"code":"recovery_required","outcome":"unknown","reopen_required":true})
        }
        Error::Invalid(reason) => json!({"code":"invalid","reason":reason}),
        Error::Model(reason) => json!({"code":"model_input","reason":reason}),
        Error::Locked => json!({"code":"locked"}),
        Error::FactBlocked(reason) => json!({"code":"fact_blocked","reason":reason}),
        Error::SourceSyncRequired => json!({"code":"source_sync_required"}),
        Error::Capacity {
            used_bytes,
            limit_bytes,
            required_bytes,
            emergency,
        } => {
            json!({"code":"storage_capacity","used_bytes":used_bytes,"limit_bytes":limit_bytes,"required_bytes":required_bytes,"emergency":emergency})
        }
    }
}
struct Session {
    store: Store,
    caller: Caller,
    paused: bool,
    day_wall_ms: u32,
    needs_reopen: bool,
}
impl Session {
    fn status(&self) -> Result<Value, Value> {
        let engine = self.store.engine().map_err(storage_error)?;
        let view = engine.world().view();
        let (profile, time) = if let Some(r) = &view.region {
            ("region_interaction", Some(r.interaction_time))
        } else if let Some(e) = &view.economy {
            ("economy", Some(e.time))
        } else if let Some(b) = &view.boot {
            ("boot", Some(b.time))
        } else {
            ("no_clock", None)
        };
        let snapshots: Vec<_> = view.region.iter().flat_map(|r| r.snapshots.iter().flatten().map(move |s| json!({"city":s.city,"version":s.version,"observed_at":s.observed_at,"age_game_seconds":r.interaction_time.saturating_sub(s.observed_at),"applied_sequence":s.applied_sequence}))).collect();
        Ok(
            json!({"evidence":"committed","regional_snapshots":snapshots,"paused":self.paused,"day_wall_ms":self.day_wall_ms,"profile":profile,"committed_game_seconds":time,
            "sequence":self.store.committed_sequence(),"revision":view.revision,"world":view.scope.world(),"branch":view.scope.branch(),
            "state_sha256":self.store.state_hash().map_err(storage_error)?,"source":self.store.source_status().map_err(storage_error)?,
            "provenance":{"source_sha256":env!("MYCITY_SOURCE_SHA256"),"lock_sha256":env!("MYCITY_LOCK_SHA256")},
            "offline_catchup":false,"full_source_scheduler":"not_implemented","memory_16gb_gate":"not_run"}),
        )
    }
    fn clock_seconds(&self, envelope: &Envelope) -> Option<u32> {
        let view = self.store.engine().ok()?.world().view();
        match envelope.actions.as_slice() {
            [Action::RegionAdvance { seconds }] if view.region.is_some() => Some(*seconds),
            [Action::Economy {
                command: mycity_core::economy::Command::Advance { seconds },
            }] if view.economy.is_some() && view.region.is_none() => Some(*seconds),
            [Action::Boot {
                command: mycity_core::boot::Command::Advance { seconds },
            }] if view.boot.is_some() && view.region.is_none() => Some(*seconds),
            _ => None,
        }
    }
    fn handle(&mut self, command: Command) -> Result<Value, Value> {
        if self.needs_reopen {
            return Err(
                json!({"code":"recovery_required","outcome":"unknown","reopen_required":true}),
            );
        }
        // A poisoned store also prevents apparently successful control acknowledgements.
        self.store.engine().map_err(storage_error)?;
        match command {
            Command::Status => self.status(),
            Command::Pause => {
                self.paused = true;
                self.status()
            }
            Command::Resume => {
                self.paused = false;
                self.status()
            }
            Command::Speed { day_wall_ms } => {
                if ![600000, 60000, 10000, 1000].contains(&day_wall_ms) {
                    return Err(json!({"code":"unsupported_speed"}));
                }
                self.day_wall_ms = day_wall_ms;
                self.status()
            }
            Command::Receipt { operation_id } => {
                let record = self.store.receipt(operation_id).map_err(storage_error)?;
                Ok(
                    json!({"state":if record.is_some(){"recorded"}else{"unknown"},"record":record,"sequence":self.store.committed_sequence()}),
                )
            }
            Command::Query {
                section,
                limit,
                cursor,
            } => serde_json::to_value(
                self.store
                    .query(section, cursor.as_deref(), limit)
                    .map_err(storage_error)?,
            )
            .map_err(|_| json!({"code":"encoding"})),
            command @ (Command::Submit { .. } | Command::Step { .. } | Command::Run { .. }) => {
                let mode = match &command {
                    Command::Step { .. } => 1,
                    Command::Run { .. } => 2,
                    _ => 0,
                };
                let envelope = match command {
                    Command::Submit { envelope }
                    | Command::Step { envelope }
                    | Command::Run { envelope } => envelope,
                    _ => unreachable!(),
                };
                let seconds = self.clock_seconds(&envelope);
                // Direct submissions cannot bypass the experiment controller clock gate.
                let contains_clock = envelope.actions.iter().any(|a| {
                    matches!(
                        a,
                        Action::RegionAdvance { .. }
                            | Action::Economy {
                                command: mycity_core::economy::Command::Advance { .. }
                            }
                            | Action::Boot {
                                command: mycity_core::boot::Command::Advance { .. }
                            }
                    )
                });
                if (mode == 0 && contains_clock) || (mode != 0 && seconds.is_none()) {
                    return Err(json!({"code":"clock_command_shape"}));
                }
                // Exact retries must remain possible after pause/reopen; Store verifies payload and caller.
                let retry = self
                    .store
                    .receipt(envelope.operation_id)
                    .map_err(storage_error)?
                    .is_some();
                if !retry && ((mode == 1 && !self.paused) || (mode == 2 && self.paused)) {
                    return Err(json!({"code":"pause_state"}));
                }
                let started = Instant::now();
                let wait_ns = if mode == 2 && !retry {
                    u128::from(seconds.unwrap()) * u128::from(self.day_wall_ms) * 1_000_000 / 86400
                } else {
                    0
                };
                if wait_ns > 60_000_000_000 {
                    return Err(json!({"code":"wall_budget","max_wall_ms":60000}));
                }
                if wait_ns > 0 {
                    std::thread::sleep(Duration::from_nanos(wait_ns as u64));
                }
                let outcome = match self.store.execute(self.caller, envelope) {
                    Ok(outcome) => outcome,
                    Err(e) => {
                        self.paused = true;
                        return Err(storage_error(e));
                    }
                };
                if outcome.model.is_err() {
                    self.paused = true;
                }
                let elapsed = started.elapsed().as_nanos();
                Ok(
                    json!({"outcome":outcome,"already_recorded":retry,"requested_game_seconds":seconds,"pacing_wait_ns":wait_ns,
                    "elapsed_ns":elapsed,"actual_game_seconds_per_wall_second":if mode!=0 && !retry && outcome.model.is_ok(){Some(f64::from(seconds.unwrap())*1e9/elapsed.max(1) as f64)}else{None},
                    "status":self.status()?}),
                )
            }
        }
    }
}
pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 6 {
        return Err("usage: mycity-headless session WORLD RULES <account ID|operator system> (trusted local host identity; never supplied in JSON)".into());
    }
    if args[3] != env!("MYCITY_SOURCE_SHA256") {
        return Err("rules_identity_must_match_current_source".into());
    }
    let caller = match (args[4].as_str(), args[5].as_str()) {
        ("operator", "system") => Caller::System,
        ("account", id) => Caller::Account(
            id.parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("invalid_account")?,
        ),
        _ => return Err("invalid_host_identity".into()),
    };
    let store = Store::open(Path::new(&args[2]), &args[3]).map_err(|e| format!("{e:?}"))?;
    let mut session = Session {
        store,
        caller,
        paused: true,
        day_wall_ms: 60000,
        needs_reopen: false,
    };
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    loop {
        let mut line = Vec::with_capacity(1024);
        let n = (&mut input)
            .take((LINE_LIMIT + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        // Oversized stream terminates immediately; no unbounded allocation/drain.
        if n > LINE_LIMIT {
            return Err("session_line_limit".into());
        }
        let response = match serde_json::from_slice::<Request>(&line) {
            Ok(request) => {
                let mut result = session.handle(request.request);
                if session.store.engine().is_err() {
                    session.paused = true;
                    result = Err(
                        json!({"code":"recovery_required","outcome":"unknown","reopen_required":true}),
                    );
                }
                if result
                    .as_ref()
                    .err()
                    .is_some_and(|e| e["reopen_required"] == true)
                {
                    session.needs_reopen = true;
                    session.paused = true;
                }
                match result {
                    Ok(value) => {
                        json!({"schema":"mycity-m0-session-v1","id":request.id,"result":value})
                    }
                    Err(error) => {
                        json!({"schema":"mycity-m0-session-v1","id":request.id,"error":error})
                    }
                }
            }
            Err(_) => {
                json!({"schema":"mycity-m0-session-v1","id":null,"error":{"code":"invalid_request"}})
            }
        };
        serde_json::to_writer(&mut output, &response).map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
