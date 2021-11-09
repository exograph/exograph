
export function syncUsingRegisteredFunction(value) {
  return Deno.core.opSync("rust_impl", [value])
}