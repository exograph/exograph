
export function syncUsingRegisteredFunction(value) {
  return Deno[Deno.internal].core.ops.rust_impl(value)
}