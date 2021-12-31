({
  name: function () {
      return Deno.core.opSync("op_intercepted_operation_name")
  },
  proceed: async function () {
      return await Deno.core.opAsync("op_intercepted_proceed")
  }
})