use exo_sql_pg::{
    ColumnId, ColumnPathLink, Database, ManyToOne, PhysicalColumnPath, SchemaObjectName, TableId,
};

/// Resolve a dot-path like `["concerts", "venue_id", "name"]` into a `(TableId, PhysicalColumnPath)`.
///
/// The first segment is the root table name. Intermediate segments are FK column names
/// identifying ManyToOne relations. The last segment is the leaf column name.
pub(crate) fn resolve_column_path(
    segments: &[String],
    database: &Database,
) -> Result<(TableId, PhysicalColumnPath), String> {
    if segments.len() < 2 {
        return Err(format!(
            "Column path must have at least 2 segments (table.column), got: {:?}",
            segments
        ));
    }

    let table_name = &segments[0];
    let table_id = database
        .get_table_id(&SchemaObjectName::new(table_name, None))
        .ok_or_else(|| format!("Unknown table: {table_name}"))?;

    let mut current_table_id = table_id;
    let mut links = Vec::new();

    for (i, segment) in segments[1..].iter().enumerate() {
        let is_last = i == segments.len() - 2;

        if is_last {
            let column_id = database
                .get_column_id(current_table_id, segment)
                .ok_or_else(|| {
                    let table = database.get_table(current_table_id);
                    format!("Unknown column '{}' on table '{:?}'", segment, table.name)
                })?;
            links.push(ColumnPathLink::Leaf(column_id));
        } else {
            let relation = find_mto_relation(database, current_table_id, segment)?;
            links.push(relation.column_path_link());
            current_table_id = relation.linked_table_id;
        }
    }

    let mut path = PhysicalColumnPath::init(links.remove(0));
    for link in links {
        path = path.push(link);
    }

    Ok((table_id, path))
}

/// Find a ManyToOne relation by matching the segment against the FK column name
/// on the current table. E.g., `venue_id` matches the relation whose self column is `venue_id`.
fn find_mto_relation(
    database: &Database,
    current_table_id: TableId,
    segment: &str,
) -> Result<ManyToOne, String> {
    let table = database.get_table(current_table_id);

    for relation in &database.relations {
        if relation.self_table_id != current_table_id {
            continue;
        }

        let fk_column_name = relation
            .column_pairs
            .iter()
            .map(|pair| {
                table.columns[pair.self_column_id.column_index]
                    .name
                    .as_str()
            })
            .next();

        if fk_column_name == Some(segment) {
            return Ok(relation.clone());
        }
    }

    Err(format!(
        "No relation '{segment}' found on table '{:?}'",
        table.name
    ))
}

pub(crate) fn resolve_column_id(
    table_id: TableId,
    column_name: &str,
    database: &Database,
) -> Result<ColumnId, String> {
    database
        .get_column_id(table_id, column_name)
        .ok_or_else(|| {
            let table = database.get_table(table_id);
            format!(
                "Unknown column '{}' on table '{:?}'",
                column_name, table.name
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_sql_pg::test_database_builder::*;

    fn test_db() -> Database {
        DatabaseBuilder::new()
            .table("venues", vec![pk("id"), string("name")])
            .table(
                "concerts",
                vec![
                    pk("id"),
                    string("name"),
                    fk("venue_id", "venues", "id", "venue_fk"),
                ],
            )
            .build()
    }

    #[test]
    fn test_resolve_simple_column() {
        let db = test_db();

        let segments: Vec<String> = ["concerts", "id"].iter().map(|s| s.to_string()).collect();
        let (table_id, path) = resolve_column_path(&segments, &db).unwrap();

        let table = db.get_table(table_id);
        assert_eq!(table.name.name, "concerts");
        assert_eq!(path.leaf_column().get_column(&db).name, "id");
    }

    #[test]
    fn test_resolve_nested_relation() {
        let db = test_db();

        let segments: Vec<String> = ["concerts", "venue_id", "name"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (table_id, path) = resolve_column_path(&segments, &db).unwrap();

        let table = db.get_table(table_id);
        assert_eq!(table.name.name, "concerts");
        assert_eq!(path.leaf_column().get_column(&db).name, "name");
        assert_eq!(
            db.get_table(path.leaf_column().table_id).name.name,
            "venues"
        );
    }
}
