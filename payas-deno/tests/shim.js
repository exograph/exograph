export async function asyncUsingShim(id, get_json_shim) {
  return await get_json_shim.async_execute("https://jsonplaceholder.typicode.com/todos/" + id);
}

export function syncUsingShim(param, get_json_shim) {
  return get_json_shim.sync_execute(param);
}