use std::{str::FromStr, vec};

use crate::{
    database_error::DatabaseError, sql::connect::database_client::DatabaseClient, PhysicalTableName,
};

use super::{issue::WithIssues, op::SchemaOp};

const TRIGGERS_QUERY: &str = r#"
    SELECT trigger_name, event_manipulation, event_object_schema, event_object_table, action_condition, action_orientation, action_timing, action_statement 
    FROM information_schema.triggers 
    WHERE trigger_name LIKE 'exograph%' AND event_object_table = $1
"#;

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
}

impl TriggerTiming {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Before => "BEFORE",
            Self::After => "AFTER",
            Self::InsteadOf => "INSTEAD OF",
        }
    }
}

impl FromStr for TriggerTiming {
    type Err = DatabaseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BEFORE" => Ok(Self::Before),
            "AFTER" => Ok(Self::After),
            "INSTEAD OF" => Ok(Self::InsteadOf),
            _ => Err(DatabaseError::Generic(format!(
                "Invalid trigger timing: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerOrientation {
    Row,
    Statement,
}

impl TriggerOrientation {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Row => "ROW",
            Self::Statement => "STATEMENT",
        }
    }
}

impl FromStr for TriggerOrientation {
    type Err = DatabaseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ROW" => Ok(Self::Row),
            "STATEMENT" => Ok(Self::Statement),
            _ => Err(DatabaseError::Generic(format!(
                "Invalid trigger orientation: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
    Truncate,
}

impl TriggerEvent {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
        }
    }
}

impl FromStr for TriggerEvent {
    type Err = DatabaseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "INSERT" => Ok(Self::Insert),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            "TRUNCATE" => Ok(Self::Truncate),
            _ => Err(DatabaseError::Generic(format!(
                "Invalid trigger event: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriggerSpec {
    pub name: String,
    pub function: String,
    pub timing: TriggerTiming,
    pub orientation: TriggerOrientation,
    pub event: TriggerEvent,
    pub table: PhysicalTableName,
}

impl TriggerSpec {
    pub fn new(
        name: String,
        function: String,
        timing: TriggerTiming,
        orientation: TriggerOrientation,
        event: TriggerEvent,
        table: PhysicalTableName,
    ) -> Self {
        Self {
            name,
            function,
            timing,
            orientation,
            event,
            table,
        }
    }

    pub async fn from_live_db(
        client: &DatabaseClient,
        table_name: &PhysicalTableName,
    ) -> Result<WithIssues<Vec<TriggerSpec>>, DatabaseError> {
        let triggers = client
            .query(TRIGGERS_QUERY, &[&table_name.fully_qualified_name()])
            .await?
            .iter()
            .map(|row| {
                let name = row.get("trigger_name");
                let body = row.get("action_statement");
                let timing_string: String = row.get("action_timing"); // BEFORE, AFTER, INSTEAD OF
                let orientation_string: String = row.get("action_orientation"); // ROW, STATEMENT
                let event_string: String = row.get("event_manipulation"); // INSERT, UPDATE, DELETE, TRUNCATE
                let table_schema: String = row.get("event_object_schema");
                let table_name: String = row.get("event_object_table");

                Ok(TriggerSpec::new(
                    name,
                    body,
                    timing_string.parse()?,
                    orientation_string.parse()?,
                    event_string.parse()?,
                    PhysicalTableName::new(table_name, Some(&table_schema)),
                ))
            })
            .collect::<Result<Vec<_>, DatabaseError>>()?;

        Ok(WithIssues {
            value: triggers,
            issues: vec![],
        })
    }

    pub fn diff<'a>(&'a self, new: &'a Self) -> Vec<SchemaOp<'a>> {
        if (self.function != new.function)
            || (self.timing != new.timing)
            || (self.orientation != new.orientation)
            || (self.event != new.event)
        {
            vec![
                SchemaOp::DeleteTrigger { trigger: self },
                SchemaOp::CreateTrigger { trigger: new },
            ]
        } else {
            vec![]
        }
    }

    /// Create a trigger in the database of the form:
    /// CREATE TRIGGER exograph_on_update_todo
    //     BEFORE UPDATE
    //     ON todos
    //     FOR EACH ROW EXECUTE FUNCTION exograph_update_todo();
    pub fn creation_sql(&self) -> String {
        format!(
            "CREATE TRIGGER {trigger_name} {timing} {event} ON {table_name} FOR EACH {orientation} EXECUTE FUNCTION {function}();",
            trigger_name = self.name,
            timing = self.timing.as_str(),
            event = self.event.as_str(),
            orientation = self.orientation.as_str(),
            table_name = self.table.name,
            function = self.function
        )
    }
}
