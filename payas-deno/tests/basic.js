export function addAndDouble(i, j) {
  return (i+j) * 2;
}

  // TODO: avoid fetching from a server in tests
export async function getJson(id) {
  const r = await fetch("https://jsonplaceholder.typicode.com/todos/" + id);
  return await r.json();
}

export async function asyncUsingShim(id, get_json_shim) {
  return await get_json_shim.async_execute("https://jsonplaceholder.typicode.com/todos/" + id);
}

export function syncUsingShim(param, get_json_shim) {
  return get_json_shim.sync_execute(param);
}

export function syncUsingRegisteredFunction(value) {
  return Deno.core.opSync("rust_impl", [value])
}