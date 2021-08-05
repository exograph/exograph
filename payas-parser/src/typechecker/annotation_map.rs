use anyhow::{bail, Result};
use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::{AstAnnotation, Untyped};

use super::{
    AccessAnnotation, AutoIncrementAnnotation, BitsAnnotation, ColumnAnnotation, DbTypeAnnotation,
    JwtAnnotation, LengthAnnotation, PkAnnotation, PluralNameAnnotation, PrecisionAnnotation,
    RangeAnnotation, ScaleAnnotation, Scope, SizeAnnotation, TableAnnotation, Type, Typecheck,
    TypedAnnotation,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnnotationMap {
    access: Option<TypedAnnotation>,
    auto_increment: Option<TypedAnnotation>,
    bits: Option<TypedAnnotation>,
    column: Option<TypedAnnotation>,
    db_type: Option<TypedAnnotation>,
    length: Option<TypedAnnotation>,
    plural_name: Option<TypedAnnotation>,
    precision: Option<TypedAnnotation>,
    scale: Option<TypedAnnotation>,
    jwt: Option<TypedAnnotation>,
    pk: Option<TypedAnnotation>,
    range: Option<TypedAnnotation>,
    size: Option<TypedAnnotation>,
    table: Option<TypedAnnotation>,
}

macro_rules! coerce {
    ($annot:expr, $v:path) => {
        $annot
            .as_ref()
            .map(|a| if let $v(a) = a { a } else { panic!() })
    };
}

impl AnnotationMap {
    pub fn add(
        &mut self,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
        annotation: TypedAnnotation,
        span: Span,
    ) -> Result<()> {
        macro_rules! s {
            ($field:expr, $errors:expr, $annotation:expr, $span:expr) => {
                match &$field {
                    Some(_) => {
                        $errors.push(Diagnostic {
                            level: Level::Error,
                            message: format!("Duplicate annotation `{}`", $annotation.name()),
                            code: Some("A000".to_string()),
                            spans: vec![SpanLabel {
                                span: $span,
                                label: None,
                                style: SpanStyle::Primary,
                            }],
                        });
                        bail!("");
                    }
                    None => $field = Some($annotation),
                }
            };
        }

        match annotation {
            TypedAnnotation::Access(_) => s!(self.access, errors, annotation, span),
            TypedAnnotation::AutoIncrement(_) => s!(self.auto_increment, errors, annotation, span),
            TypedAnnotation::Bits(_) => s!(self.bits, errors, annotation, span),
            TypedAnnotation::Column(_) => s!(self.column, errors, annotation, span),
            TypedAnnotation::DbType(_) => s!(self.db_type, errors, annotation, span),
            TypedAnnotation::Length(_) => s!(self.length, errors, annotation, span),
            TypedAnnotation::PluralName(_) => s!(self.plural_name, errors, annotation, span),
            TypedAnnotation::Precision(_) => s!(self.precision, errors, annotation, span),
            TypedAnnotation::Scale(_) => s!(self.scale, errors, annotation, span),
            TypedAnnotation::Jwt(_) => s!(self.jwt, errors, annotation, span),
            TypedAnnotation::Pk(_) => s!(self.pk, errors, annotation, span),
            TypedAnnotation::Range(_) => s!(self.range, errors, annotation, span),
            TypedAnnotation::Size(_) => s!(self.size, errors, annotation, span),
            TypedAnnotation::Table(_) => s!(self.table, errors, annotation, span),
        }
        Ok(())
    }

    pub fn pass(
        &mut self,
        ast_annotations: &[AstAnnotation<Untyped>],
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        let mut changed = false;

        for annot in [
            &mut self.access,
            &mut self.auto_increment,
            &mut self.bits,
            &mut self.column,
            &mut self.db_type,
            &mut self.length,
            &mut self.plural_name,
            &mut self.precision,
            &mut self.scale,
            &mut self.jwt,
            &mut self.pk,
            &mut self.range,
            &mut self.size,
            &mut self.table,
        ] {
            let annot_changed = if let Some(annot) = annot.as_mut() {
                ast_annotations
                    .iter()
                    .find(|a| a.name.as_str() == annot.name())
                    .unwrap()
                    .pass(annot, env, scope, errors)
            } else {
                false
            };

            changed = changed || annot_changed;
        }
        changed
    }

    pub fn access(&self) -> Option<&AccessAnnotation> {
        coerce!(self.access, TypedAnnotation::Access)
    }

    pub fn auto_increment(&self) -> Option<&AutoIncrementAnnotation> {
        coerce!(self.auto_increment, TypedAnnotation::AutoIncrement)
    }

    pub fn bits(&self) -> Option<&BitsAnnotation> {
        coerce!(self.bits, TypedAnnotation::Bits)
    }

    pub fn column(&self) -> Option<&ColumnAnnotation> {
        coerce!(self.column, TypedAnnotation::Column)
    }

    pub fn db_type(&self) -> Option<&DbTypeAnnotation> {
        coerce!(self.db_type, TypedAnnotation::DbType)
    }

    pub fn length(&self) -> Option<&LengthAnnotation> {
        coerce!(self.length, TypedAnnotation::Length)
    }

    pub fn plural_name(&self) -> Option<&PluralNameAnnotation> {
        coerce!(self.plural_name, TypedAnnotation::PluralName)
    }

    pub fn precision(&self) -> Option<&PrecisionAnnotation> {
        coerce!(self.precision, TypedAnnotation::Precision)
    }

    pub fn scale(&self) -> Option<&ScaleAnnotation> {
        coerce!(self.scale, TypedAnnotation::Scale)
    }

    pub fn jwt(&self) -> Option<&JwtAnnotation> {
        coerce!(self.jwt, TypedAnnotation::Jwt)
    }

    pub fn pk(&self) -> Option<&PkAnnotation> {
        coerce!(self.pk, TypedAnnotation::Pk)
    }

    pub fn range(&self) -> Option<&RangeAnnotation> {
        coerce!(self.range, TypedAnnotation::Range)
    }

    pub fn size(&self) -> Option<&SizeAnnotation> {
        coerce!(self.size, TypedAnnotation::Size)
    }

    pub fn table(&self) -> Option<&TableAnnotation> {
        coerce!(self.table, TypedAnnotation::Table)
    }
}
