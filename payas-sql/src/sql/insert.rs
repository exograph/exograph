use super::{
    column::{Column, PhysicalColumn},
    Expression, ExpressionContext, ParameterBinding, PhysicalTable,
};

pub struct Insert<'a> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub column_values_seq: Vec<Vec<&'a Column<'a>>>,
    pub returning: Vec<&'a Column<'a>>,
}

// INSERT INTO "concert_artists" ("rank", "role", "artist_id", "concert_id") VALUES ($1, $2, $3, $4), ($1, $2, $3, $5)
// INSERT INTO "concert_artists" ("rank", "role", "artist_id", "concert_id") VALUES ($1, $2, $3, (select id from concerts where .. limit 1 offset 0)), ($1, $2, $3, (select id from concerts where .. limit 1 offset 1))

impl<'a> Expression for Insert<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        let table_binding = self.table.binding(expression_context);

        let (column_statements, col_params): (Vec<_>, Vec<_>) =
            expression_context.with_plain(|expression_context| {
                self.column_names
                    .iter()
                    .map(|column_names| column_names.binding(expression_context).tupled())
                    .unzip()
            });

        let (value_statements, value_params): (Vec<Vec<_>>, Vec<Vec<_>>) = self
            .column_values_seq
            .iter()
            .map(|column_values| {
                column_values
                    .iter()
                    .map(|value| value.binding(expression_context).tupled())
                    .unzip()
            })
            .unzip();

        let stmt = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table_binding.stmt,
            column_statements.join(", "),
            value_statements
                .iter()
                .map(|v| format!("({})", v.join(", ")))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut params = table_binding.params;
        params.extend(col_params.into_iter().flatten());
        params.extend(value_params.into_iter().flatten().into_iter().flatten());

        if self.returning.is_empty() {
            ParameterBinding { stmt, params }
        } else {
            let (ret_stmts, ret_params): (Vec<_>, Vec<_>) = self
                .returning
                .iter()
                .map(|ret| ret.binding(expression_context).tupled())
                .unzip();

            let stmt = format!("{} RETURNING {}", stmt, ret_stmts.join(", "));
            params.extend(ret_params.into_iter().flatten());

            ParameterBinding { stmt, params }
        }
    }
}

pub struct DynamicInsert<'a, T> {
    pub table: &'a PhysicalTable,
    pub column_names: Vec<&'a PhysicalColumn>,
    pub eager_values: Vec<Vec<&'a Column<'a>>>, // ($1, $2, $3)
    pub dynamic_values: fn(T) -> Vec<Vec<&'a Column<'a>>>, // fn (array[id]) -> ($4, $5, ...)
    pub returning: Vec<&'a Column<'a>>,
}

impl<'a, T> DynamicInsert<'a, T> {
    pub fn resolve(self, value: T) -> Insert<'a> {
        let resolved_values = (self.dynamic_values)(value);

        let column_values_seq = resolved_values
            .into_iter()
            .flat_map(|resolved_value| {
                self.eager_values
                    .clone()
                    .into_iter()
                    .map(move |mut eager_value| {
                        let resolved_value = resolved_value.clone();
                        eager_value.extend(resolved_value);
                        eager_value
                    })
            })
            .collect();

        Insert {
            table: self.table,
            column_names: self.column_names,
            column_values_seq,
            returning: self.returning,
        }
    }
}
