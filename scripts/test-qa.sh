#!/usr/bin/env bash

set -euo pipefail

source_script="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/qa.sh"
temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT

make_repo() {
  local name=$1 root="$temp_dir/$1"
  mkdir -p "$root/scripts" "$root/bin"
  cp "$source_script" "$root/scripts/qa.sh"
  chmod +x "$root/scripts/qa.sh"
  printf '%s\n' "$root"
}

install_mocks() {
  local root=$1
  cat > "$root/bin/git" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" == rev-parse ]]; then printf '0123456789abcdef\n'; else printf '%s' "${QA_GIT_STATUS:-}"; fi
EOF
  cat > "$root/bin/docker" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$QA_FAKE_LOG"
case "$1" in
  info) exit 0 ;;
  create)
    count=$(grep -c '^create ' "$QA_FAKE_LOG")
    [[ "${QA_FAIL_CREATE:-0}" -eq "$count" ]] && exit 42
    printf 'container-id\n' ;;
  start) [[ "${QA_FAIL_START:-false}" == true ]] && exit 43 || exit 0 ;;
  exec) [[ "$*" == *redis-cli* ]] && printf 'PONG\n'; exit 0 ;;
  port) [[ "$2" == *postgres* ]] && printf '127.0.0.1:15432\n' || printf '127.0.0.1:16379\n' ;;
  rm) [[ "${QA_FAIL_RM:-false}" == true ]] && exit 44 || exit 0 ;;
esac
EOF
  cat > "$root/bin/cargo" <<'EOF'
#!/usr/bin/env bash
root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
printf '%s\n' "$*" > "$root/cargo-args.log"
env > "$root/cargo-env.log"
if [[ -f "$root/cargo-output" ]]; then cat "$root/cargo-output"; else printf 'running 1 test\n'; fi
if [[ -f "$root/cargo-status" ]]; then exit "$(cat "$root/cargo-status")"; fi
exit 0
EOF
  chmod +x "$root/bin/"*
}

run_case() {
  local root=$1
  shift
  set +e
  (export QA_FAKE_LOG="$root/docker.log"; export PATH="$root/bin:$PATH"; "$@" "$root/scripts/qa.sh") >"$root/output.log" 2>&1
  CASE_STATUS=$?
  set -e
  CASE_RUN_FILE="$(find "$root/target/qa" -name run.txt -print -quit)"
}

root=$(make_repo success); install_mocks "$root"
run_case "$root" env QA_GIT_STATUS='?? local-file' DATABASE_URL='postgresql://ambient' REDIS_URL='redis://ambient' DISCORD_TOKEN=ambient HTTPS_PROXY='http://ambient' USERID_HASH_SALT='ambient-salt' CARGO_BUILD_JOBS=2
test "$CASE_STATUS" -eq 0
grep -qx 'test --locked --test qa_journeys -- --ignored --test-threads=1' "$root/cargo-args.log"
grep -q '^worktree_dirty=true$' "$CASE_RUN_FILE"
grep -q '^exit_status=0$' "$CASE_RUN_FILE"
grep -q '^USERID_HASH_SALT=annie-mei-disposable-qa-only$' "$root/cargo-env.log"
grep -q '^CARGO_BUILD_JOBS=2$' "$root/cargo-env.log"
grep -qx 'QA_DATABASE_URL=postgresql://annie_qa@127.0.0.1:15432/annie_mei' "$root/cargo-env.log"
grep -qx 'REDIS_URL=redis://127.0.0.1:16379/' "$root/cargo-env.log"
if grep -Eq 'ambient|^DATABASE_URL=|^DISCORD_TOKEN=|^HTTPS_PROXY=' "$root/cargo-env.log"; then
  echo 'QA child inherited sensitive environment' >&2
  exit 1
fi
test "$(grep -c '^rm --force --volumes annie-qa-' "$root/docker.log")" -eq 2

root=$(make_repo cargo-failure); install_mocks "$root"
printf '23\n' > "$root/cargo-status"
run_case "$root" env
test "$CASE_STATUS" -eq 23; grep -q '^exit_status=23$' "$CASE_RUN_FILE"
test "$(grep -c '^rm --force --volumes annie-qa-' "$root/docker.log")" -eq 2

root=$(make_repo partial-start); install_mocks "$root"
run_case "$root" env QA_FAIL_CREATE=2
test "$CASE_STATUS" -eq 42; grep -q '^exit_status=42$' "$CASE_RUN_FILE"
test "$(grep -c '^rm --force --volumes annie-qa-postgres-' "$root/docker.log")" -eq 1
if grep -q '^rm .*annie-qa-redis-' "$root/docker.log"; then exit 1; fi

root=$(make_repo start-failure); install_mocks "$root"
run_case "$root" env QA_FAIL_START=true
test "$CASE_STATUS" -eq 43; grep -q '^exit_status=43$' "$CASE_RUN_FILE"
test "$(grep -c '^rm --force --volumes annie-qa-postgres-' "$root/docker.log")" -eq 1

root=$(make_repo cleanup-failure); install_mocks "$root"
run_case "$root" env QA_FAIL_RM=true
test "$CASE_STATUS" -eq 1; grep -q '^exit_status=1$' "$CASE_RUN_FILE"

root=$(make_repo zero-tests); install_mocks "$root"
printf 'running 0 tests\n' > "$root/cargo-output"
run_case "$root" env
test "$CASE_STATUS" -eq 1; grep -q '^exit_status=1$' "$CASE_RUN_FILE"

printf 'scripts/qa.sh focused tests passed\n'
