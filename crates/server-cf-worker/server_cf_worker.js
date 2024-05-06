import * as imports from "./server_cf_worker_bg.js";
import wkmod from "./server_cf_worker_bg.wasm";

const instance = new WebAssembly.Instance(wkmod, { "./server_cf_worker_bg.js": imports });
imports.__wbg_set_wasm(instance.exports);

export * from "./server_cf_worker_bg.js";