use nom::{
    branch::{alt, permutation},
    bytes::complete::tag,
    character::complete::{char, newline},
    combinator::opt,
    multi::{many0, many1, separated_list1},
    sequence::{pair, separated_pair, tuple},
};
use nom::{combinator::map, sequence::delimited};

use crate::ast::ast_types::{
    AstField, AstFieldType, AstRelation, AstSystem, AstType, AstTypeKind, AstTypeModifier,
};

use super::{util::*, PResult};

#[derive(Debug, Clone, PartialEq)]
struct TypeUsage {
    type_name: String,
    array: bool,
    optional: bool,
}

fn table<'a>(input: &'a str) -> PResult<&'a str, Identifier<'a>> {
    delimited(tag("@table("), spaces(quoted_identifier), char(')'))(input)
}

// Could abstract the annotation to share with table
fn column<'a>(input: &'a str) -> PResult<&'a str, Identifier<'a>> {
    delimited(tag("@column("), spaces(quoted_identifier), char(')'))(input)
}

fn pk<'a>(input: &'a str) -> PResult<&'a str, bool> {
    map(tag("@pk"), |_| true)(input)
}

fn autoincrement<'a>(input: &'a str) -> PResult<&'a str, bool> {
    map(tag("@autoincrement"), |_| true)(input)
}

pub fn system<'a>(input: &'a str) -> PResult<&'a str, AstSystem> {
    map(
        tuple((
            many0(newline),
            separated_list1(many1(newline), spaces(model)),
            many0(spaces(newline)),
        )),
        |(_, models, _)| AstSystem { types: models },
    )(input)
}

fn model<'a>(input: &'a str) -> PResult<&'a str, AstType> {
    map(
        tuple((
            spaces(tag("model")),
            spaces(identifier),
            opt(table),
            ws(char('{')),
            fields,
            many0(newline),
            spaces(char('}')),
        )),
        |(_, name, table, _, fields, _, _)| {
            let kind = AstTypeKind::Composite {
                fields,
                table_name: table.map(|table| table.0.to_string()),
            };

            AstType {
                name: name.0.to_string(),
                kind,
            }
        },
    )(input)
}

fn fields<'a>(input: &'a str) -> PResult<&'a str, Vec<AstField>> {
    separated_list1(many1(newline), spaces(field))(input)
}

fn field<'a>(input: &'a str) -> PResult<&'a str, AstField> {
    map(
        spaces(tuple((
            spaces(separated_pair(identifier, spaces(char(':')), type_usage)),
            column_attributes,
        ))),
        |((name, type_usage), column_attributes)| {
            let typ = if type_usage.type_name.as_str() == "Int" {
                AstFieldType::Int {
                    autoincrement: column_attributes.autoincrement,
                }
            } else {
                AstFieldType::Other {
                    name: type_usage.type_name.clone(),
                }
            };

            let type_modifier = if type_usage.array {
                AstTypeModifier::List
            } else {
                AstTypeModifier::NonNull
            };

            let relation = if column_attributes.pk {
                AstRelation::Pk
            } else {
                AstRelation::Other {
                    optional: type_usage.optional,
                }
            };

            AstField {
                name: name.0.to_string(),
                typ,
                type_modifier,
                relation,
                column_name: column_attributes.name.map(|name| name.0.to_string()),
            }
        },
    )(input)
}

#[derive(Debug, Clone, PartialEq)]
struct ColumnAttributes<'a> {
    name: Option<Identifier<'a>>,
    pk: bool,
    autoincrement: bool,
}

fn column_attributes<'a>(input: &'a str) -> PResult<&'a str, ColumnAttributes<'a>> {
    map(
        permutation((
            opt(spaces(column)),
            opt(spaces(pk)),
            opt(spaces(autoincrement)),
        )),
        |(name, pk, autoincrement)| ColumnAttributes {
            name,
            pk: pk.is_some(),
            autoincrement: autoincrement.is_some(),
        },
    )(input)
}

