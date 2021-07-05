use crate::ast::ast_types::{AstAnnotation, AstAnnotationParams, AstExpr};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use super::{annotation_params::TypedAnnotationParams, Scope, Type, Typecheck, TypedExpression};

use annotation_attribute::{annotation, unchecked_annotation};

#[unchecked_annotation("access")]
pub struct AccessAnnotation;

#[annotation("autoincrement")]
pub struct AutoIncrementAnnotation;

#[annotation("bits")]
pub struct BitsAnnotation(pub TypedExpression);

#[annotation("column")]
pub struct ColumnAnnotation(pub TypedExpression);

#[annotation("dbtype")]
pub struct DbTypeAnnotation(pub TypedExpression);

#[annotation("length")]
pub struct LengthAnnotation(pub TypedExpression);

#[unchecked_annotation("jwt")]
pub struct JwtAnnotation;

#[annotation("pk")]
pub struct PkAnnotation;

#[annotation("range")]
pub struct RangeAnnotation {
    pub min: TypedExpression,
    pub max: TypedExpression,
}

#[annotation("size")]
pub struct SizeAnnotation(pub TypedExpression);

#[annotation("table")]
pub struct TableAnnotation(pub TypedExpression);

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
            // Unchecked annotations
            TypedAnnotation::Access(a) => a.pass(&self.params, env, scope, errors),
            TypedAnnotation::Jwt(a) => a.pass(&self.params, env, scope, errors),
            // Regular annotations
            _ => match &self.params {
                AstAnnotationParams::None => false,
                AstAnnotationParams::Single(expr) => match typ {
                    TypedAnnotation::Bits(a) => a.pass(&expr, env, scope, errors),
                    TypedAnnotation::Column(a) => a.pass(&expr, env, scope, errors),
                    TypedAnnotation::DbType(a) => a.pass(&expr, env, scope, errors),
                    TypedAnnotation::Length(a) => a.pass(&expr, env, scope, errors),
                    TypedAnnotation::Size(a) => a.pass(&expr, env, scope, errors),
                    TypedAnnotation::Table(a) => a.pass(&expr, env, scope, errors),
                    _ => panic!(),
                },
                AstAnnotationParams::Map(params) => match typ {
                    TypedAnnotation::Range(a) => a.pass(&params, env, scope, errors),
                    _ => panic!(),
                },
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnnotationMap {
    access: Option<TypedAnnotation>,
    auto_increment: Option<TypedAnnotation>,
    bits: Option<TypedAnnotation>,
    column: Option<TypedAnnotation>,
    db_type: Option<TypedAnnotation>,
    length: Option<TypedAnnotation>,
    jwt: Option<TypedAnnotation>,
    pk: Option<TypedAnnotation>,
    range: Option<TypedAnnotation>,
    size: Option<TypedAnnotation>,
    table: Option<TypedAnnotation>,
}

impl AnnotationMap {
    pub fn access(&self) -> Option<&AccessAnnotation> {
        self.access.as_ref().map(|a| {
            if let TypedAnnotation::Access(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn auto_increment(&self) -> Option<&AutoIncrementAnnotation> {
        self.auto_increment.as_ref().map(|a| {
            if let TypedAnnotation::AutoIncrement(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn bits(&self) -> Option<&BitsAnnotation> {
        self.bits.as_ref().map(|a| {
            if let TypedAnnotation::Bits(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn column(&self) -> Option<&ColumnAnnotation> {
        self.column.as_ref().map(|a| {
            if let TypedAnnotation::Column(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn db_type(&self) -> Option<&DbTypeAnnotation> {
        self.db_type.as_ref().map(|a| {
            if let TypedAnnotation::DbType(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn length(&self) -> Option<&LengthAnnotation> {
        self.length.as_ref().map(|a| {
            if let TypedAnnotation::Length(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn jwt(&self) -> Option<&JwtAnnotation> {
        self.jwt.as_ref().map(|a| {
            if let TypedAnnotation::Jwt(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn pk(&self) -> Option<&PkAnnotation> {
        self.pk.as_ref().map(|a| {
            if let TypedAnnotation::Pk(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn range(&self) -> Option<&RangeAnnotation> {
        self.range.as_ref().map(|a| {
            if let TypedAnnotation::Range(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn size(&self) -> Option<&SizeAnnotation> {
        self.size.as_ref().map(|a| {
            if let TypedAnnotation::Size(a) = a {
                a
            } else {
                panic!()
            }
        })
    }

    pub fn table(&self) -> Option<&TableAnnotation> {
        self.table.as_ref().map(|a| {
            if let TypedAnnotation::Table(a) = a {
                a
            } else {
                panic!()
            }
        })
    }
}

impl AnnotationMap {
    pub fn add_annotation(&mut self, annotation: TypedAnnotation) {
        match annotation {
            TypedAnnotation::Access(_) => self.access = Some(annotation),
            TypedAnnotation::AutoIncrement(_) => self.auto_increment = Some(annotation),
            TypedAnnotation::Bits(_) => self.bits = Some(annotation),
            TypedAnnotation::Column(_) => self.column = Some(annotation),
            TypedAnnotation::DbType(_) => self.db_type = Some(annotation),
            TypedAnnotation::Length(_) => self.length = Some(annotation),
            TypedAnnotation::Jwt(_) => self.jwt = Some(annotation),
            TypedAnnotation::Pk(_) => self.pk = Some(annotation),
            TypedAnnotation::Range(_) => self.range = Some(annotation),
            TypedAnnotation::Size(_) => self.size = Some(annotation),
            TypedAnnotation::Table(_) => self.table = Some(annotation),
        }
    }

    pub fn pass(
        &mut self,
        ast_annotations: &[AstAnnotation],
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let mut pass = false;

        macro_rules! c {
            ($f:expr, $t:path) => {
                pass = pass
                    || if let Some(annot) = $f.as_mut() {
                        ast_annotations
                            .iter()
                            .find(|a| a.name.as_str() == annot.name())
                            .unwrap()
                            .pass(annot, env, scope, errors)
                    } else {
                        false
                    }
            };
        }

        c!(self.access, TypedAnnotation::Access);
        c!(self.auto_increment, TypedAnnotation::AutoIncrement);
        c!(self.bits, TypedAnnotation::Bits);
        c!(self.column, TypedAnnotation::Column);
        c!(self.db_type, TypedAnnotation::DbType);
        c!(self.length, TypedAnnotation::Length);
        c!(self.jwt, TypedAnnotation::Jwt);
        c!(self.pk, TypedAnnotation::Pk);
        c!(self.range, TypedAnnotation::Range);
        c!(self.size, TypedAnnotation::Size);
        c!(self.table, TypedAnnotation::Table);
        pass
    }
}
