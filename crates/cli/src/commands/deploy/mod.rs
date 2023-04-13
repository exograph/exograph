use super::command::SubcommandDefinition;

mod aws_lambda;
mod fly;

pub fn command_definition() -> SubcommandDefinition {
    SubcommandDefinition::new(
        "deploy",
        "Deploy your Exograph project",
        vec![
            Box::new(fly::FlyCommandDefinition {}),
            Box::new(aws_lambda::AwsLambdaCommandDefinition {}),
        ],
    )
}
