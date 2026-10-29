# HTTP differential verification

The live comparison passed on 2026-09-26 against a disposable PostgreSQL 18,
pgvector, and AGE database. The Node/Elysia server and Rust/Axum server read
separate copies of the Rust demo seed. `./legacy/migration/scripts/differential.sh` copied only
the source database's public relational data into a fresh migrated database;
the source database was read through `pg_dump` and was never written by the
comparison script.

```text
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured
```

The fixture used `fubbik seed --scenario demo --quiet`. Its 52 chunk creation
and update timestamps were made distinct in the disposable source database
before the comparison. The seed writes many rows in one transaction, so they
otherwise share a timestamp and two valid SQL plans can return different
orders for Node's non-total `ORDER BY created_at`. This fixture preparation
does not alter application behavior or a user's knowledge base.

The comparison found one real mismatch: the Rust ordinary chunk list sorted
text matches by creation time, while the Node list sorts them by trigram title
similarity. `chunk::list` now uses the similarity ordering, and
`text_search_orders_by_title_similarity_before_creation_time` pins it with a
database test. The final run passed chunk, collection, plan, search,
workspace, settings, staleness, and other covered GET comparisons.

The differential harness covers GET responses, not mutation parity. The
production image smoke test and legacy runtime removal remain separate gates.
