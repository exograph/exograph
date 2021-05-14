mod util;

use std::{fs, path::Path};

use crate::{ast::ast_types::*, parser::util::*};
use pest::{error::Error, iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "parser/payas.pest"]
struct PayasParser;

pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<AstSystem, Error<Rule>> {
    let file_content = fs::read_to_string(path).unwrap();
    parse(&file_content)
}

pub fn parse(input: &str) -> Result<AstSystem, Error<Rule>> {
    let parsed = PayasParser::parse(Rule::system_document, input)?;

    let iter = parsed.into_iter().next().unwrap();
    parse_system_document(iter)
}

fn parse_system_document(pair: Pair<Rule>) -> Result<AstSystem, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::system_document);

    let models: Result<Vec<_>, _> = pair
        .into_inner()
        .filter_map(|pair| {
            if pair.as_rule() == Rule::model_definition {
                Some(parse_model_definition(pair))
            } else {
                None
            }
        })
        .collect();

    models.map(|models| AstSystem { types: models })
}

fn parse_model_definition(pair: Pair<Rule>) -> Result<AstType, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::model_definition);

    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let table_name = parse_if_rule(&mut inner, Rule::table, |pair| {
        Ok(pair.into_inner().next().unwrap().as_str().to_string())
    })
    .unwrap();

    parse_fields_definition(inner.next().unwrap()).map(|fields| AstType {
        name,
        kind: AstTypeKind::Composite { fields, table_name },
    })
}

fn parse_fields_definition(pair: Pair<Rule>) -> Result<Vec<AstField>, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::fields_definition);

    pair.into_inner().map(parse_field_definition).collect()
}

fn parse_field_definition(pair: Pair<Rule>) -> Result<AstField, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::field_definition);

    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let type_info = inner.next().unwrap();

    let pk = next_if_rule(&mut inner, Rule::pk).map(|_| true);
    let column_name = inner.next().map(|column_pair| {
        column_pair
            .into_inner()
            .next()
            .unwrap()
            .as_str()
            .to_string()
    });

    parse_field_type(type_info).map(|type_info| {
        let type_modifier = if type_info.array {
            AstTypeModifier::List
        } else {
            AstTypeModifier::NonNull
        };

        let relation = match pk {
            Some(true) => AstRelation::Pk {
                auto_generated: false,
            },
            _ => AstRelation::Other {
                optional: type_info.optional,
            },
        };

        AstField {
            name,
            type_name: type_info.type_name,
            type_modifier,
            relation,
            column_name,
        }
    })
}

#[derive(Debug, Clone)]
struct TypeInfo {
    type_name: String,
    array: bool,
    optional: bool,
}

// "Foo" or "[Foo]" with or without the trailing "?"
fn parse_field_type(pair: Pair<Rule>) -> Result<TypeInfo, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::field_type);

    let mut inner = pair.into_inner();
    let base_type = inner.next().unwrap();
    let optional = inner.next().map(|_| true).unwrap_or(false);

    match base_type.as_rule() {
        Rule::field_base_type => parse_field_base_type(base_type),
        _ => unreachable!(),
    }
    .map(|type_info| TypeInfo {
        optional,
        ..type_info
    })
}

