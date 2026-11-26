//! Compose the process history of the two registered Git readers.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

#[derive(Debug, PartialEq, Eq)]
pub enum MergeError {
    UnboundLocal,
    UnboundRegistration,
    MissingLocal,
    MissingRegistration,
    DifferentBinding,
    InvalidLocal,
    InvalidRegistration,
    MissingPrefix,
}

pub enum Reader<V, M> {
    Null,
    Other(V),
    Object(M),
}

pub enum Selection<V, M> {
    Original(Reader<V, M>),
    Bind(M, M),
}

/// Keep the original local value when the independent reader supplied no facts.
pub fn select_readers<V, M>(local: Reader<V, M>, registration: Option<Reader<V, M>>)
    -> (out: Result<Selection<V, M>, MergeError>)
    ensures match registration {
        None => out == Ok::<_, MergeError>(Selection::Original(local)),
        Some(right) => match (local, right) {
            (Reader::Null, Reader::Null) => out == Ok::<_, MergeError>(Selection::Original(local)),
            (Reader::Object(left), Reader::Object(other)) =>
                out == Ok::<_, MergeError>(Selection::<V, M>::Bind(left, other)),
            (Reader::Object(_), _) => out == Err::<Selection<V, M>, _>(MergeError::UnboundRegistration),
            _ => out == Err::<Selection<V, M>, _>(MergeError::UnboundLocal),
        },
    },
{
    let Some(right) = registration else { return Ok(Selection::Original(local)); };
    match (local, right) {
        (Reader::Null, Reader::Null) => Ok(Selection::Original(Reader::Null)),
        (Reader::Object(left), Reader::Object(right)) => Ok(Selection::Bind(left, right)),
        (Reader::Object(_), _) => Err(MergeError::UnboundRegistration),
        _ => Err(MergeError::UnboundLocal),
    }
}

pub fn merge_trace<T>(mut local: Vec<T>, at: usize, mut registration: Vec<T>)
    -> (out: Result<Vec<T>, MergeError>)
    ensures match out {
        Ok(rows) => at <= local.len() && rows@ == local@.take(at as int) + registration@ + local@.skip(at as int),
        Err(error) => error == MergeError::MissingPrefix && at > local.len(),
    },
{
    if at > local.len() {
        return Err(MergeError::MissingPrefix);
    }
    let mut suffix = local.split_off(at);
    local.append(&mut registration);
    local.append(&mut suffix);
    Ok(local)
}

pub fn bind_traces<T>(same_binding: bool, local: Option<Option<Vec<T>>>, at: usize, registration: Option<Option<Vec<T>>>)
    -> (out: Result<Vec<T>, MergeError>)
    ensures match (same_binding, local, registration) {
        (_, None, _) => out == Err::<Vec<T>, _>(MergeError::MissingLocal),
        (_, _, None) => out == Err::<Vec<T>, _>(MergeError::MissingRegistration),
        (false, _, _) => out == Err::<Vec<T>, _>(MergeError::DifferentBinding),
        (_, Some(None), _) => out == Err::<Vec<T>, _>(MergeError::InvalidLocal),
        (_, _, Some(None)) => out == Err::<Vec<T>, _>(MergeError::InvalidRegistration),
        (true, Some(Some(left)), Some(Some(right))) => match out {
            Ok(rows) => at <= left.len() && rows@ == left@.take(at as int) + right@ + left@.skip(at as int),
            Err(error) => error == MergeError::MissingPrefix && at > left.len(),
        },
    },
{
    let Some(local) = local else { return Err(MergeError::MissingLocal); };
    let Some(registration) = registration else { return Err(MergeError::MissingRegistration); };
    if !same_binding {
        return Err(MergeError::DifferentBinding);
    }
    let Some(left) = local else { return Err(MergeError::InvalidLocal); };
    let Some(right) = registration else { return Err(MergeError::InvalidRegistration); };
    merge_trace(left, at, right)
}
}
