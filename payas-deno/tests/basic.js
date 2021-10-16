export function addAndDouble(i, j) {
  return (i+j) * 2;
}

  // TODO: avoid fetching from a server in tests
export async function getJson(id) {
  const r = await fetch("https://jsonplaceholder.typicode.com/todos/" + id);
  return await r.json();
}


