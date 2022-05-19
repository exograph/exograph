class ClaytipError extends Error {
    constructor(message) {
        super(message);
        this.name = "ClaytipError";
    }
}