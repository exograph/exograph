export function syncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.ops.rust_impl(value)
}

export async function asyncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.opAsync("async_rust_impl", value)
}
