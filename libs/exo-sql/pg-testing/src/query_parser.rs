use exo_sql_pg::{
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, ColumnPath, Database, Limit,
    Offset, Ordering, PgExtension, PhysicalColumnPath, RelationId, SQLParamContainer,
    SchemaObjectName, Selection, SelectionCardinality, SelectionElement, TableId,
    get_otm_relation_for_columns,
};

use sqlparser::ast::{self, Expr, OrderByKind, SelectItem, SetExpr, Statement, Value};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use crate::column_path_resolver::{resolve_column_id, resolve_column_path};

/// Parse an abstract SQL statement into an AbstractSelect<PgExtension>.
///
/// Supported syntax:
///   select <columns> from <table> [where <predicate>] [order by <ordering>] [limit <n>] [offset <n>]
///
/// Column paths use dot-notation: `concerts.id`, `concerts.venue_id.name`
pub(crate) fn parse_query(
    sql: &str,
    params: &[serde_json::Value],
    database: &Database,
) -> Result<AbstractSelect<PgExtension>, String> {
    let dialect = GenericDialect {};
    let statements =
        Parser::parse_sql(&dialect, sql).map_err(|e| format!("SQL parse error: {e}"))?;

    let stmt = statements.into_iter().next().ok_or("Empty SQL statement")?;

    match stmt {
        Statement::Query(query) => parse_select_query(*query, params, database),
        other => Err(format!("Expected SELECT query, got: {other}")),
    }
}

fn parse_select_query(
    query: ast::Query,
    params: &[serde_json::Value],
    database: &Database,
) -> Result<AbstractSelect<PgExtension>, String> {
    let body = match *query.body {
        SetExpr::Select(select) => *select,
        other => return Err(format!("Expected SELECT, got: {other}")),
    };

    let table_id = resolve_root_table(&body.from, database)?;
    let selection = parse_selection(&body.projection, table_id, database)?;

    let predicate = match body.selection {
        Some(expr) => parse_predicate(&expr, table_id, params, database)?,
        None => AbstractPredicate::True,
    };

    let order_by = match query.order_by {
        Some(order_by) => match order_by.kind {
            OrderByKind::Expressions(exprs) => Some(parse_order_by(&exprs, table_id, database)?),
            OrderByKind::All(_) => return Err("ORDER BY ALL not supported".to_string()),
        },
        None => None,
    };

    let (limit, offset) = match query.limit_clause {
        Some(ast::LimitClause::LimitOffset {
            limit: limit_expr,
            offset: offset_expr,
            ..
        }) => {
            let limit = limit_expr
                .map(|expr| parse_i64_expr(&expr, "LIMIT"))
                .transpose()?
                .map(Limit);

            let offset = offset_expr
                .map(|o| parse_i64_expr(&o.value, "OFFSET"))
                .transpose()?
                .map(Offset);

            (limit, offset)
        }
        Some(_) => return Err("Unsupported LIMIT clause syntax".to_string()),
        None => (None, None),
    };

    Ok(AbstractSelect {
        table_id,
        selection,
        predicate,
        order_by,
        offset,
        limit,
    })
}

fn parse_i64_expr(expr: &Expr, context: &str) -> Result<i64, String> {
    match expr {
        Expr::Value(ast::ValueWithSpan {
            value: Value::Number(n, _),
            ..
        }) => n
            .parse::<i64>()
            .map_err(|e| format!("Invalid {context}: {e}")),
        other => Err(format!("Expected numeric {context}, got: {other}")),
    }
}

fn resolve_root_table(
    from: &[ast::TableWithJoins],
    database: &Database,
) -> Result<TableId, String> {
    let table_ref = from.first().ok_or("Missing FROM clause")?;

    match &table_ref.relation {
        ast::TableFactor::Table { name, .. } => {
            let table_name = name.to_string();
            database
                .get_table_id(&SchemaObjectName::new(&table_name, None))
                .ok_or_else(|| format!("Unknown table in FROM: {table_name}"))
        }
        other => Err(format!("Unsupported FROM clause: {other}")),
    }
}

