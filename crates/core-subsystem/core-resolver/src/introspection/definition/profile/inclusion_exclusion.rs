// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{self, Deserialize};
use wildmatch::WildMatch;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InclusionExclusionSerialized {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl From<InclusionExclusionSerialized> for InclusionExclusion {
    fn from(serialized: InclusionExclusionSerialized) -> Self {
        InclusionExclusion {
            include: serialized
                .include
                .into_iter()
                .map(|s| WildMatch::new(&s))
                .collect(),
            exclude: serialized
                .exclude
                .into_iter()
                .map(|s| WildMatch::new(&s))
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct InclusionExclusion {
    pub(super) include: Vec<WildMatch>,
    pub(super) exclude: Vec<WildMatch>,
}

impl InclusionExclusion {
    pub fn matches(&self, name: &str) -> bool {
        let include_matches = self.include.iter().any(|pattern| pattern.matches(name));
        let exclude_matches = self.exclude.iter().any(|pattern| pattern.matches(name));
        include_matches && !exclude_matches
    }

    pub fn all() -> Self {
        Self {
            include: vec![WildMatch::new("*")],
            exclude: vec![],
        }
    }

    pub fn none() -> Self {
        Self {
            include: vec![],
            exclude: vec![WildMatch::new("*")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all() {
        let pattern = InclusionExclusion::all();
        assert!(pattern.matches("foo"));
        assert!(pattern.matches("bar"));
    }

    #[test]
    fn none() {
        let pattern = InclusionExclusion::none();
        assert!(!pattern.matches("foo"));
        assert!(!pattern.matches("bar"));
    }

    #[test]
    fn include() {
        let pattern = InclusionExclusion {
            include: vec![WildMatch::new("foo"), WildMatch::new("bar")],
            exclude: vec![],
        };

        assert!(pattern.matches("foo"));
        assert!(pattern.matches("bar"));
        assert!(!pattern.matches("baz"));
    }

    #[test]
    fn exclude() {
        let pattern = InclusionExclusion {
            include: vec![WildMatch::new("*")],
            exclude: vec![WildMatch::new("foo"), WildMatch::new("bar")],
        };

        assert!(!pattern.matches("foo"));
        assert!(!pattern.matches("bar"));
        assert!(pattern.matches("baz"));
    }

    #[test]
    fn mixed() {
        let pattern = InclusionExclusion {
            include: vec![WildMatch::new("foo*"), WildMatch::new("bar*")],
            exclude: vec![WildMatch::new("bar2")],
        };

        assert!(pattern.matches("foo1"));
        assert!(pattern.matches("bar1"));
        assert!(!pattern.matches("bar2"));
        assert!(!pattern.matches("baz"));
    }
}
