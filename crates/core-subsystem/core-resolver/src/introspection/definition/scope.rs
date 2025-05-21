// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_env::Environment;
use wildmatch::WildMatch;

#[derive(Debug, Clone)]
pub struct SchemaScope {
    pub(super) query_entities: SchemaScopeFilter,
    pub(super) mutation_entities: SchemaScopeFilter,
    pub(super) query_names: SchemaScopeFilter,
    pub(super) mutation_names: SchemaScopeFilter,
}

impl SchemaScope {
    pub fn new(
        query_entities: SchemaScopeFilter,
        mutation_entities: SchemaScopeFilter,
        query_names: SchemaScopeFilter,
        mutation_names: SchemaScopeFilter,
    ) -> Self {
        Self {
            query_entities,
            mutation_entities,
            query_names,
            mutation_names,
        }
    }

    pub fn all() -> Self {
        Self {
            query_entities: SchemaScopeFilter::all(),
            mutation_entities: SchemaScopeFilter::all(),
            query_names: SchemaScopeFilter::all(),
            mutation_names: SchemaScopeFilter::all(),
        }
    }

    pub fn queries_only() -> Self {
        Self {
            query_entities: SchemaScopeFilter::all(),
            mutation_entities: SchemaScopeFilter::none(),
            query_names: SchemaScopeFilter::all(),
            mutation_names: SchemaScopeFilter::none(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SchemaScopeFilter {
    All,
    None,
    List(Vec<ScopeMatch>),
}

#[derive(Debug, Clone)]
pub enum ScopeMatch {
    Include(WildMatch),
    Exclude(WildMatch),
}

impl ScopeMatch {
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::Include(pattern) => pattern.matches(name),
            Self::Exclude(pattern) => !pattern.matches(name),
        }
    }
}

impl SchemaScopeFilter {
    pub fn new(matches: Vec<ScopeMatch>) -> Self {
        Self::List(matches)
    }

    pub fn new_from_env(env: &dyn Environment, key: &str, default: impl FnOnce() -> Self) -> Self {
        let patterns = env.get_list(key, vec![]);

        if patterns.is_empty() {
            default()
        } else {
            let matches = patterns
                .iter()
                .map(|pattern| {
                    if let Some(pattern) = pattern.strip_prefix("-") {
                        ScopeMatch::Exclude(WildMatch::new(pattern))
                    } else if let Some(pattern) = pattern.strip_prefix("+") {
                        ScopeMatch::Include(WildMatch::new(pattern))
                    } else {
                        ScopeMatch::Include(WildMatch::new(pattern))
                    }
                })
                .collect();

            Self::new(matches)
        }
    }

    pub fn all() -> Self {
        Self::All
    }

    pub fn none() -> Self {
        Self::None
    }

    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::List(patterns) => {
                let (has_include, include_matches, exclude_matches) = patterns.iter().fold(
                    (false, false, false),
                    |(has_include, include_matches, exclude_matches), pattern| match pattern {
                        ScopeMatch::Include(p) => {
                            (true, include_matches || p.matches(name), exclude_matches)
                        }
                        ScopeMatch::Exclude(p) => (
                            has_include,
                            include_matches,
                            exclude_matches || p.matches(name),
                        ),
                    },
                );

                (!has_include || include_matches) && !exclude_matches
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use exo_env::MapEnvironment;

    use super::*;

    #[test]
    fn all() {
        let scope = SchemaScopeFilter::all();
        assert!(scope.matches("foo"));
        assert!(scope.matches("bar"));
    }

    #[test]
    fn none() {
        let scope = SchemaScopeFilter::none();
        assert!(!scope.matches("foo"));
        assert!(!scope.matches("bar"));
    }

    #[test]
    fn include() {
        for pattern in ["foo,bar", "+foo,+bar", "foo, bar", "+foo, +bar"] {
            let env: MapEnvironment = [("PATTERNS", pattern)].into();
            let scope = SchemaScopeFilter::new_from_env(&env, "PATTERNS", SchemaScopeFilter::all);
            assert!(scope.matches("foo"));
            assert!(scope.matches("bar"));
            assert!(!scope.matches("baz"));
        }
    }

    #[test]
    fn exclude() {
        for pattern in ["-foo,-bar", "-foo, -bar"] {
            let env: MapEnvironment = [("PATTERNS", pattern)].into();
            let scope = SchemaScopeFilter::new_from_env(&env, "PATTERNS", SchemaScopeFilter::all);
            assert!(!scope.matches("foo"));
            assert!(!scope.matches("bar"));
            assert!(scope.matches("baz"));
        }
    }

    #[test]
    fn mixed() {
        for pattern in ["foo*,bar*,-bar2", "foo*, bar*,-bar2", "+foo*,+bar*,-bar2"] {
            let env: MapEnvironment = [("PATTERNS", pattern)].into();
            let scope = SchemaScopeFilter::new_from_env(&env, "PATTERNS", SchemaScopeFilter::all);
            assert!(scope.matches("foo1"));
            assert!(scope.matches("bar1"));
            assert!(!scope.matches("bar2"));
            assert!(!scope.matches("baz"));
        }
    }
}
