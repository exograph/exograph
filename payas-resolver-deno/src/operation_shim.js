({
  name: function () {
      return Deno.core.opSync("op_intercepted_operation_name")
  },
  proceed: async function () {
      // Need to register the ClaytipError class so that we can use it as a custom error (see claytip_ops.rs)
      try {
          // The try/catch to protect against already registered error class
          Deno.core.registerErrorClass('ClaytipError', ClaytipError);
      } catch (e) {
      }
      return await Deno.core.opAsync("op_intercepted_proceed")
  },
  query: function () {
      return Deno.core.opSync("op_intercepted_operation_query")
  }
})