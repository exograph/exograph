import { init_and_resolve } from "./exograph_cf_worker.js";
import exo_ir from "../index.exo_ir";

export default {
  async fetch(request, env, ctx) {
    try {
      return await init_and_resolve(new Uint8Array(exo_ir), request, env);
    } catch (e) {
      return new Response(e);
    }
  },
};
