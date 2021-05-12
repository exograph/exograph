//#[cfg(test)] // needed until we use this code from `main`
pub mod common_test_data {
    use payas_model::model::system::ModelSystem;
    use payas_parser::builder::system_builder;

    use crate::parser;

    pub fn test_system() -> ModelSystem {
        system_builder::build(parser::parse())
    }
}
