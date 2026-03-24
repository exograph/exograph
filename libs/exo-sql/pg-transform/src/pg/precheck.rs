use exo_sql_core::{Database, DatabaseError, TableId};
use exo_sql_model::{
    AbstractPredicateExt, AliasedSelectionElement, ColumnPath, Selection, SelectionElement,
};
use exo_sql_pg_core::{
    PgAbstractPredicate, PgAbstractSelect, PgColumnPath, PgExtension,
    transaction::{TransactionScript, TransactionStep},
};

use exo_sql_model::transformer::SelectTransformer;

pub fn add_precheck_queries(
    precheck_predicates: Vec<PgAbstractPredicate>,
    database: &Database,
    transformer: &impl SelectTransformer<PgExtension>,
    transaction_script: &mut TransactionScript,
) {
    let precheck_queries = compute_precheck_queries(precheck_predicates).unwrap();

    for precheck_query in precheck_queries {
        let precheck_select = transformer.to_select(precheck_query, database);
        transaction_script.add_step(TransactionStep::Precheck(precheck_select));
    }
}

fn compute_precheck_queries(
    predicates: Vec<PgAbstractPredicate>,
) -> Result<Vec<PgAbstractSelect>, DatabaseError> {
    let precheck_queries: Result<Vec<_>, _> =
        predicates.into_iter().map(compute_precheck_query).collect();

    let precheck_queries = precheck_queries?;

    Ok(precheck_queries.into_iter().flatten().collect())
}

fn compute_precheck_query(
    predicate: PgAbstractPredicate,
) -> Result<Option<PgAbstractSelect>, DatabaseError> {
    if predicate == PgAbstractPredicate::True {
        return Ok(None);
    }

    fn get_lead_table_ids(column_path: &PgColumnPath) -> Vec<TableId> {
        match column_path {
            ColumnPath::Physical(physical_path) => vec![physical_path.lead_table_id()],
            ColumnPath::Predicate(predicate) => predicate
                .column_paths()
                .iter()
                .flat_map(|path| get_lead_table_ids(path))
                .collect(),
            ColumnPath::Param(_) | ColumnPath::Null => vec![],
        }
    }

    let lead_table_ids: Vec<_> = predicate
        .column_paths()
        .iter()
        .flat_map(|path| get_lead_table_ids(path))
        .collect();

    let lead_table_id = match &lead_table_ids[..] {
        [table_id] => table_id,
        [lead_table_id, rest @ ..] => {
            if rest.iter().all(|table_id| table_id == lead_table_id) {
                lead_table_id
            } else {
                return Err(DatabaseError::Precheck(
                    "Access predicates with multiple lead table ids are not supported".to_string(),
                ));
            }
        }
        [] => {
            return Err(DatabaseError::Precheck(
                "Access predicates with no lead table ids are not supported".to_string(),
            ));
        }
    };

    Ok(Some(PgAbstractSelect {
        table_id: *lead_table_id,
        selection: Selection::Seq(vec![AliasedSelectionElement::new(
            "access_predicate".to_string(),
            SelectionElement::Constant("true".to_string()),
        )]),
        predicate: predicate.clone(),
        order_by: None,
        offset: None,
        limit: None,
    }))
}
