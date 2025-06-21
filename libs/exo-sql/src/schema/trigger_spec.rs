use std::{str::FromStr, vec};

use crate::{
    SchemaObjectName, database_error::DatabaseError, sql::connect::database_client::DatabaseClient,
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

#[derive(Debug, Clone)]
pub struct TriggerSpec {
    pub name: String,
    pub function: String,
    pub timing: TriggerTiming,
    pub orientation: TriggerOrientation,
    pub event: TriggerEvent,
}

impl TriggerSpec {
    pub fn new(
        name: String,
        function: String,
        timing: TriggerTiming,
        orientation: TriggerOrientation,
        event: TriggerEvent,
    ) -> Self {
        Self {
            name,
            function,
            timing,
            orientation,
            event,
        }
    }

    pub fn debug_print(&self, indent: usize) {
        let indent_str = " ".repeat(indent);
        let trigger_type = format!(
            "{} {} {}",
            self.timing.as_str(),
            self.event.as_str(),
            self.orientation.as_str()
        );
        println!(
            "{}- ({}, {}, function: {})",
            indent_str, self.name, trigger_type, self.function
        );
    }

    pub async fn from_live_db(
        client: &DatabaseClient,
        table_name: &SchemaObjectName,
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

                Ok(TriggerSpec::new(
                    name,
                    body,
                    timing_string.parse()?,
                    orientation_string.parse()?,
                    event_string.parse()?,
                ))
            })
            .collect::<Result<Vec<_>, DatabaseError>>()?;

        Ok(WithIssues {
            value: triggers,
            issues: vec![],
        })
    }

    pub fn diff<'a>(
        &'a self,
        new: &'a Self,
        table_name: &'a SchemaObjectName,
    ) -> Vec<SchemaOp<'a>> {
        if (self.function != new.function)
            || (self.timing != new.timing)
            || (self.orientation != new.orientation)
            || (self.event != new.event)
        {
            vec![
                SchemaOp::DeleteTrigger {
                    trigger: self,
                    table_name,
                },
                SchemaOp::CreateTrigger {
                    trigger: new,
                    table_name,
                },
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
    pub fn creation_sql(&self, table_name: &SchemaObjectName) -> String {
        format!(
            "CREATE TRIGGER {trigger_name} {timing} {event} ON {table_name} FOR EACH {orientation} EXECUTE FUNCTION {function}();",
            trigger_name = self.name,
            timing = self.timing.as_str(),
            event = self.event.as_str(),
            orientation = self.orientation.as_str(),
            table_name = table_name.fully_qualified_name(),
            function = self.function
        )
    }
}
