export function addAndDoubleThroughShim(i, j, shim) {
  return shim.addAndDouble(i, j);
}

export async function getJsonThroughShim(id, shim) {
  return await shim.getJson("https://jsonplaceholder.typicode.com/todos/" + id);
}
