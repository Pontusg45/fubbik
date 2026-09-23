use std::collections::BTreeMap;

use fubbik_api::search::parser::{QueryClause, clauses_to_query_string, parse_query_string};

fn c(field: &str, operator: &str, value: &str) -> QueryClause {
    QueryClause {
        field: field.into(),
        operator: operator.into(),
        value: value.into(),
        params: None,
        negate: None,
    }
}

#[test]
fn parses_every_catalogued_query_form() {
    // Given the inline inputs and test fixtures.
    // When
    // (input, expected clauses)
    let cases: Vec<(&str, Vec<QueryClause>)> = vec![
        ("type:reference", vec![c("type", "is", "reference")]),
        ("tag:api", vec![c("tag", "is", "api")]),
        // comma => any_of, value stays UNSPLIT at parse time
        ("tag:a,b", vec![c("tag", "any_of", "a,b")]),
        // trailing + => gte, + stripped
        ("connections:3+", vec![c("connections", "gte", "3")]),
        // NO trailing + => plain `is`, NOT gte. Looks like a bug; it is Node's behaviour.
        ("connections:3", vec![c("connections", "is", "3")]),
        // Nd => within, d stripped
        ("updated:30d", vec![c("updated", "within", "30")]),
        // NO trailing d => plain `is`, NOT within.
        ("updated:30", vec![c("updated", "is", "30")]),
        // case-sensitive: `Tag` is NOT `tag`
        ("Tag:api", vec![c("Tag", "is", "api")]),
        // bare words each become their own text/contains clause
        (
            "bare words",
            vec![
                c("text", "contains", "bare"),
                c("text", "contains", "words"),
            ],
        ),
        // quoted phrase is ONE clause, quotes stripped
        (
            "\"quoted phrase\"",
            vec![c("text", "contains", "quoted phrase")],
        ),
    ];
    for (input, expected) in cases {
        // Then
        assert_eq!(parse_query_string(input), expected, "input: {input}");
    }
}

#[test]
fn not_negates_only_the_next_clause() {
    // Given the inline inputs and test fixtures.
    // When
    let got = parse_query_string("NOT tag:deprecated type:note");
    // Then
    assert_eq!(
        got[0].negate,
        Some(true),
        "NOT must negate the clause that follows it"
    );
    assert_eq!(
        got[1].negate, None,
        "NOT must not leak onto the clause after that"
    );
}

#[test]
fn hops_attaches_to_the_most_recent_near_clause() {
    // Given the inline inputs and test fixtures.
    // When
    let got = parse_query_string("near:abc hops:2");
    // Then
    assert_eq!(got.len(), 1, "hops is not a standalone clause");
    assert_eq!(got[0].field, "near");
    assert_eq!(
        got[0]
            .params
            .as_ref()
            .unwrap()
            .get("hops")
            .map(String::as_str),
        Some("2")
    );
}

#[test]
fn hops_after_a_non_near_clause_is_silently_dropped() {
    // Given the inline inputs and test fixtures.
    // When
    // Looks like a bug. It is Node's behaviour: hops scans backwards for `near` only.
    let got = parse_query_string("affected-by:req-1 hops:3");
    // Then
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].field, "affected-by");
    assert!(
        got[0].params.is_none(),
        "hops must NOT attach to affected-by"
    );
}

#[test]
fn hops_with_no_preceding_near_is_a_no_op() {
    // Given the inline inputs and test fixtures.
    // When the operation is evaluated by the assertion.
    // Then
    assert_eq!(
        parse_query_string("hops:2"),
        vec![],
        "orphan hops produces no clause at all"
    );
}

#[test]
fn path_splits_on_arrow_into_from_to_params() {
    // Given the inline inputs and test fixtures.
    // When
    let got = parse_query_string("path:\"A\"->\"B\"");
    // Then
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].field, "path");
    assert_eq!(got[0].operator, "is");
    // NOTE: the brief's draft asserted `value == "from"`, but that confuses the
    // TS source's local variable name `from` with a literal string. The actual
    // TS line is `return { operator: "is", value: from, params: { from, to } }`
    // — `value` holds the *variable* `from`, i.e. "A". Verified by executing
    // the ported algorithm verbatim under bun; see task-2-report.md.
    assert_eq!(got[0].value, "A");
    let p = got[0].params.as_ref().unwrap();
    assert_eq!(p.get("from").map(String::as_str), Some("A"));
    assert_eq!(p.get("to").map(String::as_str), Some("B"));
}

