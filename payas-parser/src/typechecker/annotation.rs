use crate::ast::ast_types::{AstAnnotation, AstAnnotationParams};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

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
    fn shallow(&self) -> TypedAnnotation {
        let params = self.params.shallow();
        let name = self.name.as_str();

        // Can't use match https://github.com/rust-lang/rust/issues/57240
        if name == AccessAnnotation::name() {
            TypedAnnotation::Access(AccessAnnotation::from_params(params).unwrap())
        } else if name == AutoIncrementAnnotation::name() {
            TypedAnnotation::AutoIncrement(AutoIncrementAnnotation::from_params(params).unwrap())
        } else if name == BitsAnnotation::name() {
            TypedAnnotation::Bits(BitsAnnotation::from_params(params).unwrap())
        } else if name == ColumnAnnotation::name() {
            TypedAnnotation::Column(ColumnAnnotation::from_params(params).unwrap())
        } else if name == DbTypeAnnotation::name() {
            TypedAnnotation::DbType(DbTypeAnnotation::from_params(params).unwrap())
        } else if name == LengthAnnotation::name() {
            TypedAnnotation::Length(LengthAnnotation::from_params(params).unwrap())
        } else if name == JwtAnnotation::name() {
            TypedAnnotation::Jwt(JwtAnnotation::from_params(params).unwrap())
        } else if name == PkAnnotation::name() {
            TypedAnnotation::Pk(PkAnnotation::from_params(params).unwrap())
        } else if name == RangeAnnotation::name() {
            TypedAnnotation::Range(RangeAnnotation::from_params(params).unwrap())
        } else if name == SizeAnnotation::name() {
            TypedAnnotation::Size(SizeAnnotation::from_params(params).unwrap())
        } else if name == TableAnnotation::name() {
            TypedAnnotation::Table(TableAnnotation::from_params(params).unwrap())
        } else {
            panic!("unknown annotation {}", name);
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
