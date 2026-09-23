# Database tests

Run the PostgreSQL integration tests against the dedicated test adapter:

```sh
./scripts/rust-test-db.sh start
pnpm --filter @fubbik/db test
./scripts/rust-test-db.sh stop
```

The adapter provides PostgreSQL 18, pgvector, and Apache AGE. Vitest creates a uniquely named database, applies the committed Rust SQL migrations, supplies that connection to every worker, and drops the database after the run. It never migrates the database named by the application’s `DATABASE_URL`.

To use another **test** PostgreSQL server with database creation privileges, set `FUBBIK_TEST_DATABASE_URL`. The default is `postgres://postgres:password@localhost:5434/postgres`. With Apache AGE installed, the graph integration tests run as well.

`pnpm test` includes these integration tests, so start the adapter first. Turborepo does not cache this package’s test results. CI starts and stops the same adapter.
