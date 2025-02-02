use crate::{
    database_error::DatabaseError,
    sql::transaction::{TransactionScript, TransactionStep},
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, ColumnPath, Database, Selection,
    SelectionElement, TableId,
};

use crate::transform::transformer::SelectTransformer;

pub fn add_precheck_queries(
    precheck_predicates: Vec<AbstractPredicate>,
    database: &Database,
    transformer: &impl SelectTransformer,
    transaction_script: &mut TransactionScript,
) {
    let precheck_queries = compute_precheck_queries(precheck_predicates).unwrap();

    for precheck_query in precheck_queries {
        let precheck_select = transformer.to_select(precheck_query, database);
        transaction_script.add_step(TransactionStep::Precheck(precheck_select));
    }
}

fn compute_precheck_queries(
    predicates: Vec<AbstractPredicate>,
) -> Result<Vec<AbstractSelect>, DatabaseError> {
    let precheck_queries: Result<Vec<_>, _> =
        predicates.into_iter().map(compute_precheck_query).collect();

    let precheck_queries = precheck_queries?;

    Ok(precheck_queries.into_iter().flatten().collect())
}

fn compute_precheck_query(
    predicate: AbstractPredicate,
) -> Result<Option<AbstractSelect>, DatabaseError> {
    if predicate == AbstractPredicate::True {
        return Ok(None);
    }

    fn get_lead_table_ids(column_path: &ColumnPath) -> Vec<TableId> {
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
            ))
        }
    };

    Ok(Some(AbstractSelect {
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
