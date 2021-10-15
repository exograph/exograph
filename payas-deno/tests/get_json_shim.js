globalThis.__get_json_shim = {
  async_execute: async function(url) {
    const resp = await fetch(url);
    return await resp.json();
  },
  sync_execute: function(value) {
    return `value: ${value}`;
  }
};
