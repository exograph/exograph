// config-overrides.js
module.exports = function override(config, env) {
    // determine publicPath automatically, as we will not know this due to EXO_PLAYGROUND_HTTP_PATH.
    config.output.publicPath = "auto";

    return config
}