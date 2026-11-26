//! Finite union/closure mechanics. Callers own endpoint validation, seeds and extra selections.
#![allow(unexpected_cfgs)]

pub mod named {
    #[cfg(verus_keep_ghost)]
    use super::super::{CauseSet, PredecessorId, byte_lt, id_obeys_cmp, ordered_names};
    #[cfg(not(verus_keep_ghost))]
    use crate::verified_graph::{CauseSet, PredecessorId};
    use std::collections::BTreeMap;
    use vstd::prelude::*;
    #[cfg(verus_keep_ghost)]
    use vstd::{std_specs::cmp::OrdSpec, string::StringSliceAdditionalSpecFns};

    verus! {

    spec fn named_entries(names: Seq<&str>, entries: Seq<(usize, CauseSet)>) -> Set<(Seq<u8>, Set<Seq<u8>>)> {
        entries.map_values(|p:(usize, CauseSet)| (names[p.0 as int].spec_bytes(), p.1.keys().to_set())).to_set()
    }
    pub open spec fn reached_rows(m: Map<PredecessorId, CauseSet>) -> Set<(Seq<u8>, Set<Seq<u8>>)> {
        m.kv_pairs().map(|p:(PredecessorId, CauseSet)| (p.0.key(),p.1.keys().to_set()))
    }
    pub open spec fn named_members(names: Seq<&str>, nodes: Map<usize,CauseSet>) -> Set<(Seq<u8>, Set<Seq<u8>>)> {
        nodes.kv_pairs().map(|p:(usize, CauseSet)| (names[p.0 as int].spec_bytes(), p.1.keys().to_set()))
    }
    pub fn named_nodes(names: &[&str], nodes: &BTreeMap<usize,CauseSet>) -> (out: BTreeMap<PredecessorId,CauseSet>)
        requires ordered_names(names@), forall |i:usize| nodes@.contains_key(i) ==> i < names.len(),
        ensures reached_rows(out@) == named_members(names@,nodes@),
    {
        proof { id_obeys_cmp(); }
        let mut out: BTreeMap<PredecessorId, CauseSet> = BTreeMap::new();
        let it = nodes.iter(); let ghost entries = it@.1;
        for pair in cursor: it
            invariant vstd::laws_cmp::obeys_cmp::<PredecessorId>(), cursor.kv_pairs == entries, entries.to_set()==nodes@.kv_pairs(),
                vstd::std_specs::btree::increasing_seq(entries.map(|_i:int,p:(usize,CauseSet)| p.0)),
                ordered_names(names@),
                forall |i:usize| nodes@.contains_key(i) ==> i < names.len(),
                reached_rows(out@) == named_entries(names@,entries.take(cursor.pos)),
        {
            let (index,causes)=pair;
            let ghost pos=cursor.pos;
            proof { assert(entries.to_set().contains((*index,*causes))); }
            let label=PredecessorId(names[*index].to_string()); let ghost key=label;
            let ghost before=out@;
            proof {
                assert(!before.contains_key(key)) by {
                    if before.contains_key(key) {
                        let prev=(key, before[key]);
                        assert(before.kv_pairs().contains(prev));
                        assert(reached_rows(before).contains((key.key(),before[key].keys().to_set())));
                        let vals=entries.take(pos).map_values(|p:(usize,CauseSet)| (names@[p.0 as int].spec_bytes(),p.1.keys().to_set()));
                        assert(vals.to_set().contains((key.key(),before[key].keys().to_set())));
                        let j=choose|j:int| 0<=j<vals.len() && vals[j]==(key.key(),before[key].keys().to_set());
                        let keys=entries.map(|_i:int,p:(usize,CauseSet)|p.0);
                        vstd::std_specs::btree::axiom_increasing_seq_meaning(keys);
                        assert(0 <= j < pos < keys.len());
                        assert(keys[j].cmp_spec(&keys[pos]) is Less);
                        assert(keys[j] < keys[pos]);
                        assert(names@[keys[j] as int].spec_bytes() == key.key());
                        assert(byte_lt(names@[keys[j] as int].spec_bytes(),names@[*index as int].spec_bytes()));
                    }
                }
            }
            out.insert(label,causes.clone());
            proof {
                let inserted=out@[key];
                assert(inserted.keys().to_set()==causes.keys().to_set());
                assert(out@.kv_pairs() =~= before.kv_pairs().insert((key,inserted)));
                let added=(key.key(),inserted.keys().to_set());
                let named = |p:(PredecessorId,CauseSet)| (p.0.key(),p.1.keys().to_set());
                before.kv_pairs().lemma_set_map_insert_commute((key,inserted),named);
                assert(reached_rows(out@) =~= reached_rows(before).insert(added));
                let to_name=|p:(usize,CauseSet)| (names@[p.0 as int].spec_bytes(),p.1.keys().to_set());
                entries.take(pos).map_values(to_name).lemma_push_to_set_commute(added);
                assert(entries.take(pos+1).map_values(to_name) =~= entries.take(pos).map_values(to_name).push(added));
            }
        }
        proof {
            assert(entries.take(entries.len() as int) =~= entries);
            assert(named_entries(names@,entries) =~= named_members(names@,nodes@)) by {
                let by_name=|p:(usize,CauseSet)| (names@[p.0 as int].spec_bytes(),p.1.keys().to_set());
                assert forall |q:(Seq<u8>,Set<Seq<u8>>)| named_entries(names@,entries).contains(q) == named_members(names@,nodes@).contains(q) by {
                    if named_entries(names@,entries).contains(q) {
                        let i=choose|i:int| 0<=i<entries.len() && entries.map_values(by_name)[i]==q;
                        assert(entries.to_set().contains(entries[i]));
                    } else if named_members(names@,nodes@).contains(q) {
                        let kv=choose|kv:(usize,CauseSet)| nodes@.kv_pairs().contains(kv) && by_name(kv)==q;
                        let i=choose|i:int| 0<=i<entries.len() && entries[i]==kv;
                        assert(entries.map_values(by_name)[i]==q);
                    }
                }
            }
        }
        out
    }
    }
}