fn parse_selection(
    projection: &[SelectItem],
    root_table_id: TableId,
    database: &Database,
) -> Result<Selection<PgExtension>, String> {
    let has_json_fn = projection.iter().any(|item| {
        let expr = match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => e,
            _ => return false,
        };
        is_json_function(expr)
    });

    let mut elements = Vec::new();

    for item in projection {
        match item {
            SelectItem::UnnamedExpr(expr) => {
                let (alias, element) = parse_selection_expr(expr, root_table_id, database)?;
                elements.push(AliasedSelectionElement::new(alias, element));
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                let (_, element) = parse_selection_expr(expr, root_table_id, database)?;
                elements.push(AliasedSelectionElement::new(alias.value.clone(), element));
            }
            other => return Err(format!("Unsupported SELECT item: {other}")),
        }
    }

    if has_json_fn {
        Ok(Selection::Json(elements, SelectionCardinality::Many))
    } else {
        Ok(Selection::Seq(elements))
    }
}

fn is_json_function(expr: &Expr) -> bool {
    matches!(expr, Expr::Function(f) if {
        let name = f.name.to_string().to_lowercase();
        name == "json_object" || name == "json_agg"
    })
}

/// Resolve an identifier expression to a (name, PhysicalColumnPath).
/// Works for both `CompoundIdentifier` (table.column) and plain `Identifier`.
fn resolve_expr_to_physical_path(
    expr: &Expr,
    root_table_id: TableId,
    database: &Database,
) -> Result<(String, PhysicalColumnPath), String> {
    match expr {
        Expr::CompoundIdentifier(parts) => {
            let segments: Vec<String> = parts.iter().map(|p| p.value.clone()).collect();
            let (_, column_path) = resolve_column_path(&segments, database)?;
            let alias = segments[1..].join(".");
            Ok((alias, column_path))
        }
        Expr::Identifier(ident) => {
            let column_id = resolve_column_id(root_table_id, &ident.value, database)?;
            Ok((ident.value.clone(), PhysicalColumnPath::leaf(column_id)))
        }
        other => Err(format!("Unsupported column reference: {other}")),
    }
}

fn parse_selection_expr(
    expr: &Expr,
    root_table_id: TableId,
    database: &Database,
) -> Result<(String, SelectionElement<PgExtension>), String> {
    match expr {
        Expr::Function(f) if is_json_function(expr) => parse_json_function_expr(f, database),
        Expr::Value(ast::ValueWithSpan { value, .. }) => match value {
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                Ok((s.clone(), SelectionElement::Constant(s.clone())))
            }
            other => Err(format!("Unsupported literal in SELECT: {other}")),
        },
        _ => {
            let (alias, path) = resolve_expr_to_physical_path(expr, root_table_id, database)?;
            Ok((alias, SelectionElement::Physical(path.leaf_column())))
        }
    }
}

