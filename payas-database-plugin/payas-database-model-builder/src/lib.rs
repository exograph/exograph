// TODO: We should not need to expose `builder`, but see payas-parser::typechecker::field_default_value
//       In general, we need to consider how we typecheck subsystem specific annotations
pub mod builder;

pub use builder::system_builder::build;
