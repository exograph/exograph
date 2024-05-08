(cd crates/server-cf-worker && wasm-pack build --target bundler --out-name exograph_cf_worker)

mkdir -p target/cf-worker-dist
cp crates/server-cf-worker/pkg/*.wasm target/cf-worker-dist
cp crates/server-cf-worker/pkg/*.js target/cf-worker-dist
cp crates/server-cf-worker/js/exograph_cf_worker.js target/cf-worker-dist
cp crates/server-cf-worker/js/index.js target/cf-worker-dist
cp LICENSE target/cf-worker-dist
(cd target/cf-worker-dist/ && zip ../exograph-cf-worker-wasm.zip *)
  