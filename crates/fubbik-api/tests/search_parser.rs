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
        assert_eq!(parse_query_string(input), expected, "input: {input}");
    }
}

#[test]
fn not_negates_only_the_next_clause() {
    let got = parse_query_string("NOT tag:deprecated type:note");
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
    let got = parse_query_string("near:abc hops:2");
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
    // Looks like a bug. It is Node's behaviour: hops scans backwards for `near` only.
    let got = parse_query_string("affected-by:req-1 hops:3");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].field, "affected-by");
    assert!(
        got[0].params.is_none(),
        "hops must NOT attach to affected-by"
    );
}

#[test]
fn hops_with_no_preceding_near_is_a_no_op() {
    assert_eq!(
        parse_query_string("hops:2"),
        vec![],
        "orphan hops produces no clause at all"
    );
}

#[test]
fn path_splits_on_arrow_into_from_to_params() {
    let got = parse_query_string("path:\"A\"->\"B\"");
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
    let clauses = parse_query_string("near:AuthFlow hops:2");
    let s = clauses_to_query_string(&clauses);
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
