// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// Set up precedence following Rust's rules (which matches JavaScript etc as well):
// https://doc.rust-lang.org/reference/expressions.html#expression-precedence

// For logical operators, `&&` has a higher precedence than `||`
// So, `a || b && c` is parsed as `a || (b && c)`
const logical_level = 1;
const logical_or_level = logical_level + 1;
const logical_and_level = logical_or_level + 1;

// Relational operators have the next highest precedence
// So, `a == b && c > d` is parsed as `(a == b) && (c > d)`
const relational_level = logical_and_level + 1;

// `!` has the highest precedence
// So, `!a || b` is parsed as `(!a) || b`
// And `!a == b` is parsed as `(!a) == b`
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
      $.module,
      $.import
    ),
    import: $ => seq(
      "import",
      field("path", $.literal_str)
    ),
    module: $ => seq(
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      "module",
      field("name", $.term),
      field("body", $.module_body)
    ),
    module_body: $ => seq(
      "{",
      repeat(field("field", $.module_field)),
      "}"
    ),
    module_field: $ => choice(
      $.type,
      $.fragment,
      $.module_method,
      $.interceptor
    ),
    module_method: $ => seq(
      optional(field("doc_comment", $.doc_comment)),
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
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      "interceptor",
      field("name", $.term),
      "(",
      optional(commaSep(field("args", $.argument))),
      ")",
    ),
    context: $ => seq(
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      "context",
      field("name", $.term),
      field("body", $.type_body)
    ),
    type: $ => seq(
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      "type",
      field("name", $.term),
      field("body", $.type_body)
    ),
    fragment: $ => seq(
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      "fragment",
      field("name", $.term),
      field("body", $.fragment_body)
    ),
    type_body: $ => seq("{", repeat(choice(field("field", $.field), field("fragment_reference", $.fragment_reference))), "}"),
    fragment_body: $ => seq("{", repeat(field("field", $.field)), "}"),
    fragment_reference: $ => seq("...", field("name", $.term)),
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
      optional(field("doc_comment", $.doc_comment)),
      repeat(field("annotation", $.annotation)),
      field("name", $.term),
      ":",
      field("field_type", $.field_type),
      optional(seq(
        "=",
        field("default_value", $.field_default_value)
      )),
      optional(";")
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
    field_term: $ => seq(optional(seq(field("module", $.term), ".")), field("name", $.term)),
    field_type: $ => choice(
      $.optional_field_type,
      seq($.field_term, optional(seq("<", commaSep(field("type_param", $.field_type)), ">")))
    ),
    optional_field_type: $ => seq(field("inner", $.field_type), "?"),
    literal: $ => choice(
      $.literal_number,
      $.literal_str,
      $.literal_boolean,
      $.literal_null
    ),
    expression: $ => choice(
      $.parenthetical,
      prec(logical_level, $.logical_op),
      prec(relational_level, $.relational_op),
      $.selection,
      $.literal,
    ),
    parenthetical: $ => seq("(", field("expression", $.expression), ")"),
    selection: $ => choice(
      $.selection_select,
      $.term
    ),
    selection_select: $ => seq(
      field("prefix", $.selection),
      ".",
      field("selection_element", $.selection_element)
    ),
    selection_element: $ => choice(
      $.term,
      $.func_call,
    ),
    func_call: $ => seq(
      field("name", $.term), // "contains"
      "(",
      optional(commaSep(field("args", $.expression))), // "ADMIN"
      ")"
    ),
    // High-order function call of the `name(param_name, expr)` such as: 
    // - `some((du) => du.userId == AuthContext.id && du.read)`
    // - `some(du => du.userId == AuthContext.id && du.read)`
    // (note `du` vs `(du)`)
    func_call: $ => seq(
      field("name", $.term), // "some"
      "(",
      choice(
        field("hof_args", $.hof_args),
        field("normal_param", optional(commaSep($.literal))) // "ADMIN"
      ),
      ")"
    ),
    hof_args: $ => seq(
      choice(
        field("param_name", $.term), // "du"
        seq("(", field("param_name", $.term), ")") // "(du)"
      ),
      "=>",
      field("expr", $.expression) // "du.userId == AuthContext.id && du.read"
    ),
    logical_op: $ => choice(
      $.logical_or,
      $.logical_and,
      $.logical_not
    ),
    logical_or: $ => prec.left(logical_or_level, seq(
      field("left", $.expression), "||", field("right", $.expression)
    )),
    logical_and: $ => prec.left(logical_and_level, seq(
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
    literal_null: $ => "null",
    literal: $ => choice(
      $.literal_str,
      $.literal_number,
      $.literal_boolean,
      $.literal_null
    ),
    comment: $ => choice(
      seq('//', /.*/),
      seq(
        '/*',
        /[^*]*\*+([^\/*][^*]*\*+)*/,
        '/'
      )
    ),
    doc_comment: $ => choice(
      seq(
        /\/\*\*[ \t]*\n/,
        repeat1(seq('*', field("doc_line", $.doc_line_content))),
        /\*\//
      ),
      repeat1(seq('///', field("doc_line", $.doc_line_content)))
    ),

    doc_line_content: $ => /.*/,
  }
});

function commaSep(rule) {
  return seq(rule, repeat(seq(",", rule)))
}
