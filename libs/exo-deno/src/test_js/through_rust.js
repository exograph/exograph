const {
    op_rust_impl,
    op_async_rust_impl,
} = Deno.core.ensureFastOps();

export function rust_impl(value) {
    return op_rust_impl(value);
}

export function async_rust_impl(value) {
    return op_async_rust_impl(value);
}
