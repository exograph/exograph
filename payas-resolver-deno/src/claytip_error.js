class ClaytipError extends Error {
    constructor(message) {
        super(message);
        this.name = "ClaytipError";
    }
}

// Need to register the ClaytipError class so that we can use it as a custom error (see claytip_ops.rs)
try {
    // The try/catch to protect against already registered error class
    Deno.core.registerErrorClass('ClaytipError', ClaytipError);
} catch (e) {
}