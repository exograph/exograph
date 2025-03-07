// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use heck::ToSnakeCase;
use postgres_core_model::types::EntityType;

/// A type with both singular and plural versions of itself.
pub trait ToPlural {
    fn self_name(&self) -> String;
    fn to_plural(&self) -> String;
}

impl ToPlural for str {
    fn self_name(&self) -> String {
        self.to_owned()
    }

    fn to_plural(&self) -> String {
        let plural_name = pluralizer::pluralize(self, 2, false);
        if plural_name == self {
            // Force pluralization if the pluralizer returns the same string
            format!("{self}s")
        } else {
            plural_name
        }
    }
}

impl ToPlural for EntityType {
    fn self_name(&self) -> String {
        self.name.clone()
    }

    fn to_plural(&self) -> String {
        self.plural_name.clone()
    }
}

pub trait ToTableName {
    fn table_name(&self, plural_name: Option<String>) -> String;
}

impl ToTableName for str {
    fn table_name(&self, plural_name: Option<String>) -> String {
        match plural_name {
            Some(plural_name) => plural_name.to_snake_case(),
            None => self.to_plural().to_snake_case(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn table_names() {
        assert_eq!("concerts", "Concert".table_name(None));
        assert_eq!(
            "cons_foos",
            "Concert".table_name(Some("consFoos".to_string()))
        );

        assert_eq!("concert_artists", "ConcertArtist".table_name(None));
    }
}
