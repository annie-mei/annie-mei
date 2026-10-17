# Disposable QA journeys

Run the ignored application journeys with:

```sh
scripts/qa.sh
```

The runner requires `docker`, a running Docker daemon, `cargo`, and `git`. It creates uniquely named PostgreSQL 16 and Redis 7 containers bound to random loopback ports, waits up to 60 seconds for both, and runs exactly:

```sh
cargo test --locked --test qa_journeys -- --ignored --test-threads=1
```

The test process receives a clean environment. `QA_DATABASE_URL` and the production `REDIS_URL` point only at the newly created services; `QA_RUN_ID` identifies the run. A fixed, synthetic `USERID_HASH_SALT` is supplied only inside this clean QA environment. A small allowlist retains local Rust/build necessities (including `CARGO_BUILD_JOBS`), while ambient bot, vendor, telemetry, proxy, and shared database credentials are excluded. Containers and their anonymous volumes are removed after success, failure, or an interrupt; cleanup failure fails an otherwise successful run. Revision, dirty-worktree state, timestamps, final exit status, and captured per-test output remain under `target/qa/<run-id>/`, including when provisioning fails; CI uploads that directory even when the run fails. A successful Cargo invocation is rejected if it ran zero journeys.

These journeys exercise the shared production execution path against real PostgreSQL and Redis. Discord HTTP and upstream APIs are simulated. They do **not** connect to the live Discord Gateway/client, call live vendors, or validate end-to-end UI rendering. Normal `cargo test` runs do not start containers because the journey tests are ignored and this runner invokes them explicitly.

## What the first journeys prove

- Open `/settings`, follow its returned button/menu IDs, select English titles, read the saved PostgreSQL row, reopen the panel, and verify its selected option. Then search for a synthetic anime whose English and Romaji titles differ and check the resulting embed.
- Repeat that search as a second user. Verify the default title preference remains independent, no second AniList request occurs, and real Redis contains the response with an expiring TTL.
- Change a server setting with `MANAGE_GUILD`, attempt another change without it, and verify the denial response and unchanged database value.
- Return an AniList HTTP failure or hold the response past the injected timeout. Both must complete the deferred response within the journey deadline.

Every interaction asserts the actual callback/edit routes, acknowledgement type, initial visibility flags, and HTTP ordering. Acknowledgement must arrive within three seconds, and before an upstream request. Successful edits receive valid Discord message JSON from the recording server, not an empty object.

The failure scenarios intentionally preserve current behavior: upstream errors and timeouts use the same “I couldn't find that anime on AniList.” response as no results. Distinct outage copy is not implemented by this QA work. Gateway dispatch, populated guild-member caches/scores, OAuth, other commands, and visual Discord rendering are not covered by these journeys.

## Implementation and extending coverage

`CommandContext` projects HTTP, cache, and shared application data from Serenity without a shard; production and tests call the same `dispatch_command`/`dispatch_component` functions. An explicit `AniListClient` carries the HTTP endpoint and timeout through the existing query, parsing, fuzzy-match, and cache code. Production builds that client at startup using the existing endpoint and ten-second timeout. Client-construction failure now stops startup; network failures retain their existing handling.

Add journeys in `tests/qa_journeys.rs`, retaining the explicit ignore reason. Use synthetic payloads and local API fixtures, follow returned controls where possible, and assert database/wire outcomes independently of production helpers. Keep cases serial because production Redis configuration is initialized once per process; do not change environment variables inside tests. New external boundaries need explicit local dependencies before adding journeys for them.

Run `scripts/test-qa.sh` for the runner's failure/cleanup and environment-isolation checks, then `scripts/qa.sh` and `scripts/verify.sh` for workflow and normal regression coverage. CI publishes these results but does not configure branch protection, merge changes, or deploy a bot. A dirty-worktree result identifies the base revision plus local changes, not an immutable release build. Docker must be local and disposable test ports must not be forwarded outside the machine; neither service requires credentials. An uncatchable kill or machine failure can prevent cleanup, so the run ID is also recorded in each container's `dev.annie-mei.qa-run` label.
