//! Read-only CLI planner. Output is a proposal, never a claim that Engine committed it.
use mycity_core::{
    read_bounded,
    town_economy::{plan_day, MarketInput},
};
use std::{env, fs::File, io};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("usage: cargo run -p mycity-core --example town_economy -- <day-input.json>")?;
    let file = File::open(path)?;
    let bytes = read_bounded(file, 128 * 1024)?;
    let input: MarketInput = serde_json::from_slice(&bytes)?;
    match plan_day(&input) {
        Ok(plan) => {
            println!("{{\"status\":\"pureplan_not_settled\",\"input_provenance\":\"fixture_supply_not_integrated\",\"engine_commit_required\":true,\"batch\":{}}}",serde_json::to_string(&plan)?);
            Ok(())
        }
        Err(error) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("market plan rejected: {error:?}"),
        )
        .into()),
    }
}
