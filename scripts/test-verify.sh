#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
verify_script="$script_dir/verify.sh"

expected_commands=(
  'cargo fmt --check'
  'cargo test --all-features'
  'cargo clippy --all-targets --all-features -- -D warnings'
  'git diff HEAD --check'
)

grep -qx 'set -euo pipefail' "$verify_script"

previous_line=0
for command in "${expected_commands[@]}"; do
  line="$(grep -nxF "$command" "$verify_script" | cut -d: -f1)"
  test -n "$line"
  test "$line" -gt "$previous_line"
  previous_line="$line"
done

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT
cat > "$temp_dir/cargo" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$VERIFY_LOG"
exit 17
EOF
chmod +x "$temp_dir/cargo"

set +e
PATH="$temp_dir:$PATH" VERIFY_LOG="$temp_dir/log" "$verify_script"
status=$?
set -e

test "$status" -eq 17
test "$(cat "$temp_dir/log")" = 'fmt --check'

repo_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir" "$repo_dir"' EXIT
git -C "$repo_dir" init --quiet
git -C "$repo_dir" config user.email verify@example.com
git -C "$repo_dir" config user.name Verify
git -C "$repo_dir" config commit.gpgSign false
printf 'baseline\n' > "$repo_dir/notes.txt"
git -C "$repo_dir" add notes.txt
git -C "$repo_dir" commit --quiet -m baseline
printf 'trailing whitespace \n' > "$repo_dir/notes.txt"
git -C "$repo_dir" add notes.txt

mkdir "$temp_dir/success-bin"
cat > "$temp_dir/success-bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$temp_dir/success-bin/cargo"

set +e
(
  cd "$repo_dir"
  PATH="$temp_dir/success-bin:$PATH" "$verify_script"
)
status=$?
set -e

test "$status" -ne 0
