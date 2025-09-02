// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::command::SubcommandDefinition;

mod aws_lambda;
mod cf_worker;
mod fly;
mod railway;
mod util;

pub fn command_definition() -> SubcommandDefinition {
    SubcommandDefinition::new(
        "deploy",
        "Deploy your Exograph project",
        vec![
            Box::new(fly::FlyCommandDefinition {}),
            Box::new(railway::RailwayCommandDefinition {}),
            Box::new(aws_lambda::AwsLambdaCommandDefinition {}),
            Box::new(cf_worker::CfWorkerCommandDefinition {}),
        ],
    )
}
