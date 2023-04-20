// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io::Write;
use std::{fs::File, path::Path};

use core_plugin_interface::core_model::context_type::{ContextFieldType, ContextType};
use core_plugin_interface::core_model_builder::builder::system_builder::BaseModelSystem;
use core_plugin_interface::core_model_builder::{
    ast::ast_types::{AstArgument, AstFieldType, AstModel, AstModule},
    error::ModelBuildingError,
    typechecker::Typed,
};

// Temporary. Eventually, we will have a published artifact (at https://deno.land/x/exograph@<version>) that contains this code.
// Then, we will have this imported in each generated module code (currently, it suffices to just have it in the same directory as the module code).
static EXOTIP_D_TEMPLATE_TS: &str = include_str!("exograph.d.template.ts");

/// Generates a module skeleton based on module definitions in the exo file so that users can have a good starting point.
///
/// # Example:
/// For a module definition in a exo file as follows:
/// ```exo
/// @deno("todo.ts")
/// module TodoModule {
///     type Todo {
///       userId: Int
///       id: Int
///       title: String
///       completed: Boolean
///     }
///
///     query todo(id: Int): Todo
///   }
/// ```
///
/// The generated code will look like this:
/// ```typescript
/// interface Todo {
///     userId: number
///     id: number
///     title: string
///     completed: boolean
/// }
///
/// export /*async*/ function todo(id: number): Todo {
///     // TODO
///     throw new Error('not implemented');
/// }
/// ```
///
/// Note that we add a commented `async` to let user know that they may have an async function.
///
/// If the `@deno("todo.js") was specified, the generated code will look like this:
/// ```javascript
/// export /*async*/ function todo(id) {
///     // TODO
///     throw new Error('not implemented');
/// }
/// ```
/// We also generate a exograph.d.ts file that contains the Exograph interface.
///
pub fn generate_module_skeleton(
    module: &AstModule<Typed>,
    base_system: &BaseModelSystem,
    out_file: impl AsRef<Path>,
) -> Result<(), ModelBuildingError> {
    let is_typescript = out_file
        .as_ref()
        .extension()
        .map(|ext| ext == "ts")
        .unwrap_or(false);

    let out_file = Path::new(out_file.as_ref());

    // Generated a typescript definition file even for Javascript, so that user can know
    // the expected interface and IDEs can assist with code completion (if they use jsdoc, for).
    let exograph_d_path = out_file.parent().unwrap().join("exograph.d.ts");
    if !exograph_d_path.exists() {
        let mut exograph_d_file = File::create(&exograph_d_path)?;
        exograph_d_file.write_all(EXOTIP_D_TEMPLATE_TS.as_bytes())?;
    }

    // We don't want to overwrite any user files
    // TODO: Parse the existing file and warn if any definitions don't match the expected ones
    // along with a helpful message so that users can copy/paster the expected definitions.
    if out_file.exists() {
        return Ok(());
    }

    println!(
        "File {} does not exist, generating skeleton",
        out_file.display()
    );

    let mut file = std::fs::File::create(out_file)?;

    // Types (defined in `module`) matter only if the target is a typescript file.
    if is_typescript {
        for (_, context) in base_system.contexts.iter() {
            generate_type_skeleton(context, &mut file)?;
        }

        for module_type in module.types.iter() {
            generate_type_skeleton(module_type, &mut file)?;
        }
    }

    for method in module.methods.iter() {
        generate_method_skeleton(
            &method.name,
            &method.arguments,
            Some(&method.return_type),
            &mut file,
            is_typescript,
        )?;
    }

    for interceptor in module.interceptors.iter() {
        generate_method_skeleton(
            &interceptor.name,
            &interceptor.arguments,
            None,
            &mut file,
            is_typescript,
        )?;
    }

    Ok(())
}