#[cfg(not(verus_keep_ghost))]
pub use native::*;
#[cfg(not(verus_keep_ghost))]
mod native {
    use serde::{Deserialize, Serialize};
    use std::collections::{BTreeMap, BTreeSet};

    pub use crate::verified_graph::{
        CauseSet, Closure, Edge, EdgeKind, Origin, Predecessor, PredecessorId, PredecessorTree,
        Seed, TraversedEdge, UnionEdge, union,
    };

    impl std::fmt::Debug for EdgeKind {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(match self {
                Self::Compile => "Compile",
                Self::BuildInput => "BuildInput",
                Self::RuntimeInput => "RuntimeInput",
                Self::TestExecution => "TestExecution",
                Self::JudgeTrigger => "JudgeTrigger",
            })
        }
    }
    impl Serialize for EdgeKind {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(self.wire_name())
        }
    }
    impl<'de> Deserialize<'de> for EdgeKind {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct KindVisitor;
            impl serde::de::Visitor<'_> for KindVisitor {
                type Value = EdgeKind;
                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a canonical FILEMAP edge kind")
                }
                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<EdgeKind, E> {
                    EdgeKind::from_wire(value)
                        .ok_or_else(|| E::invalid_value(serde::de::Unexpected::Str(value), &self))
                }
            }
            deserializer.deserialize_str(KindVisitor)
        }
    }
    impl std::fmt::Debug for Edge {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("Edge")
                .field("from", &self.from)
                .field("kind", &self.kind)
                .field("to", &self.to)
                .finish()
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Edge", rename = "Edge")]
    struct EdgeFields {
        from: String,
        kind: EdgeKind,
        to: String,
    }
    impl Serialize for Edge {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            EdgeFields::serialize(self, serializer)
        }
    }
    impl<'de> Deserialize<'de> for Edge {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            EdgeFields::deserialize(deserializer)
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Origin", rename_all = "lowercase")]
    enum OriginFields {
        Base,
        Candidate,
        Both,
    }
    impl Serialize for Origin {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            OriginFields::serialize(self, serializer)
        }
    }
    impl<'de> Deserialize<'de> for Origin {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            OriginFields::deserialize(deserializer)
        }
    }
    impl std::fmt::Debug for Origin {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(match self {
                Self::Base => "Base",
                Self::Candidate => "Candidate",
                Self::Both => "Both",
            })
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "UnionEdge", rename = "UnionEdge")]
    struct UnionEdgeFields {
        #[serde(flatten)]
        edge: Edge,
        origin: Origin,
    }
    impl Serialize for UnionEdge {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            UnionEdgeFields::serialize(self, serializer)
        }
    }
    impl<'de> Deserialize<'de> for UnionEdge {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            UnionEdgeFields::deserialize(deserializer)
        }
    }
    impl std::fmt::Debug for UnionEdge {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("UnionEdge")
                .field("edge", &self.edge)
                .field("origin", &self.origin)
                .finish()
        }
    }
    pub(crate) struct JsonSeedId;

    impl crate::verified_graph::SeedEncoder for JsonSeedId {
        fn encode(&self, fields: [&str; 3]) -> String {
            serde_json::to_string(&fields).expect("three string fields serialize as a JSON array")
        }
    }

    impl std::fmt::Debug for Seed {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("Seed")
                .field("id", &self.id)
                .field("node", &self.node)
                .field("reference", &self.reference)
                .field("reason", &self.reason)
                .finish()
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Seed", rename = "Seed")]
    struct SeedFields {
        id: String,
        node: String,
        reference: String,
        reason: String,
    }
    impl Serialize for Seed {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            SeedFields::serialize(self, serializer)
        }
    }
    impl<'de> Deserialize<'de> for Seed {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            SeedFields::deserialize(deserializer)
        }
    }
    impl std::fmt::Debug for PredecessorTree {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_map()
                .entries(self.entries().iter().map(|row| (&row.node, &row.parent)))
                .finish()
        }
    }
    impl Serialize for PredecessorTree {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(Some(self.entries().len()))?;
            for row in self.entries() {
                map.serialize_entry(&row.node, &row.parent)?;
            }
            map.end()
        }
    }
    impl<'de> Deserialize<'de> for PredecessorTree {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let rows = BTreeMap::<String, Option<Edge>>::deserialize(deserializer)?
                .into_iter()
                .map(|(node, parent)| Predecessor { node, parent })
                .collect();
            Self::new(rows).ok_or_else(|| {
                serde::de::Error::custom("predecessor nodes must be strictly ordered")
            })
        }
    }
    impl std::fmt::Debug for CauseSet {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.debug_set().entries(self.entries()).finish()
        }
    }
    impl Serialize for CauseSet {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.entries().serialize(serializer)
        }
    }
    impl<'de> Deserialize<'de> for CauseSet {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let names = Vec::<String>::deserialize(deserializer)?;
            Ok(Self::from_names(&names))
        }
    }
    impl crate::verified_graph::ReachedNodes {
        pub fn get(&self, node: &str) -> Option<&CauseSet> {
            self.entries.get(node)
        }
        pub fn contains_key(&self, node: &str) -> bool {
            self.entries.contains_key(node)
        }
        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
        pub fn keys(&self) -> impl DoubleEndedIterator<Item = &String> + ExactSizeIterator {
            self.entries.keys().map(|node| &node.0)
        }
        pub fn into_keys(self) -> impl DoubleEndedIterator<Item = String> + ExactSizeIterator {
            self.entries.into_keys().map(|node| node.0)
        }
    }

    impl std::ops::Index<&str> for crate::verified_graph::ReachedNodes {
        type Output = CauseSet;
        fn index(&self, node: &str) -> &CauseSet {
            &self.entries[node]
        }
    }

    impl<'a> IntoIterator for &'a crate::verified_graph::ReachedNodes {
        type Item = (&'a String, &'a CauseSet);
        type IntoIter = std::iter::Map<
            std::collections::btree_map::Iter<'a, PredecessorId, CauseSet>,
            fn((&'a PredecessorId, &'a CauseSet)) -> Self::Item,
        >;
        fn into_iter(self) -> Self::IntoIter {
            self.entries.iter().map(|(node, seeds)| (&node.0, seeds))
        }
    }

    impl std::borrow::Borrow<str> for PredecessorId {
        fn borrow(&self) -> &str {
            &self.0
        }
    }

    /// Each ID identifies one root in the public predecessor dictionary.
    pub fn closure(edges: &[UnionEdge], seeds: &[Seed]) -> Result<Closure, String> {
        if let Some((first, second)) = crate::verified_graph::conflicting_seed_roots(seeds) {
            return Err(format!(
                "E_SEED_ID_CONFLICT: seed ID {:?} refers to both {:?} and {:?}",
                seeds[first].id, seeds[first].node, seeds[second].node
            ));
        }
        Ok(closure_for_bound_roots(edges, seeds))
    }

    /// The impact producer encodes the root node in every ID. Public callers first check this binding.
    pub(crate) fn closure_for_bound_roots(edges: &[UnionEdge], seeds: &[Seed]) -> Closure {
        let topology = crate::verified_graph::seed_topology(edges, seeds);
        crate::verified_graph::publish_closure(&topology, seeds)
    }

    impl Closure {
        pub fn selected_tests(&self) -> BTreeSet<String> {
            BTreeSet::from_iter(crate::verified_graph::selected_test_targets(
                &self.traversed,
            ))
        }
        pub fn witness(&self, seed: &str, node: &str) -> Option<Vec<Edge>> {
            self.predecessors.get(seed)?.witness(node)
        }
    }

    /// Compute both endpoint origins and the traversed subgraph for scoped CI replay.
    pub fn query_closure(input: impl std::io::Read) -> Result<serde_json::Value, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            base: BTreeSet<Edge>,
            candidate: BTreeSet<Edge>,
            seeds: Vec<Seed>,
            reachable_only: bool,
        }
        let request: Input = serde_json::from_reader(input).map_err(|e| e.to_string())?;
        let edges = union(&request.base, &request.candidate);
        let result = closure(&edges, &request.seeds)?;
        let selected = result.selected_tests();
        let reached: Vec<_> = result.reached.keys().collect();
        let mut response = serde_json::json!({"selected_tests":selected,"reached":reached});
        if request.reachable_only {
            let reachable: Vec<_> = edges
                .iter()
                .filter(|edge| result.reached.contains_key(&edge.edge.from))
                .collect();
            response["edges"] = serde_json::json!(reachable);
        } else {
            response["edges"] = serde_json::json!(edges);
            response["closure"] = serde_json::json!(result);
        }
        Ok(response)
    }
}
