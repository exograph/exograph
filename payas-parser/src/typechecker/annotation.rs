use crate::ast::ast_types::{AstAnnotation, AstAnnotationParams};
use anyhow::{bail, Result};
use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};

use super::{annotation_params::TypedAnnotationParams, Scope, Type, Typecheck, TypedExpression};

use annotation_attribute::annotation;

#[annotation("access")]
#[allow(clippy::large_enum_variant)]
pub enum AccessAnnotation {
    Single(TypedExpression), // default access
    Map {
        query: Option<TypedExpression>,
        mutation: Option<TypedExpression>,
        create: Option<TypedExpression>,
        update: Option<TypedExpression>,
        delete: Option<TypedExpression>,
    },
}

#[annotation("autoincrement")]
pub enum AutoIncrementAnnotation {
    None,
}

#[annotation("bits")]
pub enum BitsAnnotation {
    Single(TypedExpression),
}

#[annotation("column")]
pub enum ColumnAnnotation {
    Single(TypedExpression),
}

#[annotation("dbtype")]
pub enum DbTypeAnnotation {
    Single(TypedExpression),
}

#[annotation("length")]
pub enum LengthAnnotation {
    Single(TypedExpression),
}

#[annotation("jwt")]
pub enum JwtAnnotation {
    None,
    Single(TypedExpression),
}

#[annotation("pk")]
pub enum PkAnnotation {
    None,
}

#[annotation("range")]
pub enum RangeAnnotation {
    Map {
        min: TypedExpression,
        max: TypedExpression,
    },
}

#[annotation("size")]
pub enum SizeAnnotation {
    Single(TypedExpression),
}

#[annotation("table")]
pub enum TableAnnotation {
    Single(TypedExpression),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TypedAnnotation {
    Access(AccessAnnotation),
    AutoIncrement(AutoIncrementAnnotation),
    Bits(BitsAnnotation),
    Column(ColumnAnnotation),
    DbType(DbTypeAnnotation),
    Length(LengthAnnotation),
    Jwt(JwtAnnotation),
    Pk(PkAnnotation),
    Range(RangeAnnotation),
    Size(SizeAnnotation),
    Table(TableAnnotation),
}

impl TypedAnnotation {
    pub fn name(&self) -> &str {
        match &self {
            TypedAnnotation::Access(_) => AccessAnnotation::name(),
            TypedAnnotation::AutoIncrement(_) => AutoIncrementAnnotation::name(),
            TypedAnnotation::Bits(_) => BitsAnnotation::name(),
            TypedAnnotation::Column(_) => ColumnAnnotation::name(),
            TypedAnnotation::DbType(_) => DbTypeAnnotation::name(),
            TypedAnnotation::Length(_) => LengthAnnotation::name(),
            TypedAnnotation::Jwt(_) => JwtAnnotation::name(),
            TypedAnnotation::Pk(_) => PkAnnotation::name(),
            TypedAnnotation::Range(_) => RangeAnnotation::name(),
            TypedAnnotation::Size(_) => SizeAnnotation::name(),
            TypedAnnotation::Table(_) => TableAnnotation::name(),
        }
    }
}

impl Typecheck<TypedAnnotation> for AstAnnotation {
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<TypedAnnotation> {
        let params = self.params.shallow(errors)?;
        let name = self.name.as_str();

        // Can't use match https://github.com/rust-lang/rust/issues/57240
        if name == AccessAnnotation::name() {
            Ok(TypedAnnotation::Access(AccessAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == AutoIncrementAnnotation::name() {
            Ok(TypedAnnotation::AutoIncrement(
                AutoIncrementAnnotation::from_params(self, params, errors)?,
            ))
        } else if name == BitsAnnotation::name() {
            Ok(TypedAnnotation::Bits(BitsAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == ColumnAnnotation::name() {
            Ok(TypedAnnotation::Column(ColumnAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == DbTypeAnnotation::name() {
            Ok(TypedAnnotation::DbType(DbTypeAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == LengthAnnotation::name() {
            Ok(TypedAnnotation::Length(LengthAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == JwtAnnotation::name() {
            Ok(TypedAnnotation::Jwt(JwtAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == PkAnnotation::name() {
            Ok(TypedAnnotation::Pk(PkAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == RangeAnnotation::name() {
            Ok(TypedAnnotation::Range(RangeAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == SizeAnnotation::name() {
            Ok(TypedAnnotation::Size(SizeAnnotation::from_params(
                self, params, errors,
            )?))
        } else if name == TableAnnotation::name() {
            Ok(TypedAnnotation::Table(TableAnnotation::from_params(
                self, params, errors,
            )?))
        } else {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("Unknown annotation `{}`", name),
                code: Some("A000".to_string()),
                spans: vec![SpanLabel {
                    span: self.span,
                    label: None,
                    style: SpanStyle::Primary,
                }],
            });
            bail!("")
        }
    }

    fn pass(
        &self,
        typ: &mut TypedAnnotation,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match typ {
            TypedAnnotation::Access(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::AutoIncrement(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Bits(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Column(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::DbType(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Length(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Jwt(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Pk(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Range(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Size(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Table(a) => a.pass(&self.params, env, scope, errors),
        }
    }
}
