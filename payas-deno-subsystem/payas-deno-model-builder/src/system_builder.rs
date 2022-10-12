use std::{
    io::Write,
    path::{Path, PathBuf},
};

use payas_core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_core_model_builder::{
    ast::ast_types::{AstAnnotationParams, AstExpr, AstService},
    builder::{resolved_builder::AnnotationMapHelper, system_builder::BaseModelSystem},
    error::ModelBuildingError,
    typechecker::{typ::Type, Typed},
};
use payas_deno_model::{
    interceptor::Interceptor,
    model::ModelDenoSystem,
    operation::{DenoMutation, DenoQuery},
};

use crate::service_skeleton_generator;

pub struct ModelDenoSystemWithInterceptors {
    pub underlying: ModelDenoSystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

pub fn build(
    typechecked_system: &MappedArena<Type>,
    base_system: &BaseModelSystem,
) -> Option<Result<ModelDenoSystemWithInterceptors, ModelBuildingError>> {
    let service_selection_predicate = |service: &AstService<Typed>| {
        let module_path = match service.annotations.get("external").unwrap() {
            AstAnnotationParams::Single(AstExpr::StringLiteral(s, _), _) => s,
            _ => panic!(),
        }
        .clone();

        let extension = Path::new(&module_path).extension().and_then(|e| e.to_str());

        extension == Some("ts") || extension == Some("js")
    };

    let service_system = payas_subsystem_model_builder_util::build_with_selection(
        typechecked_system,
        base_system,
        service_selection_predicate,
        process_script,
    );

    match service_system {
        Ok(service_system) => {
            let underlying_service_system = service_system.underlying;

            if underlying_service_system.queries.is_empty()
                && underlying_service_system.mutations.is_empty()
                && underlying_service_system.interceptors.is_empty()
            {
                return None;
            }

            let mut queries = MappedArena::default();
            for query in underlying_service_system.queries.values.into_iter() {
                queries.add(&query.name.clone(), DenoQuery(query));
            }

            let mut mutations = MappedArena::default();
            for mutation in underlying_service_system.mutations.values.into_iter() {
                mutations.add(&mutation.name.clone(), DenoMutation(mutation));
            }

            Some(Ok(ModelDenoSystemWithInterceptors {
                underlying: ModelDenoSystem {
                    contexts: underlying_service_system.contexts,
                    service_types: underlying_service_system.service_types,
                    queries,
                    mutations,
                    methods: underlying_service_system.methods,
                    scripts: underlying_service_system.scripts,
                    interceptors: underlying_service_system.interceptors,
                },
                interceptors: service_system.interceptors,
            }))
        }
        Err(e) => Some(Err(e)),
    }
}

fn process_script(
    service: &AstService<Typed>,
    module_fs_path: &PathBuf,
) -> Result<Vec<u8>, ModelBuildingError> {
    service_skeleton_generator::generate_service_skeleton(service, &module_fs_path)?;

    // Bundle js/ts files using Deno; we need to bundle even the js files since they may import ts files
    let bundler_output = std::process::Command::new("deno")
        .args(["bundle", "--no-check", module_fs_path.to_str().unwrap()])
        .output()
        .map_err(|err| {
            ModelBuildingError::Generic(format!(
                "While trying to invoke `deno` in order to bundle .ts files: {}",
                err
            ))
        })?;

    if bundler_output.status.success() {
        Ok(bundler_output.stdout)
    } else {
        std::io::stdout().write_all(&bundler_output.stderr).unwrap();
        Err(ModelBuildingError::Generic(
            "Deno bundler did not exit successfully".to_string(),
        ))
    }
}
