// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

/// Built-in projection name: primary key fields only. Default for mutations.
pub const PROJECTION_PK: &str = "pk";
/// Built-in projection name: all scalars + ManyToOne as PK refs. Default for queries.
pub const PROJECTION_BASIC: &str = "basic";

/// A resolved projection — the concrete set of fields to include in a response.
///
/// Each projection is stored in two forms:
///
/// - `elements`: The compositional form, which may contain [`SelfProjection`](ProjectionElement::SelfProjection)
///   references. For example, `withVenue = [/basic, venue/basic]` produces:
///   ```text
///   elements: [SelfProjection("basic"), RelationProjection("venue", ["basic"])]
///   ```
///   Used by `exo reflect` to expose composition structure to SDK generators
///   (e.g., so they can produce `ConcertWithVenue extends Concert`).
///
/// - `resolved_elements`: The fully flattened form, precomputed at build time. Contains only
///   [`ScalarField`](ProjectionElement::ScalarField) and [`RelationProjection`](ProjectionElement::RelationProjection).
///   For the same `withVenue` example:
///   ```text
///   resolved_elements: [
///     ScalarField("id"), ScalarField("title"),
///     RelationProjection("venue", ["pk", "basic"]),  // pk from /basic, basic from explicit
///     ScalarField("published"), ScalarField("price"),
///   ]
///   ```
///   Used by the runtime (resolver, schema generation) with no per-request computation.
///
/// A `RelationProjection`'s `projection_names` (e.g., `["pk", "basic"]`) lists which projections
/// on the *foreign* entity to union for the nested select. At query time, each foreign projection's
/// own `resolved_elements` is used to get the flattened fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedProjection {
    pub name: String,
    /// Compositional form — may contain [`SelfProjection`](ProjectionElement::SelfProjection)
    /// references preserving the composition structure. Used by `exo reflect`.
    pub elements: Vec<ProjectionElement>,
    /// Fully flattened form — only [`ScalarField`](ProjectionElement::ScalarField) and
    /// [`RelationProjection`](ProjectionElement::RelationProjection). Precomputed at build time;
    /// used directly by the runtime with no per-request resolution.
    pub resolved_elements: Vec<ProjectionElement>,
}

impl ResolvedProjection {
    /// Compute `resolved_elements` by recursively expanding all `SelfProjection`
    /// entries, merging relation projections that appear in multiple sources.
    pub fn resolve_elements(
        elements: &[ProjectionElement],
        all_projections: &[ResolvedProjection],
    ) -> Vec<ProjectionElement> {
        let mut result: Vec<ProjectionElement> = Vec::new();

        for element in elements {
            let expanded: Vec<ProjectionElement> = match element {
                ProjectionElement::SelfProjection(name) => {
                    if let Some(referenced) = all_projections.iter().find(|p| p.name == *name) {
                        // Referenced projection's resolved_elements may not be populated yet
                        // during build, so fall back to resolving its elements recursively.
                        if referenced.resolved_elements.is_empty() {
                            Self::resolve_elements(&referenced.elements, all_projections)
                        } else {
                            referenced.resolved_elements.clone()
                        }
                    } else {
                        vec![]
                    }
                }
                other => vec![other.clone()],
            };

            for elem in expanded {
                merge_element(&mut result, elem);
            }
        }

        result
    }
}

/// An element in a resolved projection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProjectionElement {
    /// A scalar field of this entity (field name).
    ScalarField(String),
    /// A relation with one or more named projections on the foreign entity.
    ///
    /// At query time, each named projection's `resolved_elements` on the foreign entity is used
    /// to get the flattened fields. When multiple names are given (e.g., `["pk", "basic"]`),
    /// their fields are unioned and deduplicated.
    ///
    /// Multiple names arise from composition: if `/basic` contributes `venue/pk` and an explicit
    /// `venue/basic` is also listed, they merge into `RelationProjection("venue", ["pk", "basic"])`.
    RelationProjection {
        relation_field_name: String,
        projection_names: Vec<String>,
    },
    /// A reference to another projection on this same type (composition).
    /// For example, `withVenue = [/basic, venue/basic]` produces elements
    /// `[SelfProjection("basic"), RelationProjection("venue", ["basic"])]`.
    SelfProjection(String),
}

/// Merge a projection element into a list, deduplicating scalar fields and
/// merging relation projection names for the same relation field.
pub fn merge_element(target: &mut Vec<ProjectionElement>, element: ProjectionElement) {
    match &element {
        ProjectionElement::ScalarField(name) => {
            if !target
                .iter()
                .any(|e| matches!(e, ProjectionElement::ScalarField(n) if n == name))
            {
                target.push(element);
            }
        }
        ProjectionElement::RelationProjection {
            relation_field_name,
            projection_names: new_names,
        } => {
            if let Some(ProjectionElement::RelationProjection {
                projection_names: existing,
                ..
            }) = target.iter_mut().find(|e| {
                matches!(e, ProjectionElement::RelationProjection { relation_field_name: n, .. } if n == relation_field_name)
            }) {
                for name in new_names {
                    if !existing.contains(name) {
                        existing.push(name.clone());
                    }
                }
            } else {
                target.push(element);
            }
        }
        ProjectionElement::SelfProjection(name) => {
            if !target
                .iter()
                .any(|e| matches!(e, ProjectionElement::SelfProjection(n) if n == name))
            {
                target.push(element);
            }
        }
    }
}
