//! Attribute a reached registration problem to its original record and witness.
use super::equal_text;
use super::{Delta, Edge, Origin};
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
use vstd::string::StringExecFns;

verus! {

pub fn record_rank(
    delta: &[Delta], path: &str, contains_target: bool,
) -> (rank: u8)
    ensures rank == if exists|i: int| 0 <= i < delta.len() && #[trigger] delta@[i].path@ == path@ {
        3u8
    } else if contains_target { 2u8 } else { 1u8 },
{
    let mut i = 0usize;
    while i < delta.len()
        invariant i <= delta.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] delta@[j].path@ != path@,
        decreases delta.len() - i,
    {
        if equal_text(delta[i].path.as_str(), path) { return 3; }
        i += 1;
    }
    if contains_target { 2 } else { 1 }
}

pub fn record_source<T>(base: Option<(T,u8)>, candidate: Option<(T,u8)>, at_base: bool)
    -> (source: Option<(T,u8)>)
    ensures source == match (base, candidate) {
        (Some(b), Some(c)) => if b.1 > c.1 || (b.1 == c.1 && at_base) { Some(b) } else { Some(c) },
        (Some(b), None) => Some(b),
        (None, c) => c,
    },
{
    match (base, candidate) {
        (Some(b), Some(c)) => if b.1 > c.1 || (b.1 == c.1 && at_base) { Some(b) } else { Some(c) },
        (Some(b), None) => Some(b),
        (None, c) => c,
    }
}

pub fn registry_reference(side: &str, prefix: &str, pointer: &str) -> (out: String)
    ensures out@ == "/registries/"@ + side@ + prefix@ + pointer@,
{
    "/registries/".to_owned().concat(side).concat(prefix).concat(pointer)
}

pub fn record_reference(rank: u8, path: &str, side: &str, pointer: &str) -> (out: String)
    ensures out@ == if rank == 3 { path@ } else { "/registries/"@ + side@ + ""@ + pointer@ },
{
    if rank == 3 { path.to_owned() } else { registry_reference(side, "", pointer) }
}

pub fn workflow_reference(side: &str, pointer: &str, schema: Option<u64>, original: u64) -> (out: String)
    ensures out@ == "/registries/"@ + side@ + "/workflow"@
        + (if schema == Some(original) { pointer@ } else { ""@ }),
{
    registry_reference(side, "/workflow", if schema == Some(original) { pointer } else { "" })
}

pub enum Ambiguity<T> { Unique, Historical, Reached(T) }

pub fn ambiguity<T>(definitions: usize, at_base: bool, repaired: bool, reached: Option<T>)
    -> (out: Ambiguity<T>)
    ensures out == if definitions <= 1 { Ambiguity::Unique }
        else if at_base && repaired { Ambiguity::Historical }
        else { match reached { Some(r) => Ambiguity::Reached(r), None => Ambiguity::Historical } },
{
    if definitions <= 1 { Ambiguity::Unique }
    else if at_base && repaired { Ambiguity::Historical }
    else { match reached { Some(r) => Ambiguity::Reached(r), None => Ambiguity::Historical } }
}

pub enum EdgeProblem { MissingSource, MissingTarget, InvalidType }

pub fn problem(source: bool, target: bool, valid: bool) -> (out: Option<EdgeProblem>)
    ensures out == if !source { Some(EdgeProblem::MissingSource) }
        else if !target { Some(EdgeProblem::MissingTarget) }
        else if !valid { Some(EdgeProblem::InvalidType) } else { None },
{
    if !source { Some(EdgeProblem::MissingSource) }
    else if !target { Some(EdgeProblem::MissingTarget) }
    else if !valid { Some(EdgeProblem::InvalidType) } else { None }
}

impl EdgeProblem {
    pub fn code(&self) -> (out: &'static str)
        ensures out@ == match self { Self::InvalidType => "E_EDGE_TYPE"@, _ => "E_DANGLING_EDGE"@ },
    { match self { Self::InvalidType => "E_EDGE_TYPE", _ => "E_DANGLING_EDGE" } }
}

pub fn on_endpoint(origin: &Origin, at_base: bool) -> (applies: bool)
    ensures applies == match origin { Origin::Base => at_base, Origin::Candidate => !at_base, Origin::Both => true },
{ match origin { Origin::Base => at_base, Origin::Candidate => !at_base, Origin::Both => true } }

pub fn reached_edge<T>(from: Option<T>, to: Option<T>) -> (out: Option<(T,bool)>)
    ensures out == match from {
        Some(f) => Some((f,true)), None => match to { Some(t) => Some((t,false)), None => None },
    },
{
    match from { Some(f) => Some((f,true)), None => match to { Some(t) => Some((t,false)), None => None } }
}

/// The original seed, every traversed destination in order, then the last
/// target only when the source of the faulty edge was the reached endpoint.
pub fn witness(seed: &String, path: &[Edge], tail: Option<&String>) -> (out: Vec<String>)
    ensures out.len() == 1 + path.len() + (if tail.is_some() {1int} else {0int}),
        out@[0] == *seed,
        forall|j: int| 0 <= j < path.len() ==> out@[j+1] == #[trigger] path@[j].to,
        tail.is_some() ==> out@.last() == *tail.unwrap(),
{
    let mut chain = Vec::new();
    chain.push(seed.clone());
    let mut i = 0usize;
    while i < path.len()
        invariant i <= path.len(), chain.len() == 1 + i, chain@[0] == *seed,
            forall|j: int| 0 <= j < i ==> chain@[j+1] == #[trigger] path@[j].to,
        decreases path.len() - i,
    { chain.push(path[i].to.clone()); i += 1; }
    if let Some(target) = tail { chain.push(target.clone()); }
    chain
}

}
