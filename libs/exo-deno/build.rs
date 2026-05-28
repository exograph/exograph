// Stub build script with no work of its own. Exists so that cargo activates
// this crate's `[target.'cfg(windows)'.build-dependencies]`, which in turn
// forces `winapi/std` into the host-tree feature graph. Without that,
// `deno_snapshots`'s build script (which transitively depends on `deno_io`)
// compiles a `winapi` whose `ctypes::c_void` is a distinct type from
// `core::ffi::c_void`, breaking the Windows build.
fn main() {}
