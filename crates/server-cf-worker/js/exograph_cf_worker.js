import * as imports from "./exograph_cf_worker_bg.js";
import wkmod from "./exograph_cf_worker_bg.wasm";

const instance = new WebAssembly.Instance(wkmod, { "./exograph_cf_worker_bg.js": imports });
imports.__wbg_set_wasm(instance.exports);

export * from "./exograph_cf_worker_bg.js";