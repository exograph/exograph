fn main() {
    use deno_runtime::ops::bootstrap::SnapshotOptions;
    use std::path::PathBuf;

    let snapshot_options = SnapshotOptions {
        deno_version: deno::version::deno().to_string(),
        ts_version: deno::version::TYPESCRIPT.to_string(),
        v8_version: deno_core::v8_version(),
        target: std::env::var("TARGET").unwrap(),
    };

    let o = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

    deno_runtime::snapshot::create_runtime_snapshot(snapshot_path, snapshot_options);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TARGET");
}
