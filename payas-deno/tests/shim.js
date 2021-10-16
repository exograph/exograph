globalThis.__shim = {
  getJson: async function(url) {
    const resp = await fetch(url);
    return await resp.json();
  },
  addAndDouble: function(i, j) {
    return (i+j) * 2;
  }
};
