use std::fmt::{self, Display, Formatter};

/// An SQL statement along with any foreign constraint statements that should follow after all the
/// statements have been executed.
#[derive(Default)]
pub struct SchemaStatement {
    pub statement: String,
    // foreign constraint statements that need to be executed before this statement. For example, when deleting a table,
    // foreign constraint statements need to be executed before the table is deleted.
    pub pre_statements: Vec<String>,
    // foreign constraint statements that need to be executed after this statement. For example, when creating a table,
    // foreign constraint statements need to be executed after the table is created.
    pub post_statements: Vec<String>,
}

impl Display for SchemaStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{}\n{}",
            self.pre_statements.join("\n"),
            self.statement,
            self.post_statements.join("\n")
        )
    }
}
