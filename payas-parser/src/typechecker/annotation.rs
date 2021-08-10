use crate::ast::ast_types::{AstAnnotation, AstAnnotationParams, AstExpr, Untyped};
use anyhow::{bail, Result};
use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};

use super::{annotation_params::TypedAnnotationParams, Scope, Type, TypecheckFrom, Typed};

use annotation_attribute::annotation;

#[annotation("access")]
#[allow(clippy::large_enum_variant)]
pub enum AccessAnnotation {
    Single(AstExpr<Typed>), // default access
    Map {
        query: Option<AstExpr<Typed>>,
        mutation: Option<AstExpr<Typed>>,
        create: Option<AstExpr<Typed>>,
        update: Option<AstExpr<Typed>>,
        delete: Option<AstExpr<Typed>>,
    },
}

#[annotation("autoincrement")]
pub enum AutoIncrementAnnotation {
    None,
}

#[annotation("bits")]
pub enum BitsAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("column")]
pub enum ColumnAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("dbtype")]
pub enum DbTypeAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("length")]
pub enum LengthAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("plural_name")]
pub enum PluralNameAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("precision")]
pub enum PrecisionAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("scale")]
pub enum ScaleAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("jwt")]
pub enum JwtAnnotation {
    None,
    Single(AstExpr<Typed>),
}

#[annotation("pk")]
pub enum PkAnnotation {
    None,
}

#[annotation("range")]
pub enum RangeAnnotation {
    Map {
        min: AstExpr<Typed>,
        max: AstExpr<Typed>,
    },
}

#[annotation("size")]
pub enum SizeAnnotation {
    Single(AstExpr<Typed>),
}

#[annotation("table")]
pub enum TableAnnotation {
    Single(AstExpr<Typed>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TypedAnnotation {
    Access(AccessAnnotation),
    AutoIncrement(AutoIncrementAnnotation),
    Bits(BitsAnnotation),
    Column(ColumnAnnotation),
    DbType(DbTypeAnnotation),
    Length(LengthAnnotation),
    PluralName(PluralNameAnnotation),
    Precision(PrecisionAnnotation),
    Scale(ScaleAnnotation),
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
            TypedAnnotation::PluralName(_) => PluralNameAnnotation::name(),
            TypedAnnotation::Precision(_) => PrecisionAnnotation::name(),
            TypedAnnotation::Scale(_) => ScaleAnnotation::name(),
            TypedAnnotation::Jwt(_) => JwtAnnotation::name(),
            TypedAnnotation::Pk(_) => PkAnnotation::name(),
            TypedAnnotation::Range(_) => RangeAnnotation::name(),
            TypedAnnotation::Size(_) => SizeAnnotation::name(),
            TypedAnnotation::Table(_) => TableAnnotation::name(),
        }
    }
}

impl TypecheckFrom<AstAnnotation<Untyped>> for TypedAnnotation {
    fn shallow(
        untyped: &AstAnnotation<Untyped>,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> Result<TypedAnnotation> {
        let params = TypedAnnotationParams::shallow(&untyped.params, errors)?;
        let name = untyped.name.as_str();

        // Can't use match https://github.com/rust-lang/rust/issues/57240
        if name == AccessAnnotation::name() {
            Ok(TypedAnnotation::Access(AccessAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == AutoIncrementAnnotation::name() {
            Ok(TypedAnnotation::AutoIncrement(
                AutoIncrementAnnotation::from_params(untyped, params, errors)?,
            ))
        } else if name == BitsAnnotation::name() {
            Ok(TypedAnnotation::Bits(BitsAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == ColumnAnnotation::name() {
            Ok(TypedAnnotation::Column(ColumnAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == DbTypeAnnotation::name() {
            Ok(TypedAnnotation::DbType(DbTypeAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == LengthAnnotation::name() {
            Ok(TypedAnnotation::Length(LengthAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == PluralNameAnnotation::name() {
            Ok(TypedAnnotation::PluralName(
                PluralNameAnnotation::from_params(untyped, params, errors)?,
            ))
        } else if name == PrecisionAnnotation::name() {
            Ok(TypedAnnotation::Precision(
                PrecisionAnnotation::from_params(untyped, params, errors)?,
            ))
        } else if name == ScaleAnnotation::name() {
            Ok(TypedAnnotation::Scale(ScaleAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == JwtAnnotation::name() {
            Ok(TypedAnnotation::Jwt(JwtAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == PkAnnotation::name() {
            Ok(TypedAnnotation::Pk(PkAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == RangeAnnotation::name() {
            Ok(TypedAnnotation::Range(RangeAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == SizeAnnotation::name() {
            Ok(TypedAnnotation::Size(SizeAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else if name == TableAnnotation::name() {
            Ok(TypedAnnotation::Table(TableAnnotation::from_params(
                untyped, params, errors,
            )?))
        } else {
            errors.push(Diagnostic {
                level: Level::Error,
                message: format!("Unknown annotation `{}`", name),
                code: Some("A000".to_string()),
                spans: vec![SpanLabel {
                    span: untyped.span,
                    label: None,
                    style: SpanStyle::Primary,
                }],
            });
            bail!("")
        }
    }

    fn pass(
        &mut self,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        match self {
            TypedAnnotation::Access(a) => a.pass(env, scope, errors),
            TypedAnnotation::AutoIncrement(a) => a.pass(env, scope, errors),
            TypedAnnotation::Bits(a) => a.pass(env, scope, errors),
            TypedAnnotation::Column(a) => a.pass(env, scope, errors),
            TypedAnnotation::DbType(a) => a.pass(env, scope, errors),
            TypedAnnotation::Length(a) => a.pass(env, scope, errors),
            TypedAnnotation::PluralName(a) => a.pass(env, scope, errors),
            TypedAnnotation::Precision(a) => a.pass(env, scope, errors),
            TypedAnnotation::Scale(a) => a.pass(env, scope, errors),
            TypedAnnotation::Jwt(a) => a.pass(env, scope, errors),
            TypedAnnotation::Pk(a) => a.pass(env, scope, errors),
            TypedAnnotation::Range(a) => a.pass(env, scope, errors),
            TypedAnnotation::Size(a) => a.pass(env, scope, errors),
            TypedAnnotation::Table(a) => a.pass(env, scope, errors),
        }
    }
}
