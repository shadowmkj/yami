use quickcheck_macros::quickcheck;
use yami::parse;

/// Invariant: `parse` must never panic on arbitrary string inputs.
/// It must return either `Ok(Yaml)` or `Err(YamlError)`.
#[quickcheck]
fn prop_parse_never_panics(input: String) -> bool {
    let _ = parse(&input);
    true
}

/// Invariant: Repeatedly parsing the same input produces identical results.
#[quickcheck]
fn prop_parse_is_deterministic(input: String) -> bool {
    let res1 = parse(&input);
    let res2 = parse(&input);
    res1 == res2
}
