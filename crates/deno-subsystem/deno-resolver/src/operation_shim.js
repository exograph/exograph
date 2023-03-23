({
  name: function () {
      return Deno[Deno.internal].core.ops.op_operation_name()
  },
  proceed: async function () {
      return await Deno[Deno.internal].core.opAsync("op_operation_proceed")
  },
  query: function () {
      return Deno[Deno.internal].core.ops.op_operation_query()
  }
})