# Needs the wasm32-wasip2 target (rustup target add wasm32-wasip2) and protoc.
.PHONY: build test clean

# function-wasm serves modules from demo/plugins/.
out := ../../plugins/pdb.wasm

build: ## Compile the plugin to demo/plugins/pdb.wasm
	cargo build --release --target wasm32-wasip2
	cp target/wasm32-wasip2/release/pdb.wasm $(out)

test: ## Run the unit tests natively
	cargo test

clean:
	cargo clean
	rm -f $(out)
