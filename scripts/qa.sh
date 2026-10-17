#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_dir"

for command in docker cargo git; do
  command -v "$command" >/dev/null 2>&1 || { echo "qa: required command not found: $command" >&2; exit 127; }
done
docker info >/dev/null 2>&1 || { echo "qa: Docker daemon is unavailable" >&2; exit 1; }

run_id="$(date -u +%Y%m%dT%H%M%SZ)-$$-$RANDOM"
postgres_container="annie-qa-postgres-$run_id"
redis_container="annie-qa-redis-$run_id"
results_dir="$repo_dir/target/qa/$run_id"
mkdir -p "$results_dir"
revision="$(git rev-parse HEAD)"
if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then dirty=true; else dirty=false; fi
printf 'qa_run_id=%s\nrevision=%s\nworktree_dirty=%s\nstarted_at=%s\n' \
  "$run_id" "$revision" "$dirty" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$results_dir/run.txt"

postgres_created=false
redis_created=false
cleanup() {
  local status=$? cleanup_status=0
  trap - EXIT INT TERM
  if [[ "$redis_created" == true ]]; then
    docker rm --force --volumes "$redis_container" >/dev/null 2>&1 || cleanup_status=1
  fi
  if [[ "$postgres_created" == true ]]; then
    docker rm --force --volumes "$postgres_container" >/dev/null 2>&1 || cleanup_status=1
  fi
  if [[ "$cleanup_status" -ne 0 && "$status" -eq 0 ]]; then
    echo "qa: failed to remove one or more disposable services" >&2
    status=1
  fi
  printf 'exit_status=%s\nfinished_at=%s\n' "$status" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$results_dir/run.txt"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Record creation before starting: a start failure can leave a container behind.
docker create --name "$postgres_container" \
  --env POSTGRES_HOST_AUTH_METHOD=trust \
  --label "dev.annie-mei.qa-run=$run_id" --publish 127.0.0.1::5432 \
  --env POSTGRES_USER=annie_qa --env POSTGRES_DB=annie_mei postgres:16-alpine >/dev/null
postgres_created=true
docker start "$postgres_container" >/dev/null
docker create --name "$redis_container" \
  --label "dev.annie-mei.qa-run=$run_id" --publish 127.0.0.1::6379 \
  redis:7-alpine >/dev/null
redis_created=true
docker start "$redis_container" >/dev/null

ready=false
for _ in $(seq 1 60); do
  if docker exec "$postgres_container" pg_isready -U annie_qa -d annie_mei >/dev/null 2>&1 \
    && docker exec "$redis_container" redis-cli ping 2>/dev/null | grep -qx PONG; then
    ready=true
    break
  fi
  sleep 1
done
if [[ "$ready" != true ]]; then
  echo "qa: disposable services did not become ready within 60 seconds" >&2
  exit 1
fi

postgres_port="$(docker port "$postgres_container" 5432/tcp | sed -n 's/.*:\([0-9][0-9]*\)$/\1/p' | head -n1)"
redis_port="$(docker port "$redis_container" 6379/tcp | sed -n 's/.*:\([0-9][0-9]*\)$/\1/p' | head -n1)"
if [[ -z "$postgres_port" || -z "$redis_port" ]]; then
  echo "qa: could not determine disposable service ports" >&2
  exit 1
fi

allowed_env=(
  "PATH=$PATH"
  "HOME=${HOME:-$repo_dir}"
  "QA_RUN_ID=$run_id"
  "QA_DATABASE_URL=postgresql://annie_qa@127.0.0.1:$postgres_port/annie_mei"
  "REDIS_URL=redis://127.0.0.1:$redis_port/"
  "USERID_HASH_SALT=annie-mei-disposable-qa-only"
)
for variable in CARGO_HOME RUSTUP_HOME CARGO_TARGET_DIR RUSTC_WRAPPER SCCACHE_DIR TMPDIR TERM CARGO_TERM_COLOR CARGO_BUILD_JOBS SSL_CERT_FILE SSL_CERT_DIR RUSTFLAGS; do
  if [[ -n "${!variable:-}" ]]; then allowed_env+=("$variable=${!variable}"); fi
done

printf 'qa: run %s at revision %s (dirty: %s)\n' "$run_id" "$revision" "$dirty"
set +e
env -i "${allowed_env[@]}" cargo test --locked --test qa_journeys -- --ignored --test-threads=1 \
  2>&1 | tee "$results_dir/test-results.log"
statuses=("${PIPESTATUS[@]}")
status=${statuses[0]}
if [[ "$status" -eq 0 && "${statuses[1]}" -ne 0 ]]; then status=${statuses[1]}; fi
set -e
if [[ "$status" -eq 0 ]] && ! grep -Eq '^running [1-9][0-9]* tests?$' "$results_dir/test-results.log"; then
  echo "qa: test command succeeded without running any ignored journeys" >&2
  exit 1
fi
exit "$status"
