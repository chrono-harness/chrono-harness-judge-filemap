//! Selection over paths from the two registered endpoints and the supplied DELTA.
use std::collections::BTreeSet;
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

pub fn admit_rules<K: Ord>(delta: &BTreeSet<K>, rules: &[K]) -> (ok: bool)
    requires vstd::laws_cmp::obeys_cmp::<K>(),
    ensures ok == (forall|i: int| 0 <= i < rules.len() ==> delta@.contains(#[trigger] rules@[i])),
{
    broadcast use vstd::std_specs::btree::axiom_set_contains_deref_key;
    let mut i = 0;
    while i < rules.len()
        invariant vstd::laws_cmp::obeys_cmp::<K>(), i <= rules.len(),
            forall|j: int| 0 <= j < i ==> delta@.contains(#[trigger] rules@[j]),
        decreases rules.len() - i,
    {
        if !delta.contains(&rules[i]) { return false; }
        i += 1;
    }
    true
}

pub fn schema_paths<K: Ord + Copy>(
    old_path: K, new_path: K, before: u64, after: u64,
) -> (paths: BTreeSet<K>)
    requires vstd::laws_cmp::obeys_cmp::<K>(),
    ensures paths@ == if before == after { Set::empty() } else { set![old_path, new_path] },
{
    let mut paths = BTreeSet::new();
    if before != after {
        paths.insert(old_path);
        paths.insert(new_path);
    }
    paths
}

pub fn migration_applies(
    before: u64, after: u64, from: Option<u64>, to: Option<u64>, has_test: bool,
) -> (applies: bool)
    ensures applies == (from == Some(before) && to == Some(after) && has_test),
{
    matches!(from, Some(version) if version == before)
        && matches!(to, Some(version) if version == after) && has_test
}

pub fn stability_paths<K: Ord + Copy>(
    delta: &BTreeSet<K>, declared: &[K], registry: K, record_changed: bool,
) -> (paths: BTreeSet<K>)
    requires vstd::laws_cmp::obeys_cmp::<K>(),
    ensures forall|k: K| #[trigger] paths@.contains(k) ==
        ((delta@.contains(k) && declared@.contains(k)) || (record_changed && k == registry)),
{
    broadcast use vstd::std_specs::btree::axiom_set_contains_deref_key;
    let mut paths = BTreeSet::new();
    let mut i = 0;
    while i < declared.len()
        invariant vstd::laws_cmp::obeys_cmp::<K>(), i <= declared.len(),
            forall|k: K| #[trigger] paths@.contains(k) ==
                (delta@.contains(k) && exists|j: int| 0 <= j < i && #[trigger] declared@[j] == k),
        decreases declared.len() - i,
    {
        let p = declared[i];
        if delta.contains(&p) { paths.insert(p); }
        i += 1;
    }
    if record_changed { paths.insert(registry); }
    paths
}

pub fn integration_required<K: Ord + Copy>(
    paths: &mut BTreeSet<K>, base: bool, candidate: bool, rules: &[K],
) -> (required: bool)
    requires vstd::laws_cmp::obeys_cmp::<K>(),
    ensures paths@ == if base || candidate { old(paths)@.union(rules@.to_set()) } else { old(paths)@ },
        required == !paths@.is_empty(),
{
    if base || candidate {
        let mut i = 0;
        while i < rules.len()
            invariant vstd::laws_cmp::obeys_cmp::<K>(), i <= rules.len(),
                forall|k: K| #[trigger] paths@.contains(k) ==
                    (old(paths)@.contains(k) || exists|j: int| 0 <= j < i && #[trigger] rules@[j] == k),
            decreases rules.len() - i,
        {
            paths.insert(rules[i]);
            i += 1;
        }
        assert(paths@ =~= old(paths)@.union(rules@.to_set()));
    }
    !paths.is_empty()
}

}
