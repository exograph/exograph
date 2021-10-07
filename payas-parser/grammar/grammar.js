const logical_level = 1;
const relational_level = logical_level + 1;
const not_level = relational_level + 1;

module.exports = grammar({
  name: 'grammar',

  extras: $ => [
    /\s/,
    $.comment
  ],

  rules: {
    source_file: $ => repeat($.declaration),
    declaration: $ => choice(
      $.model,
      $.service
    ),
    service: $ => seq(
      repeat(field("annotation", $.annotation)),
      "service",
      field("name", $.term),
      field("body", $.service_body)
    ),
    service_body: $ => seq(
      "{", 
      repeat(field("field", $.service_field)), 
      "}"
    ),
    service_field: $ => choice(
      $.model,
      $.service_method
    ),
    service_method: $ => seq(
      repeat(field("annotation", $.annotation)),
      optional(field("is_exported", "export")),
      field("type", choice("query", "mutation")),
      field("name", $.term),
      "(",
      commaSep(field("args", $.argument)),
      "):",
      field("return_type", $.type)
    ),
    model: $ => seq(
      repeat(field("annotation", $.annotation)),
      field("kind", $.model_kind),
      field("name", $.term),
      field("body", $.model_body)
    ),
    model_kind: $ => choice("model", "context", "type"),
    model_body: $ => seq("{", repeat(field("field", $.field)), "}"),
    annotation: $ => seq(
      "@",
      field("name", $.term),
      optional(seq(
        "(",
        field("params", $.annotation_params),
        ")"
      ))
    ),
    annotation_params: $ => choice(
      $.expression,
      $.annotation_map_params
    ),
    annotation_map_params: $ => commaSep(field("param", $.annotation_map_param)),
    annotation_map_param: $ => seq(field("name", $.term), "=", field("expr", $.expression)),
    argument: $ => seq(
      repeat(field("annotation", $.annotation)),
      field("name", $.term),
      ":",
      field("type", $.type),
    ),
    field: $ => seq(
      field("name", $.term),
      ":",
      field("type", $.type),
      repeat(field("annotation", $.annotation))
    ),
    type: $ => choice(
      $.optional_type,
      seq($.term, optional(seq("[", commaSep(field("type_param", $.type)), "]")))
    ),
    array_type: $ => seq("[", field("inner", $.type), "]"),
    optional_type: $ => seq(field("inner", $.type), "?"),
    expression: $ => choice(
      $.parenthetical,
      prec(1, $.logical_op),
      prec(3, $.relational_op),
      $.selection,
      $.literal_number,
      $.literal_str,
      $.literal_boolean
    ),
    parenthetical: $ => seq("(", $.expression, ")"),
    selection: $ => choice(
      $.selection_select,
      $.term
    ),
    selection_select: $ => seq(
      field("prefix", $.selection),
      ".",
      field("term", $.term)
    ),
    logical_op: $ => choice(
      $.logical_or,
      $.logical_and,
      $.logical_not
    ),
    logical_or: $ => prec.left(logical_level, seq(
      field("left", $.expression), "||", field("right", $.expression)
    )),
    logical_and: $ => prec.left(logical_level, seq(
      field("left", $.expression), "&&", field("right", $.expression)
    )),
    logical_not: $ => prec(not_level, seq(
      "!", field("value", $.expression)
    )),
    relational_op: $ => choice(
      $.relational_eq,
      $.relational_neq,
      $.relational_lt,
      $.relational_lte,
      $.relational_gt,
      $.relational_gte
    ),
    relational_eq: $ => prec.left(relational_level, seq(
      field("left", $.expression), "==", field("right", $.expression)
    )),
    relational_neq: $ => prec.left(relational_level, seq(
      field("left", $.expression), "!=", field("right", $.expression)
    )),
    relational_lt: $ => prec.left(relational_level, seq(
      field("left", $.expression), "<", field("right", $.expression)
    )),
    relational_lte: $ => prec.left(relational_level, seq(
      field("left", $.expression), "<=", field("right", $.expression)
    )),
    relational_gt: $ => prec.left(relational_level, seq(
      field("left", $.expression), ">", field("right", $.expression)
    )),
    relational_gte: $ => prec.left(relational_level, seq(
      field("left", $.expression), ">=", field("right", $.expression)
    )),
    term: $ => /[a-zA-Z_]+/,
    str: $ => /[a-zA-Z_\.]+/, // TODO: need to do proper string parsing
    number: $ => /\d+/,
    literal_str: $ => seq("\"", field("value", $.str), "\""),
    literal_boolean: $ => choice("true", "false"),
    literal_number: $ => field("value", $.number),
    comment: $ => token(choice(
      seq('//', /.*/),
      seq(
        '/*',
        /[^*]*\*+([^/*][^*]*\*+)*/,
        '/'
      )
    ))
  }
});

function commaSep(rule) {
  return seq(rule, repeat(seq(",", rule)))
}
