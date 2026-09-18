//! The function-wasm ABI v2 world (wit/wasmfn-function.wit): the async `run`
//! export and the host's `log` import. wasm-only, so the crate also builds
//! and tests natively.

wit_bindgen::generate!({
    path: "wit",
    world: "function",
    generate_all,
});

struct Plugin;

impl Guest for Plugin {
    async fn run(request: Vec<u8>) -> Result<Vec<u8>, String> {
        Ok(crate::handle(&request))
    }
}

export!(Plugin);
