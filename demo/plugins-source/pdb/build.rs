// Generates the RunFunction messages from the vendored crossplane proto.
// prost-build shells out to protoc; set PROTOC if it is not on PATH.
fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=proto/run_function.proto");
    prost_build::compile_protos(&["proto/run_function.proto"], &["proto"])
}