fn generate_type_skeleton(model: &dyn Type, out_file: &mut File) -> Result<(), ModelBuildingError> {
    out_file.write_all(format!("interface {} {{\n", model.name()).as_bytes())?;

    for (name, typ) in model.fields() {
        out_file.write_all(format!("\t{}\n", generate_field(name, typ, true)).as_bytes())?;
    }

    out_file.write_all("}\n\n".as_bytes())?;

    Ok(())
}

fn generate_field(name: &str, tpe: &dyn TypeScriptType, is_typescript: bool) -> String {
    if is_typescript {
        format!("{}: {}", name, tpe.typescript_type())
    } else {
        name.to_string()
    }
}

fn generate_method_skeleton(
    name: &str,
    arguments: &[AstArgument<Typed>],
    return_type: Option<&dyn TypeScriptType>,
    out_file: &mut File,
    is_typescript: bool,
) -> Result<(), ModelBuildingError> {
    // We put `async` in a comment as an indication to the user that it is okay to have async functions
    out_file.write_all("export async function ".as_bytes())?;
    out_file.write_all(name.as_bytes())?;
    out_file.write_all("(".as_bytes())?;

    generate_arguments_skeleton(arguments, out_file, is_typescript)?;

    out_file.write_all(")".as_bytes())?;

    if is_typescript {
        if let Some(return_type) = return_type {
            out_file.write_all(": Promise<".as_bytes())?;
            out_file.write_all(return_type.typescript_type().as_bytes())?;
            out_file.write_all(">".as_bytes())?;
        }
    }

    out_file.write_all(" {\n".as_bytes())?;
    out_file.write_all("\t// TODO\n".as_bytes())?;
    out_file.write_all("\tthrow new Error('not implemented');\n".as_bytes())?;
    out_file.write_all("}\n\n".as_bytes())?;

    Ok(())
}

fn generate_arguments_skeleton(
    arguments: &[AstArgument<Typed>],
    out_file: &mut File,
    is_typescript: bool,
) -> Result<(), ModelBuildingError> {
    let args_str = arguments
        .iter()
        .map(|argument| generate_field(&argument.name, &argument.typ, is_typescript))
        .collect::<Vec<_>>()
        .join(", ");

    out_file.write_all(args_str.as_bytes())?;

    Ok(())
}

trait Type {
    fn name(&self) -> &str;
    fn fields(&self) -> Vec<(&str, &dyn TypeScriptType)>;
}

impl Type for AstModel<Typed> {
    fn fields(&self) -> Vec<(&str, &dyn TypeScriptType)> {
        self.fields
            .iter()
            .map(|field| (field.name.as_str(), &field.typ as &dyn TypeScriptType))
            .collect()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Type for ContextType {
    fn fields(&self) -> Vec<(&str, &dyn TypeScriptType)> {
        self.fields
            .iter()
            .map(|field| (field.name.as_str(), &field.typ as &dyn TypeScriptType))
            .collect()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

trait TypeScriptType {
    fn typescript_type(&self) -> String;
}

impl TypeScriptType for AstFieldType<Typed> {
    fn typescript_type(&self) -> String {
        match self {
            AstFieldType::Optional(tpe) => format!("{}?", tpe.typescript_type()),
            AstFieldType::Plain(name, ..) => typescript_base_type(name),
        }
    }
}

impl TypeScriptType for ContextFieldType {
    fn typescript_type(&self) -> String {
        match self {
            ContextFieldType::Optional(typ) => format!("{}?", typ.typescript_type()),
            ContextFieldType::Plain(pt) => typescript_base_type(&pt.name()),
            ContextFieldType::List(typ) => format!("{}[]", typ.typescript_type()),
        }
    }
}

fn typescript_base_type(exo_type_name: &str) -> String {
    match exo_type_name {
        "String" => "string".to_string(),
        "Int" => "number".to_string(),
        "Float" => "number".to_string(),
        "Boolean" => "boolean".to_string(),
        "DateTime" => "Date".to_string(),
        "Exograph" => "Exograph".to_string(),
        "ExographPriv" => "ExographPriv".to_string(),
        t => t.to_string(),
    }
}
