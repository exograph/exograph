use super::command::SubcommandDefinition;

mod fly;

pub fn command_definition() -> SubcommandDefinition {
    SubcommandDefinition::new(
        "deploy",
        "Deploy your Exograph project",
        vec![Box::new(fly::FlyCommandDefinition {})],
    )
}
