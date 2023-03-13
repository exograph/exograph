export function syncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.ops.rust_impl(value)
}

export async function asyncUsingRegisteredFunction(value) {
  return await Deno[Deno.internal].core.ops.async_rust_impl(value)
}