/// Parse `json_object(table.rel.col1, table.rel.col2)` or `json_agg(table.rel.col1, table.rel.col2)`
/// into a SubSelect with Json selection on the related table.
fn parse_json_function_expr(
    func: &ast::Function,
    database: &Database,
) -> Result<(String, SelectionElement<PgExtension>), String> {
    let func_name = func.name.to_string().to_lowercase();
    let cardinality = if func_name == "json_agg" {
        SelectionCardinality::Many
    } else {
        SelectionCardinality::One
    };

    let args: Vec<&Expr> = match &func.args {
        ast::FunctionArguments::List(arg_list) => arg_list
            .args
            .iter()
            .map(|a| match a {
                ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(e)) => Ok(e),
                other => Err(format!("Unsupported function argument: {other}")),
            })
            .collect::<Result<_, _>>()?,
        other => return Err(format!("Unsupported function arguments: {other}")),
    };

    if args.is_empty() {
        return Err("json_object/json_agg requires at least one argument".to_string());
    }

    // All arguments must be cross-table column paths through the same relation.
    // E.g., json_object(concerts.venues.id, concerts.venues.name) — all go through "venues".
    let mut relation_table_id = None;
    let mut fk_column_ids = None;
    let mut alias = None;
    let mut leaf_elements = Vec::new();

    for arg in &args {
        let segments = match arg {
            Expr::CompoundIdentifier(parts) => {
                parts.iter().map(|p| p.value.clone()).collect::<Vec<_>>()
            }
            other => {
                return Err(format!(
                    "json_object/json_agg arguments must be column paths, got: {other}"
                ));
            }
        };

        if segments.len() < 3 {
            return Err(format!(
                "json_object/json_agg arguments must be cross-table paths (table.relation.column), got: {}",
                segments.join(".")
            ));
        }

        let (_, column_path) = resolve_column_path(&segments, database)?;
        let leaf_col = column_path.leaf_column();
        let leaf_name = segments.last().unwrap().clone();
        let this_relation_table = leaf_col.table_id;

        // Extract FK column IDs from the first link in the path (the relation link)
        let (head_link, _) = column_path.split_head();
        let this_fk_ids = head_link.self_column_ids();

        match relation_table_id {
            None => {
                relation_table_id = Some(this_relation_table);
                fk_column_ids = Some(this_fk_ids);
                alias = Some(segments[1].clone());
            }
            Some(existing) if existing != this_relation_table => {
                return Err(
                    "All json_object/json_agg arguments must reference the same relation"
                        .to_string(),
                );
            }
            _ => {}
        }

        leaf_elements.push(AliasedSelectionElement::new(
            leaf_name,
            SelectionElement::Physical(leaf_col),
        ));
    }

    let fk_column_ids = fk_column_ids.unwrap();
    let relation_table_id = relation_table_id.unwrap();
    let alias = alias.unwrap();

    let relation_id = RelationId::OneToMany(
        get_otm_relation_for_columns(&fk_column_ids, database)
            .ok_or("Failed to find relation for FK columns")?,
    );

    let sub_select = AbstractSelect {
        table_id: relation_table_id,
        selection: Selection::Json(leaf_elements, cardinality),
        predicate: AbstractPredicate::True,
        order_by: None,
        offset: None,
        limit: None,
    };

    Ok((
        alias,
        SelectionElement::SubSelect(relation_id, Box::new(sub_select)),
    ))
}

fn parse_predicate(
    expr: &Expr,
    root_table_id: TableId,
    params: &[serde_json::Value],
    database: &Database,
) -> Result<AbstractPredicate<PgExtension>, String> {
    match expr {
        Expr::BinaryOp { left, op, right } => match op {
            ast::BinaryOperator::And => {
                let left = parse_predicate(left, root_table_id, params, database)?;
                let right = parse_predicate(right, root_table_id, params, database)?;
                Ok(AbstractPredicate::And(Box::new(left), Box::new(right)))
            }
            ast::BinaryOperator::Or => {
                let left = parse_predicate(left, root_table_id, params, database)?;
                let right = parse_predicate(right, root_table_id, params, database)?;
                Ok(AbstractPredicate::Or(Box::new(left), Box::new(right)))
            }
            _ => {
                let left_path = parse_column_path_expr(left, root_table_id, params, database)?;
                let right_path = parse_column_path_expr(right, root_table_id, params, database)?;

                match op {
                    ast::BinaryOperator::Eq => Ok(AbstractPredicate::Eq(left_path, right_path)),
                    ast::BinaryOperator::NotEq => Ok(AbstractPredicate::Neq(left_path, right_path)),
                    ast::BinaryOperator::Lt => Ok(AbstractPredicate::Lt(left_path, right_path)),
                    ast::BinaryOperator::LtEq => Ok(AbstractPredicate::Lte(left_path, right_path)),
                    ast::BinaryOperator::Gt => Ok(AbstractPredicate::Gt(left_path, right_path)),
                    ast::BinaryOperator::GtEq => Ok(AbstractPredicate::Gte(left_path, right_path)),
                    other => Err(format!("Unsupported operator: {other}")),
                }
            }
        },
        Expr::UnaryOp {
            op: ast::UnaryOperator::Not,
            expr,
        } => {
            let inner = parse_predicate(expr, root_table_id, params, database)?;
            Ok(AbstractPredicate::Not(Box::new(inner)))
        }
        Expr::Nested(inner) => parse_predicate(inner, root_table_id, params, database),
        other => Err(format!("Unsupported predicate expression: {other}")),
    }
}

