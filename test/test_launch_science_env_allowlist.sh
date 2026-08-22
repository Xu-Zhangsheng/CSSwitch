#!/usr/bin/env bash
# Verifies launch-virtual-sandbox.sh starts Science from env -i allowlist.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$ROOT/scripts/launch-virtual-sandbox.sh"
TMP="$(mktemp -d /private/tmp/csswitch-science-env-allowlist.XXXXXX)"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

STUB_BIN="$TMP/fake-science"
cat > "$STUB_BIN" <<'STUB'
#!/bin/zsh
set -euo pipefail
if [[ "${1:-}" == "serve" ]]; then
  /usr/bin/env | /usr/bin/sort > "${HOME:?}/science-env-dump.txt"
  exit 0
fi
if [[ "${1:-}" == "--version" ]]; then
  echo "fake-science 0.0.0-test"
  exit 0
fi
exit 2
STUB
chmod 755 "$STUB_BIN"

SANDBOX_HOME="$TMP/sandbox-home"
HOST_HOME="$TMP/host-home"
mkdir -p "$SANDBOX_HOME/.claude-science" "$SANDBOX_HOME/Library/Keychains" "$HOST_HOME"
ENV_OUT="$SANDBOX_HOME/science-env-dump.txt"

export CSSWITCH_TEST_SENTINEL_SECRET="parent-sentinel-must-not-leak"
export OPENAI_API_KEY="sk-parent-must-not-leak"
export AWS_SECRET_ACCESS_KEY="aws-parent-must-not-leak"
export SSH_AUTH_SOCK="/private/tmp/fake-ssh-agent.sock"
export DEEPSEEK_API_KEY="ds-parent-must-not-leak"

PORT=$((41000 + RANDOM % 2000))
OPAQUE="conda=absent;runtime=absent;seed-assets=absent;r-libs=absent;sbx-bind-src=absent"

set +e
out="$(
  CSSWITCH_HOST_HOME="$HOST_HOME" \
  SANDBOX_HOME="$SANDBOX_HOME" \
  SCIENCE_BIN="$STUB_BIN" \
  CSSWITCH_RUNTIME_VERSION_PRECHECKED=1 \
  CSSWITCH_SCIENCE_OPAQUE_BINDINGS="$OPAQUE" \
  CSSWITCH_PROXY_URL="http://127.0.0.1:18991/deadbeefdeadbeefdeadbeefdeadbeef" \
  CSSWITCH_REUSE_SYSTEM_SSH=0 \
  CSSWITCH_SYSTEM_SSH_HOSTS= \
  PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  zsh "$SCRIPT" --port "$PORT" --skip-oauth-forge 2>&1
)"
status=$?
set -e

if [[ ! -f "$ENV_OUT" ]]; then
  echo "FAIL: stub did not write env dump (script status=$status)"
  echo "$out"
  exit 1
fi

for needle in \
  CSSWITCH_TEST_SENTINEL_SECRET \
  parent-sentinel-must-not-leak \
  OPENAI_API_KEY \
  sk-parent-must-not-leak \
  AWS_SECRET_ACCESS_KEY \
  aws-parent-must-not-leak \
  SSH_AUTH_SOCK \
  DEEPSEEK_API_KEY \
  ds-parent-must-not-leak \
  CSSWITCH_PROXY_URL \
  CSSWITCH_HOST_HOME \
  SCIENCE_BIN \
  CSSWITCH_SCIENCE_OPAQUE_BINDINGS
do
  if grep -F -q -- "$needle" "$ENV_OUT"; then
    echo "FAIL: Science child env leaked or retained forbidden '$needle'"
    cat "$ENV_OUT"
    exit 1
  fi
done

grep -F -q "HOME=$SANDBOX_HOME" "$ENV_OUT"
grep -E -q '^ANTHROPIC_BASE_URL=http://127\.0\.0\.1:18991/' "$ENV_OUT"
grep -E -q '^(HTTPS_PROXY|https_proxy)=http://127\.0\.0\.1:18991$' "$ENV_OUT"
grep -E -q '^(NO_PROXY|no_proxy)=.*127\.0\.0\.1' "$ENV_OUT"
grep -E -q '^PATH=/usr/bin:/bin:/usr/sbin:/sbin$' "$ENV_OUT"

echo "PASS: Science serve allowlist blocks ambient secrets and keeps required vars"
