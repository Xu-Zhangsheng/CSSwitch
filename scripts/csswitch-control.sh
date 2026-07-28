#!/bin/zsh
# CSSwitch companion CLI. The private control token stays in a 0600 record and
# a temporary curl config; it is never printed or placed in a process argv.
set -euo pipefail
umask 077

RECORD="$HOME/.csswitch/control.json"
APP="/Applications/CSSwitch.app"

fail() { print -u2 -- "csswitch-control: $1"; exit 1; }

load_record() {
  [[ -f "$RECORD" && ! -L "$RECORD" ]] || return 1
  local mode
  mode="$(stat -f '%Lp' "$RECORD" 2>/dev/null || true)"
  [[ "$mode" == "600" ]] || fail "控制记录权限不安全"
  PORT="$(plutil -extract port raw "$RECORD" 2>/dev/null || true)"
  TOKEN="$(plutil -extract token raw "$RECORD" 2>/dev/null || true)"
  [[ "$PORT" == <-> && "$PORT" -gt 0 && "$PORT" -lt 65536 ]] || fail "控制记录端口无效"
  [[ "$TOKEN" =~ '^[[:xdigit:]]{32}$' ]] || fail "控制记录令牌无效"
}

ensure_running() {
  if load_record; then return; fi
  [[ -d "$APP" ]] || fail "未安装 CSSwitch"
  open -gj "$APP" || fail "无法打开 CSSwitch"
  local attempt
  for attempt in {1..100}; do
    if load_record; then return; fi
    sleep 0.1
  done
  fail "CSSwitch 启动后未提供控制桥；请安装带控制桥的构建"
}

call() {
  local method="$1" path="$2" payload="$3" config response
  config="$(mktemp -t csswitch-control.XXXXXX)" || fail "无法创建临时控制文件"
  trap 'rm -f "$config"' EXIT
  chmod 600 "$config"
  printf 'header = "X-CSSwitch-Control: %s"\n' "$TOKEN" > "$config"
  response="$(curl --silent --show-error --fail-with-body --max-time 50 --config "$config" -X "$method" -H 'Content-Type: application/json' --data "$payload" "http://127.0.0.1:$PORT$path")" || fail "CSSwitch 控制请求失败"
  print -r -- "$response"
}

case "${1:-}" in
  status) [[ $# -eq 1 ]] || fail '用法：csswitch-control status'; ensure_running; call GET /v1/status '' ;;
  start) [[ $# -eq 1 ]] || fail '用法：csswitch-control start'; ensure_running; call POST /v1/start '{}' ;;
  check)
    if [[ $# -eq 1 ]]; then payload='{}'
    elif [[ $# -eq 2 && "$2" == '--e2e' ]]; then payload='{"e2e":true}'
    else fail '用法：csswitch-control check [--e2e]'; fi
    ensure_running; call POST /v1/check "$payload" ;;
  *) fail '用法：csswitch-control status|start|check [--e2e]' ;;
esac
