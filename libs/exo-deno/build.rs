fn main() {
    use deno_runtime::ops::bootstrap::SnapshotOptions;
    use std::path::PathBuf;

    let snapshot_options = SnapshotOptions {
        ts_version: deno::version::DENO_VERSION_INFO.typescript.to_string(),
        v8_version: deno_core::v8::VERSION_STRING,
        target: std::env::var("TARGET").unwrap(),
    };

    let o = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

    deno_runtime::snapshot::create_runtime_snapshot(snapshot_path, snapshot_options, vec![]);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TARGET");
}
