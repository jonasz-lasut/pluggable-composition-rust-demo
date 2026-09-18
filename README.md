# pluggable-composition-rust-demo

A Crossplane project that composes a `WebApp` (`platform.example.com/v1alpha1`)
into a Deployment and a Service, and lets each `WebApp` name a WebAssembly
plugin that extends the result. The composition pipeline has three steps:

1. `compose-webapp`: a Rust composition function, in `functions/compose-webapp`.
2. `extend-webapp`: [function-wasm](https://github.com/jonasz-lasut/function-wasm)
   runs the module named in `spec.extensionName`, and does nothing when the
   field is empty.
3. `function-auto-ready`.

The `pdb` plugin in `demo/plugins-source/pdb` adds a PodDisruptionBudget for
the composed Deployment.

## Run it locally

The demo renders a `WebApp` with `crossplane composition render` against
functions running on your machine. It needs no cluster.

### Prerequisites

- Rust (stable) with the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `protoc`, to build the plugin
- The `crossplane` CLI from [crossplane/cli](https://github.com/crossplane/cli),
  which has `crossplane composition render`
- Docker, running: render uses it for the Crossplane render engine and for
  `function-auto-ready`
- `yq`. `jq` is optional and makes the function-wasm logs readable.

### 1. Build the function-wasm runtime

```sh
git clone https://github.com/jonasz-lasut/function-wasm ~/code/function-wasm
cd ~/code/function-wasm
cargo build --release -p function-wasm
```

`demo/function-wasm.sh` looks for the binary at
`~/code/function-wasm/target/release/function`. Set `FUNCTION_WASM_BIN` to use
a binary from another path.

### 2. Build the plugin

```sh
make -C demo/plugins-source/pdb build
```

This writes `demo/plugins/pdb.wasm`, the directory function-wasm serves modules
from.

### 3. Start function-wasm

```sh
demo/function-wasm.sh
```

It runs in the foreground on `:9444` and applies the Cedar policy in
`demo/policies/default.cedar`, which grants plugins nothing. Set `POLICY` to
use another policy file. Ctrl-C stops it.

### 4. Render an example

In a second terminal:

```sh
demo/render.sh simple   # Deployment and Service
demo/render.sh pdb      # the same, plus a PodDisruptionBudget from pdb.wasm
```

The argument is the name of a file in `examples/webapps/`. The script builds
`compose-webapp` when its binary is missing, starts it on `:9443`, renders, and
stops it again. After editing the function, rebuild it with
`BUILD=1 demo/render.sh simple`.

Arguments after the example name go to `crossplane composition render`. For
example, `demo/render.sh pdb -x` prints the full XR.
