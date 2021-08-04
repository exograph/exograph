use heck::MixedCase;

use crate::model::GqlType;

/// A type with both singular and plural versions of itself.
pub trait ToPlural {
    fn to_singular(&self) -> String;
    fn to_plural(&self) -> String;
}

impl ToPlural for str {
    fn to_singular(&self) -> String {
        self.to_owned()
    }

    fn to_plural(&self) -> String {
        format!("{}s", self)
    }
}

impl ToPlural for GqlType {
    fn to_singular(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

/// A type that can generate GraphQL query names.
pub trait ToGqlQueryName {
    /// Single query name (e.g. `concert`)
    fn pk_query(&self) -> String;
    /// Plural query name (e.g. `concerts`)
    fn collection_query(&self) -> String;
}

fn to_query(name: &str) -> String {
    name.to_mixed_case()
}

impl<T: ToPlural> ToGqlQueryName for T {
    fn pk_query(&self) -> String {
        to_query(&self.to_singular())
    }

    fn collection_query(&self) -> String {
        to_query(&self.to_plural())
    }
}

fn to_create(name: &str) -> String {
    format!("create{}", name)
}

fn to_delete(name: &str) -> String {
    format!("delete{}", name)
}

fn to_update(name: &str) -> String {
    format!("update{}", name)
}

/// A type that can generate GraphQL mutation names.
pub trait ToGqlMutationNames {
    /// Single create name (e.g. `createConcert`)
    fn pk_create(&self) -> String;
    /// Single delete name (e.g. `deleteConcert`)
    fn pk_delete(&self) -> String;
    /// Single update name (e.g. `updateConcert`)
    fn pk_update(&self) -> String;
    /// Plural create name (e.g. `createConcerts`)
    fn collection_create(&self) -> String;
    /// Plural delete name (e.g. `deleteConcerts`)
    fn collection_delete(&self) -> String;
    /// Plural update name (e.g. `updateConcerts`)
    fn collection_update(&self) -> String;
}

impl<T: ToPlural> ToGqlMutationNames for T {
    fn pk_create(&self) -> String {
        to_create(&self.to_singular())
    }

    fn pk_delete(&self) -> String {
        to_delete(&self.to_singular())
    }

    fn pk_update(&self) -> String {
        to_update(&self.to_singular())
    }

    fn collection_create(&self) -> String {
        to_create(&self.to_plural())
    }

    fn collection_delete(&self) -> String {
        to_delete(&self.to_plural())
    }

    fn collection_update(&self) -> String {
        to_update(&self.to_plural())
    }
}

fn to_creation_type(name: &str) -> String {
    format!("{}CreationInput", name)
}

fn to_update_type(name: &str) -> String {
    format!("{}UpdateInput", name)
}

fn to_reference_type(name: &str) -> String {
    format!("{}ReferenceInput", name)
}

/// A type that can generate GraphQL type names.
pub trait ToGqlTypeNames {
    /// Creation type name (e.g. `ConcertCreationInput`)
    fn creation_type(&self) -> String;
    /// Update type name (e.g. `ConcertUpdateInput`)
    fn update_type(&self) -> String;
    /// Reference type name (e.g. `ConcertReferenceInput`)
    fn reference_type(&self) -> String;
}

impl ToGqlTypeNames for str {
    fn creation_type(&self) -> String {
        to_creation_type(self)
    }

    fn update_type(&self) -> String {
        to_update_type(self)
    }

    fn reference_type(&self) -> String {
        to_reference_type(self)
    }
}

impl<T: ToPlural> ToGqlTypeNames for T {
    fn creation_type(&self) -> String {
        to_creation_type(&self.to_singular())
    }

    fn update_type(&self) -> String {
        to_update_type(&self.to_singular())
    }

    fn reference_type(&self) -> String {
        to_reference_type(&self.to_singular())
    }
}
