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
      field("name", $.term),
      field("body", $.model_body)
    ),
    model_body: $ => seq("{", repeat($.field), "}"),
    annotation: $ => seq(
      "@",
      field("name", $.term),
      field("params", $.param_list)
    ),
    param_list: $ => seq("(", commaSep($.expression), ")"),
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

function commaSep1(rule) {
  return seq(rule, repeat(seq(",", rule)))
}

function commaSep(rule) {
  return optional(commaSep1(rule))
}
