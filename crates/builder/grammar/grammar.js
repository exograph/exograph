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
      $.context,
      $.service,
      $.import
    ),
    import: $ => seq(
      "import",
      field("path", $.literal_str)
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
      $.type,
      $.service_method,
      $.interceptor
    ),
    service_method: $ => seq(
      repeat(field("annotation", $.annotation)),
      optional(field("is_exported", "export")),
      field("method_type", choice("query", "mutation")),
      field("name", $.term),
      "(",
      optional(commaSep(field("args", $.argument))),
      "):",
      field("return_type", $.field_type)
    ),
    interceptor: $ => seq(
      repeat(field("annotation", $.annotation)),
      "interceptor",
      field("name", $.term),
      "(",
      optional(commaSep(field("args", $.argument))),
      ")",
    ),
    context: $ => seq(
      repeat(field("annotation", $.annotation)),
      "context",
      field("name", $.term),
      field("body", $.type_body)
    ),
    type: $ => seq(
      repeat(field("annotation", $.annotation)),
      "type",
      field("name", $.term),
      field("body", $.type_body)
    ),
    type_body: $ => seq("{", repeat(field("field", $.field)), "}"),
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
      $.annotation_multiple_params,
      $.annotation_map_params
    ),
    annotation_multiple_params: $ => commaSep(field("exprs", $.expression)),
    annotation_map_params: $ => commaSep(field("param", $.annotation_map_param)),
    annotation_map_param: $ => seq(field("name", $.term), "=", field("expr", $.expression)),
    argument: $ => seq(
      repeat(field("annotation", $.annotation)),
      field("name", $.term),
      ":",
      field("argument_type", $.field_type),
    ),
    field: $ => seq(
      field("name", $.term),
      ":",
      field("field_type", $.field_type),
      optional(seq(
        "=",
        field("default_value", $.field_default_value) 
      )),
      repeat(field("annotation", $.annotation))
    ),
    field_default_value: $ => choice(
      field("default_value_concrete", $.expression),
      seq(
        field("default_value_fn", $.term), 
        "(", 
        optional(commaSep(field("default_value_fn_args", $.expression))),
        ")"
      )
    ),
    field_type: $ => choice(
      $.optional_field_type,
      seq($.term, optional(seq("<", commaSep(field("type_param", $.field_type)), ">")))
    ),
    optional_field_type: $ => seq(field("inner", $.field_type), "?"),
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
      $.relational_gte,
      $.relational_in,
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
    relational_in: $ => prec.left(relational_level, seq(
      field("left", $.expression), "in", field("right", $.expression)
    )),
    term: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
    str: $ => /(?:[^"\\]|\\.)*/, // string with escaped quotes
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
