class ClaytipError extends Error {
    constructor(message) {
        super(message);
        this.name = "ClaytipError";
    }
}

// Need to register the ClaytipError class so that we can use it as a custom error (see claytip_ops.rs)
Deno.core.registerErrorClass('ClaytipError', ClaytipError);
