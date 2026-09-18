#!/usr/bin/env bash
# Renders examples/webapps/<example>.yaml against functions running on this
# machine. Project-mode render recompiles compose-webapp on every run; this
# starts the already built binary, renders, and stops it again.
#
# function-wasm is not started here: run demo/function-wasm.sh in another
# terminal first, where its logs stay visible.
#
# Usage: demo/render.sh <example> [crossplane composition render flags...]
#
#   BUILD=1   rebuild compose-webapp first (otherwise only if missing)
#
# A compose-webapp that already listens on its port is used as it is and left
# running.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)

# Must match the Development targets in manifests/functions.yaml.
webapp_port=9443
wasm_port=9444

usage="usage: demo/render.sh <example in examples/webapps/> [render flags...]"
example=${1:?$usage}
shift
xr="$root/examples/webapps/${example%.yaml}.yaml"
[[ -f "$xr" ]] || {
  echo "$xr not found; examples:" >&2
  for f in "$root"/examples/webapps/*.yaml; do echo "  $(basename "$f" .yaml)" >&2; done
  exit 1
}

webapp_dir="$root/functions/compose-webapp"
webapp_bin="$webapp_dir/target/release/function"

listening() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

command -v crossplane >/dev/null || { echo "crossplane CLI not found" >&2; exit 1; }
listening "$wasm_port" || {
  echo "function-wasm does not listen on :$wasm_port; start it in another terminal:" >&2
  echo "  demo/function-wasm.sh" >&2
  exit 1
}

if [[ -n "${BUILD:-}" || ! -x "$webapp_bin" ]]; then
  echo "==> building compose-webapp" >&2
  (cd "$webapp_dir" && cargo build --release >&2)
fi

log=$(mktemp)
webapp_pid=
trap '[[ -z "$webapp_pid" ]] || kill "$webapp_pid" 2>/dev/null || true; rm -f "$log"' EXIT

if listening "$webapp_port"; then
  echo "==> compose-webapp already listens on :$webapp_port, using it" >&2
else
  echo "==> starting compose-webapp on :$webapp_port" >&2
  "$webapp_bin" --insecure --address "0.0.0.0:$webapp_port" --metrics-address "" >"$log" 2>&1 &
  webapp_pid=$!
  for _ in $(seq 1 40); do
    listening "$webapp_port" && break
    sleep 0.25
  done
  listening "$webapp_port" || { echo "compose-webapp did not start:" >&2; cat "$log" >&2; exit 1; }
fi

echo "==> crossplane composition render $example" >&2
cd "$root"
status=0
crossplane composition render "$xr" apis/webapps/composition.yaml \
  demo/manifests/functions.yaml "$@" | yq . || status=$?

if [[ $status -ne 0 && -s "$log" ]]; then
  echo "--- compose-webapp log ---" >&2
  cat "$log" >&2
fi
exit $status
