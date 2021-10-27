({
  name: function () {
      return Deno.core.opSync("op_intercepted_operation_name");
  },
  proceed: function () {
    return Deno.core.opSync("op_intercepted_proceed");
  }
})