fn parse_column_path_expr(
    expr: &Expr,
    root_table_id: TableId,
    params: &[serde_json::Value],
    database: &Database,
) -> Result<ColumnPath<PgExtension>, String> {
    match expr {
        Expr::CompoundIdentifier(_) | Expr::Identifier(_) => {
            let (_, path) = resolve_expr_to_physical_path(expr, root_table_id, database)?;
            Ok(ColumnPath::Physical(path))
        }
        Expr::Value(ast::ValueWithSpan { value, .. }) => match value {
            Value::Number(n, _) => parse_number_param(n),
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                Ok(ColumnPath::Param(SQLParamContainer::string(s.clone())))
            }
            Value::Boolean(b) => Ok(ColumnPath::Param(SQLParamContainer::bool(*b))),
            Value::Null => Ok(ColumnPath::Null),
            Value::Placeholder(p) => {
                let idx = parse_placeholder_index(p)?;
                if idx == 0 || idx > params.len() {
                    return Err(format!(
                        "Parameter ${idx} out of range (have {} params)",
                        params.len()
                    ));
                }
                let value = &params[idx - 1];
                json_value_to_column_path(value)
            }
            other => Err(format!("Unsupported value: {other}")),
        },
        Expr::UnaryOp {
            op: ast::UnaryOperator::Minus,
            expr,
        } => {
            if let Expr::Value(ast::ValueWithSpan {
                value: Value::Number(n, _),
                ..
            }) = expr.as_ref()
            {
                parse_number_param(&format!("-{n}"))
            } else {
                Err(format!("Unsupported unary expression: -{expr}"))
            }
        }
        other => Err(format!("Unsupported expression in predicate: {other}")),
    }
}

fn parse_number_param(n: &str) -> Result<ColumnPath<PgExtension>, String> {
    if let Ok(i) = n.parse::<i64>() {
        if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
            Ok(ColumnPath::Param(SQLParamContainer::i32(i as i32)))
        } else {
            Ok(ColumnPath::Param(SQLParamContainer::i64(i)))
        }
    } else if let Ok(f) = n.parse::<f64>() {
        Ok(ColumnPath::Param(SQLParamContainer::f64(f)))
    } else {
        Err(format!("Unsupported number: {n}"))
    }
}

fn parse_placeholder_index(placeholder: &str) -> Result<usize, String> {
    if let Some(index_str) = placeholder.strip_prefix('$') {
        index_str
            .parse::<usize>()
            .map_err(|e| format!("Invalid placeholder '{placeholder}': {e}"))
    } else {
        Err(format!("Expected $N placeholder, got: {placeholder}"))
    }
}

fn json_value_to_column_path(value: &serde_json::Value) -> Result<ColumnPath<PgExtension>, String> {
    match value {
        serde_json::Value::String(s) => Ok(ColumnPath::Param(SQLParamContainer::string(s.clone()))),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(ColumnPath::Param(SQLParamContainer::i32(i as i32)))
            } else if let Some(f) = n.as_f64() {
                Ok(ColumnPath::Param(SQLParamContainer::f64(f)))
            } else {
                Err(format!("Unsupported number param: {n}"))
            }
        }
        serde_json::Value::Bool(b) => Ok(ColumnPath::Param(SQLParamContainer::bool(*b))),
        other => Err(format!("Unsupported param value: {other}")),
    }
}

fn parse_order_by(
    order_by_exprs: &[ast::OrderByExpr],
    root_table_id: TableId,
    database: &Database,
) -> Result<exo_sql_pg::AbstractOrderBy<PgExtension>, String> {
    use exo_sql_model::order_by::AbstractOrderByExpr;

    let mut elements = Vec::new();

    for expr in order_by_exprs {
        let (_, physical_path) =
            resolve_expr_to_physical_path(&expr.expr, root_table_id, database)?;

        let ordering = match expr.options.asc {
            Some(true) | None => Ordering::Asc,
            Some(false) => Ordering::Desc,
        };

        elements.push((AbstractOrderByExpr::Column(physical_path), ordering));
    }

    Ok(exo_sql_pg::AbstractOrderBy(elements))
}
