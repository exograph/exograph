class ExographError extends Error {
    constructor(message) {
        super(message);
        this.name = "ExographError";
    }
}

// Need to register the ExographError class so that we can use it as a custom error (see exograph_ops.rs)
Deno[Deno.internal].core.registerErrorClass('ExographError', ExographError);
