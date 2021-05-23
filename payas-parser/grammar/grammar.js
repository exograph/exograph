module.exports = grammar({
  name: 'grammar',

  rules: {
    // TODO: add the actual grammar rules
    source_file: $ => repeat($.declaration),
    declaration: $ => choice(
      $.model
    ),
    model: $ => seq(
      repeat($.annotation),
      "model",
      $.term,
      "{",
      repeat($.field),
      "}"
    ),
    annotation: $ => seq(
      "@",
      $.term,
      "(",
      repeat($.expression),
      ")"
    ),
    field: $ => seq(
      $.term,
      ":",
      $.term,
      repeat($.annotation)
    ),
    expression: $ => choice(
      $.selection,
      $.logical_or
    ),
    selection: $ => choice(
      seq(
        $.selection,
        ".",
        $.term
      ),
      $.term
    ),
    logical_or: $ => prec.left(1, seq(
      $.expression, "|", $.expression
    )),
    number: $ => /\d+/,
    term: $ => /[a-zA-Z]+/,
  }
});
