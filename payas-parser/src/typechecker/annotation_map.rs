use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::AstAnnotation;

use super::{
    AccessAnnotation, AutoIncrementAnnotation, BitsAnnotation, ColumnAnnotation, DbTypeAnnotation,
    JwtAnnotation, LengthAnnotation, PkAnnotation, RangeAnnotation, Scope, SizeAnnotation,
    TableAnnotation, Type, Typecheck, TypedAnnotation,
};

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

macro_rules! coerce {
    ($annot:expr, $v:path) => {
        $annot
            .as_ref()
            .map(|a| if let $v(a) = a { a } else { panic!() })
    };
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
        let mut changed = false;

        for annot in [
            &mut self.access,
            &mut self.auto_increment,
            &mut self.bits,
            &mut self.column,
            &mut self.db_type,
            &mut self.length,
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
