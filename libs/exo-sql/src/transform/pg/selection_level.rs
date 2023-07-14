use crate::{Database, RelationId};

/// Selection level represents the level of a subselection in a query.
#[derive(Debug, Clone)]
pub enum SelectionLevel {
    /// Top level selection
    TopLevel,
    /// Nested sub selection, which each element representing the relation between parent and child selection
    /// For example, if we have a query like: `users { documents { .. }}`, the selection level for the documents
    /// selection will be `Nested(vec![RelationId::ManyToOne(<documents.user_id, users.id>)])`.
    Nested(Vec<RelationId>),
}

const ALIAS_SEPARATOR: &str = "$";

impl SelectionLevel {
    pub(super) fn is_top_level(&self) -> bool {
        matches!(self, SelectionLevel::TopLevel)
    }

    pub(super) fn with_relation_id(&self, relation_id: RelationId) -> Self {
        match self {
            SelectionLevel::TopLevel => SelectionLevel::Nested(vec![relation_id]),
            SelectionLevel::Nested(relation_ids) => {
                let mut relation_ids = relation_ids.clone();
                relation_ids.push(relation_id);
                SelectionLevel::Nested(relation_ids)
            }
        }
    }

    pub(super) fn tail_relation_id(&self) -> Option<&RelationId> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => relation_ids.last(),
        }
    }

    /// Compute a suitable alias for this selection level
    pub(crate) fn alias(&self, name: String, database: &Database) -> String {
        match self {
            SelectionLevel::TopLevel => name,
            SelectionLevel::Nested(relation_ids) => {
                relation_ids.iter().rev().fold(name, |acc, relation_id| {
                    let foreign_table_id = match relation_id {
                        RelationId::ManyToOne(r) => r.deref(database).self_column_id.table_id,
                        RelationId::OneToMany(r) => r.deref(database).self_pk_column_id.table_id,
                    };
                    let table_name = &database.get_table(foreign_table_id).name;
                    format!("{table_name}{ALIAS_SEPARATOR}{acc}")
                })
            }
        }
    }
}