// NOTE: the brief's draft used `near:"Auth Flow" hops:2` as the round-trip
// input. That does NOT round-trip in real Node: the tokenizer only special-
// cases a `"` that opens a *whole* token, so a quote glued onto `field:"..`
// is just an ordinary character. `near:"Auth` and `Flow"` end up as two
// separate tokens, splitting into a `near` clause (value "Auth") and a
// stray `text` clause (value `Flow"`) — verified by executing the ported
// algorithm verbatim under bun. Any `field:"value with a space"` suffers
// this, not just `near`. So round-tripping a value containing a space only
// works for the `text` field, where the value arrives as a standalone
// quoted token the tokenizer *does* handle correctly. This test pins both
// the params round trip (via a space-free `near` value) and the
// space-quoting round trip (via a `text` clause).
#[test]
fn serialiser_round_trips_params_and_quotes_text_values_containing_spaces() {
    // Given
    let clauses = parse_query_string("near:AuthFlow hops:2");
    // When
    let s = clauses_to_query_string(&clauses);
    // Then
    assert_eq!(s, "near:AuthFlow hops:2");
    assert_eq!(
        parse_query_string(&s),
        clauses,
        "round trip must be lossless"
    );

    let clauses = parse_query_string("\"quoted phrase\" tag:api");
    let s = clauses_to_query_string(&clauses);
    assert!(
        s.contains("\"quoted phrase\""),
        "values with spaces must be re-quoted, got: {s}"
    );
    assert_eq!(
        parse_query_string(&s),
        clauses,
        "round trip must be lossless"
    );
}

/// C1: pins the exact clause list for `near:"Auth Flow" hops:2`, the full
/// truth captured in
/// `tests/fixtures/node-contract-2c/search-parse-near-quoted-hops-2.json`:
/// `{"clauses":[{"field":"near","operator":"is","value":"Auth","params":{"hops":"2"}},{"field":"text","operator":"contains","value":"Flow\""}]}`.
/// Only a NEGATIVE used to be asserted here (that no clause equals the
/// literal `"Auth Flow"`) — see the NOTE above on why that round-trip input
/// doesn't behave the way a naive reading suggests. This test locks the
/// full positive shape: the tokenizer only special-cases a `"` that opens a
/// *whole* token, so `near:"Auth` and `Flow"` become two separate tokens —
/// a `near` clause valued `"Auth"` (quote-opening token, `hops:2` attaches
/// to it as usual) **plus** a stray `text` clause valued `Flow"` (note the
/// trailing double-quote baked into the value itself, since the tokenizer
/// only strips a *leading* quote from a token that starts with one, not a
/// trailing one that isn't the whole token). This looks exactly like a
/// tokenizer bug that a future reader would "clean up" — it is Node's
/// verified behaviour, reproduced byte-for-byte.
#[test]
fn near_quoted_with_a_space_produces_a_near_clause_plus_a_stray_text_clause() {
    // Given the inline inputs and test fixtures.
    // When
    let got = parse_query_string("near:\"Auth Flow\" hops:2");

    let mut hops = BTreeMap::new();
    hops.insert("hops".to_string(), "2".to_string());

    // Then
    assert_eq!(
        got,
        vec![
            QueryClause {
                field: "near".into(),
                operator: "is".into(),
                value: "Auth".into(),
                params: Some(hops),
                negate: None,
            },
            QueryClause {
                field: "text".into(),
                operator: "contains".into(),
                value: "Flow\"".into(),
                params: None,
                negate: None,
            },
        ],
        "must match tests/fixtures/node-contract-2c/search-parse-near-quoted-hops-2.json exactly"
    );
}
