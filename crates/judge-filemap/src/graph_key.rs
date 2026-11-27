//! Byte ordering for graph keys, including empty names and embedded zeroes.
use vstd::prelude::*;

verus! {

pub open spec fn byte_prefix(a: Seq<u8>, b: Seq<u8>, n: int) -> bool {
    0 <= n <= a.len() && n <= b.len() && forall|i: int| 0 <= i < n ==> a[i] == b[i]
}

// End of key sorts before every byte, including 0.
pub open spec fn digit(key: Seq<u8>, index: int) -> int {
    if index < key.len() { key[index] as int } else { -1 }
}

pub open spec fn byte_lt(a: Seq<u8>, b: Seq<u8>) -> bool {
    exists|i: int| #[trigger] byte_prefix(a, b, i) && digit(a, i) < digit(b, i)
}

pub proof fn byte_lt_strict(a: Seq<u8>, b: Seq<u8>)
    requires byte_lt(a, b),
    ensures a != b,
{}

pub proof fn byte_lt_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
    requires byte_lt(a, b), byte_lt(b, c),
    ensures byte_lt(a, c),
{
    let first = choose|i: int| #[trigger] byte_prefix(a, b, i) && digit(a, i) < digit(b, i);
    let second = choose|i: int| #[trigger] byte_prefix(b, c, i) && digit(b, i) < digit(c, i);
    let boundary = vstd::math::min(first, second);
    assert(byte_prefix(a, c, boundary));
    assert(digit(a, boundary) < digit(c, boundary));
}

pub proof fn byte_lt_asymmetric(a: Seq<u8>, b: Seq<u8>)
    ensures byte_lt(a, b) ==> !byte_lt(b, a),
{
    if byte_lt(a, b) && byte_lt(b, a) {
        byte_lt_transitive(a, b, a);
        byte_lt_strict(a, a);
    }
}

pub proof fn byte_trichotomy(a: Seq<u8>, b: Seq<u8>)
    ensures a == b || byte_lt(a, b) || byte_lt(b, a),
{
    let end = vstd::math::min(a.len() as int, b.len() as int);
    let differences = Seq::new(end as nat, |i: int| a[i] != b[i]);
    differences.index_of_first_ensures(true);
    let i = match differences.index_of_first(true) { Some(index) => index, None => end };
    assert(byte_prefix(a, b, i)) by {
        assert forall|j: int| 0 <= j < i implies a[j] == b[j] by {
            assert(!differences[j]);
        }
    }
    if digit(a, i) == digit(b, i) {
        assert(a =~= b);
    } else {
        assert(byte_prefix(b, a, i));
    }
}

/// Compare UTF-8 text using the native and verified graph key order.
pub fn equal_text(a: &str, b: &str) -> (equal: bool)
    ensures equal == (a@ == b@),
{
    let equal = compare_bytes(a.as_bytes(), b.as_bytes()) == 0;
    proof {
        vstd::utf8::encode_utf8_decode_utf8(a@);
        vstd::utf8::encode_utf8_decode_utf8(b@);
    }
    equal
}

/// Compare the shared portion once, then break a prefix tie by length.
pub fn compare_bytes(a: &[u8], b: &[u8]) -> (order: i8)
    ensures -1 <= order <= 1, order == 0 <==> a@ == b@,
        order == -1 ==> byte_lt(a@, b@), order == 1 ==> byte_lt(b@, a@),
{
    let end = if a.len() < b.len() { a.len() } else { b.len() };
    let mut prefix = 0;
    while prefix < end && a[prefix] == b[prefix]
        invariant prefix <= end <= a.len(), end <= b.len(), byte_prefix(a@, b@, prefix as int),
        decreases end - prefix,
    {
        prefix += 1;
    }
    assert(byte_prefix(b@, a@, prefix as int));
    let order = if prefix == end {
        if a.len() == b.len() { 0 } else if a.len() < b.len() { -1 } else { 1 }
    } else if a[prefix] < b[prefix] { -1 } else { 1 };
    proof {
        if order == 0 { assert(a@ =~= b@); }
        else if order < 0 { assert(digit(a@, prefix as int) < digit(b@, prefix as int)); }
        else { assert(digit(b@, prefix as int) < digit(a@, prefix as int)); }
    }
    order
}
}
