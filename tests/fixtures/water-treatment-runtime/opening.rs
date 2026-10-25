//! Explicit V2 genesis only; actual joint production remains separately gated.
use mycity_core::{assembly,transaction::{Engine,Resource}};
pub fn input()->serde_json::Value{serde_json::from_slice(include_bytes!("joint-v2-opening.json")).unwrap()}
pub fn resources()->Vec<Resource>{[301,302,303,304,501,502,601,602,603,604].into_iter().map(|id|Resource::new(id,if id==501||id==502{4}else{1}).unwrap()).collect()}
pub fn engine(v:serde_json::Value)->Engine{Engine::new(assembly::assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),resources()).unwrap()}