// "Foo" or "[Foo]""
fn parse_field_base_type(pair: Pair<Rule>) -> Result<TypeInfo, Error<Rule>> {
    debug_assert_eq!(pair.as_rule(), Rule::field_base_type);

    let inner = pair.into_inner().next().unwrap();

    match inner.as_rule() {
        Rule::field_array_type => {
            let type_name = inner.into_inner().next().unwrap().as_str().to_string();

            Ok(TypeInfo {
                type_name,
                array: true,
                optional: false,
            })
        }
        Rule::type_name => {
            let type_name = inner.as_str().to_string();

            Ok(TypeInfo {
                type_name,
                array: false,
                optional: false,
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn non_optional_simple_field() {
        let pair = compute_pair(Rule::field_definition, r#"foo: Foo"#);

        assert_eq!(
            parse_field_definition(pair),
            Ok(AstField {
                name: "foo".to_string(),
                type_name: "Foo".to_string(),
                type_modifier: AstTypeModifier::NonNull,
                relation: AstRelation::Other { optional: false },
                column_name: None
            })
        )
    }

    #[test]
    fn optional_base_type() {
        let pair = compute_pair(Rule::field_definition, r#"foo: Foo?"#);

        assert_eq!(
            parse_field_definition(pair),
            Ok(AstField {
                name: "foo".to_string(),
                type_name: "Foo".to_string(),
                type_modifier: AstTypeModifier::NonNull,
                relation: AstRelation::Other { optional: true },
                column_name: None
            })
        )
    }

    #[test]
    fn non_optional_list_type() {
        let pair = compute_pair(Rule::field_definition, r#"foo: [Foo]"#);

        assert_eq!(
            parse_field_definition(pair),
            Ok(AstField {
                name: "foo".to_string(),
                type_name: "Foo".to_string(),
                type_modifier: AstTypeModifier::List,
                relation: AstRelation::Other { optional: false },
                column_name: None
            })
        )
    }

    #[test]
    fn optional_list_type() {
        let pair = compute_pair(Rule::field_definition, r#"foo: [Foo]?"#);

        assert_eq!(
            parse_field_definition(pair),
            Ok(AstField {
                name: "foo".to_string(),
                type_name: "Foo".to_string(),
                type_modifier: AstTypeModifier::List,
                relation: AstRelation::Other { optional: true },
                column_name: None
            })
        )
    }

    #[test]
    fn with_column_name() {
        let pair = compute_pair(Rule::field_definition, r#"foo: [Foo]? @column(col)"#);

        assert_eq!(
            parse_field_definition(pair),
            Ok(AstField {
                name: "foo".to_string(),
                type_name: "Foo".to_string(),
                type_modifier: AstTypeModifier::List,
                relation: AstRelation::Other { optional: true },
                column_name: Some("col".to_string())
            })
        )
    }

    #[test]
    fn simple_type() {
        let pair = compute_pair(
            Rule::model_definition,
            r#"model Venue {
                name: String?
                address: String        
        }
        "#,
        );

        assert_eq!(parse_model_definition(pair), Ok(venue_type()))
    }

    #[test]
    fn with_table_type() {
        let pair = compute_pair(
            Rule::model_definition,
            r#"model Person @table(people) {
                first_name: String? @column(f_name)
                age: Int        
        }
        "#,
        );

        assert_eq!(parse_model_definition(pair), Ok(person_type()))
    }

    #[test]
    fn simple_system() {
        let pair = compute_pair(
            Rule::system_document,
            r#"model Venue {
                name: String?
                address: String        
        }
        
        model Person @table(people) {
                first_name: String? @column(f_name)
                age: Int        
        }
        "#,
        );

        assert_eq!(
            parse_system_document(pair),
            Ok(AstSystem {
                types: vec![venue_type(), person_type()]
            })
        )
    }

    fn compute_pair(rule: Rule, input: &str) -> Pair<Rule> {
        let parsed = PayasParser::parse(rule, input).expect("");
        parsed.into_iter().next().unwrap()
    }

    fn person_type() -> AstType {
        AstType {
            name: "Person".to_string(),
            kind: AstTypeKind::Composite {
                fields: vec![
                    AstField {
                        name: "first_name".to_string(),
                        type_name: "String".to_string(),
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: true },
                        column_name: Some("f_name".to_string()),
                    },
                    AstField {
                        name: "age".to_string(),
                        type_name: "Int".to_string(),
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
                        type_name: "String".to_string(),
                        type_modifier: AstTypeModifier::NonNull,
                        relation: AstRelation::Other { optional: true },
                        column_name: None,
                    },
                    AstField {
                        name: "address".to_string(),
                        type_name: "String".to_string(),
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