fn type_usage<'a>(input: &'a str) -> PResult<&'a str, TypeUsage> {
    fn array_usage<'a>(input: &'a str) -> PResult<&'a str, Identifier> {
        delimited(char('['), spaces(identifier), char(']'))(input)
    }

    fn base_type_usage<'a>(input: &'a str) -> PResult<&'a str, (Identifier, bool)> {
        alt((
            map(array_usage, |ident| (ident, true)),
            map(identifier, |ident| (ident, false)),
        ))(input)
    }

    map(
        pair(base_type_usage, opt(char('?'))),
        |((ident, array), optional)| TypeUsage {
            type_name: ident.0.to_string(),
            array,
            optional: optional.is_some(),
        },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table() {
        assert_eq!(table(r#"@table("orders")"#), Ok(("", Identifier("orders"))));
        assert_eq!(
            table(r#"@table( "orders" )"#),
            Ok(("", Identifier("orders")))
        );
    }

    #[test]
    fn parse_type_usage() {
        assert_eq!(
            type_usage("Person"),
            Ok((
                "",
                TypeUsage {
                    type_name: "Person".to_string(),
                    array: false,
                    optional: false
                }
            ))
        );

        assert_eq!(
            type_usage("Person?"),
            Ok((
                "",
                TypeUsage {
                    type_name: "Person".to_string(),
                    array: false,
                    optional: true
                }
            ))
        );

        assert_eq!(
            type_usage("[Person]"),
            Ok((
                "",
                TypeUsage {
                    type_name: "Person".to_string(),
                    array: true,
                    optional: false
                }
            ))
        );

        assert_eq!(
            type_usage("[Person]?"),
            Ok((
                "",
                TypeUsage {
                    type_name: "Person".to_string(),
                    array: true,
                    optional: true
                }
            ))
        );
    }

    #[test]
    fn parse_column_attributes() {
        assert_eq!(
            column_attributes(r#"@column("col")"#),
            Ok((
                "",
                ColumnAttributes {
                    name: Some(Identifier("col")),
                    pk: false,
                    autoincrement: false
                }
            ))
        );

        assert_eq!(
            column_attributes(r#"@column("col") @pk @autoincrement"#),
            Ok((
                "",
                ColumnAttributes {
                    name: Some(Identifier("col")),
                    pk: true,
                    autoincrement: true
                }
            ))
        );

        // TODO: Check why permutation combinator didn't help here
        // assert_eq!(
        //     column_attributes(r#"@autoincrement @column("col") @pk"#),
        //     Ok((
        //         "",
        //         ColumnAttributes {
        //             name: Some(Identifier("col")),
        //             pk: true,
        //             autoincrement: true
        //         }
        //     ))
        // )
    }

    #[test]
    fn parse_field() {
        assert_eq!(
            field(r#"id: Int @column("ident") @pk @autoincrement"#),
            Ok((
                "",
                AstField {
                    name: "id".to_string(),
                    typ: AstFieldType::Int {
                        autoincrement: true
                    },
                    type_modifier: AstTypeModifier::NonNull,
                    relation: AstRelation::Pk,
                    column_name: Some("ident".to_string())
                }
            ))
        );

        assert_eq!(
            field(r#"teams: [Team]? @column("team_id")"#),
            Ok((
                "",
                AstField {
                    name: "teams".to_string(),
                    typ: AstFieldType::Other {
                        name: "Team".to_string(),
                    },
                    type_modifier: AstTypeModifier::List,
                    relation: AstRelation::Other { optional: true },
                    column_name: Some("team_id".to_string())
                }
            ))
        )
    }

    #[test]
    fn parse_simple_type() {
        // The extra newline between the fields ensure that the parser is robust against such format
        assert_eq!(
            model(
                r#"model Venue {
                name: String?

                address: String
        }"#,
            ),
            Ok(("", venue_type()))
        )
    }

    #[test]
    fn parse_type_with_table_name() {
        assert_eq!(
            model(
                r#"model Person @table("people") {
                first_name: String? @column("f_name")
                age: Int
        }"#
            ),
            Ok(("", person_type()))
        )
    }

    #[test]
    fn parse_simple_system() {
        assert_eq!(
            system(
                r#"
        model Venue {
                name: String?
                address: String        
        }
        model Person @table("people") {
                first_name: String? @column("f_name")
                age: Int        
        }
        "#,
            ),
            Ok((
                "",
                AstSystem {
                    types: vec![venue_type(), person_type()]
                }
            ))
        )
    }

    fn person_type() -> AstType {
        AstType {
            name: "Person".to_string(),
            kind: AstTypeKind::Composite {
                fields: vec![
                    AstField {
                        name: "first_name".to_string(),
                        typ: AstFieldType::Other {
                            name: "String".to_string(),
                        },
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: true },
                        column_name: Some("f_name".to_string()),
                    },
                    AstField {
                        name: "age".to_string(),
                        typ: AstFieldType::Int {
                            autoincrement: false,
                        },
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: false },
                        column_name: None,
                    },
                ],
                table_name: Some("people".to_string()),
            },
        }
    }

    fn venue_type() -> AstType {
        AstType {
            name: "Venue".to_string(),
            kind: AstTypeKind::Composite {
                fields: vec![
                    AstField {
                        name: "name".to_string(),
                        typ: AstFieldType::Other {
                            name: "String".to_string(),
                        },
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: true },
                        column_name: None,
                    },
                    AstField {
                        name: "address".to_string(),
                        typ: AstFieldType::Other {
                            name: "String".to_string(),
                        },
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: false },
                        column_name: None,
                    },
                ],
                table_name: None,
            },
        }
    }
}
