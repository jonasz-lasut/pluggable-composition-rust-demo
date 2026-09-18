#!/usr/bin/env bash
# Runs the function-wasm runtime in the foreground for demo/render.sh. It
# serves the wasm plugins in demo/plugins/, compiles each of them before the
# first request (--warm-modules), and applies the operator's Cedar policy.
# Its JSON log lines are printed readable through jq; Ctrl-C stops it.
#
# Usage: demo/function-wasm.sh [function-wasm flags...]
#
#   POLICY              Cedar grant policy (default demo/policies/default.cedar)
#   FUNCTION_WASM_BIN   runtime binary
#                       (default ~/code/function-wasm/target/release/function)
#
# Metrics: http://localhost:8080/metrics, /livez and /readyz on :8081.
set -euo pipefail

demo=$(cd "$(dirname "$0")" && pwd)
bin=${FUNCTION_WASM_BIN:-$HOME/code/function-wasm/target/release/function}
policy=${POLICY:-$demo/policies/default.cedar}

# Must match the Development target in manifests/functions.yaml.
port=9444

[[ -x "$bin" ]] || {
  echo "$bin not found; build it in a function-wasm checkout:" >&2
  echo "  cargo build --release -p function-wasm" >&2
  exit 1
}
[[ -f "$policy" ]] || { echo "$policy not found" >&2; exit 1; }

cmd=("$bin" --insecure --address "0.0.0.0:$port"
  --module-dir "$demo/plugins" --sandbox-policy-file "$policy")
shopt -s nullglob
for module in "$demo"/plugins/*.wasm; do
  cmd+=(--warm-modules "path:$(basename "$module")")
done
cmd+=("$@")

echo "==> ${cmd[*]}" >&2

command -v jq >/dev/null || exec "${cmd[@]}"

if [[ -t 1 ]]; then color=true; else color=false; fi

# One line per event: time, level, target, message, then the other fields.
# A line that is not JSON (--debug output, a panic) is printed as it is.
# shellcheck disable=SC2016 # $-names below are jq variables
format='
  def paint($code): if $color then "[\($code)m\(.)[0m" else . end;
  . as $line
  | try (
      fromjson
      | (.level | {ERROR: "31", WARN: "33", INFO: "32", DEBUG: "34"}[.] // "0") as $code
      | [ (.timestamp[11:19] | paint("2")),
          (.level | (. + "     ")[0:5] | paint($code)),
          ((.target // "") | paint("2")),
          (.fields.message // ""),
          (.fields | del(.message) | to_entries
            | map("\(.key | paint("36"))=\(.value | if type == "string" then . else tojson end)")
            | join(" ")) ]
      | map(select(. != "")) | join(" ")
    ) catch $line'

# jq ignores Ctrl-C so it still prints what the runtime logs while shutting
# down; it ends when the runtime closes the pipe.
"${cmd[@]}" 2>&1 | (trap '' INT; exec jq -R -r --unbuffered --argjson color "$color" "$format")
