// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fs::create_dir_all;
use std::io::Write;
use std::path::PathBuf;
use std::{fs::File, path::Path};

use core_plugin_interface::core_model::context_type::{ContextFieldType, ContextType};
use core_plugin_interface::core_model_builder::builder::system_builder::BaseModelSystem;
use core_plugin_interface::core_model_builder::{
    ast::ast_types::{AstArgument, AstFieldType, AstModel, AstModule},
    error::ModelBuildingError,
    typechecker::Typed,
};

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
///     query todo(id: Int, exograph: Exograph, @inject authContext: AuthContext): Todo
///   }
/// ```
///
/// The generated code will look like this:
/// ```typescript
/// import type { Exograph } from '../generated/exograph.d.ts';
///
/// import { AuthContext } from './contexts.d.ts'
///
/// export interface Todo {
///     userId: number
///     id: number
///     title: string
///     completed: boolean
/// }
///
/// export async function todo(id: number): Todo {
///     // TODO
///     throw new Error('not implemented');
/// }
/// ```
///
/// We add `async` to indicate that the function may be async, but users may remove it if they want.
///
/// If the `@deno("todo.js") was specified, the generated code will look like this:
/// ```javascript
/// export async function todo(id) {
///     // TODO
///     throw new Error('not implemented');
/// }
/// ```
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
    let out_file_dir = out_file
        .parent()
        .ok_or(ModelBuildingError::Generic(format!(
            "Cannot get parent directory of {}",
            out_file.display()
        )))?;

    // Make sure the directory exists in case the path provides is "new_dir/new_file.ts" and the "new_dir" doesn't exist.
    std::fs::create_dir_all(out_file_dir)?;

    generate_exograph_d_ts()?;

    // Generate context definitions (even if the target is a Javascript file to help with code completion)
    // Context definitions are generated in the same directory as the module code, since the types in it
    // are independent of the module code.
    generate_context_definitions(base_system)?;

    // We don't want to overwrite any user files
    // TODO: Parse the existing file and warn if any definitions don't match the expected ones
    // along with a helpful message so that users can copy/paste the expected definitions.
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
        generate_exograph_imports(module, &mut file, out_file_dir)?;
        generate_context_imports(module, base_system, &mut file, out_file_dir)?;

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

fn generate_exograph_imports(
    module: &AstModule<Typed>,
    file: &mut File,
    out_file_dir: &Path,
) -> Result<(), ModelBuildingError> {
    fn is_exograph_type(argument: &AstArgument<Typed>) -> bool {
        let exograph_type_names = ["Exograph", "ExographPriv", "Operation", "ExographError"];
        exograph_type_names.contains(&argument.typ.name().as_str())
    }

    let imports = import_types(module, &is_exograph_type);

    if imports.is_empty() {
        return Ok(());
    }

    let relative_path = generated_dir_path(out_file_dir)?;

    writeln!(
        file,
        "import type {{ {imports} }} from '{relative_path}generated/exograph.d.ts';\n"
    )?;

    Ok(())
}

fn generate_context_imports(
    module: &AstModule<Typed>,
    base_system: &BaseModelSystem,
    file: &mut File,
    out_file_dir: &Path,
) -> Result<(), ModelBuildingError> {
    fn is_context_type(argument: &AstArgument<Typed>, base_system: &BaseModelSystem) -> bool {
        base_system
            .contexts
            .get_by_key(&argument.typ.name())
            .is_some()
    }

    let imports = import_types(module, &|arg| is_context_type(arg, base_system));

    if imports.is_empty() {
        return Ok(());
    }

    let relative_path = generated_dir_path(out_file_dir)?;

    writeln!(
        file,
        "import type {{ {imports} }} from '{relative_path}generated/contexts.d.ts';\n"
    )?;

    Ok(())
}

fn generated_dir_path(out_file_dir: &Path) -> Result<String, ModelBuildingError> {
    // Exograph projects have a src/index.exo file
    fn is_exoproject(dir: &Path) -> bool {
        fn directory_contains(dir: &Path, name: &str, is_dir: bool) -> bool {
            if !dir.is_dir() {
                return false;
            }

            let dir_entry = dir.read_dir().unwrap().flatten().find(|dir_entry| {
                dir_entry.file_name() == name && dir_entry.file_type().unwrap().is_dir() == is_dir
            });

            dir_entry.is_some()
        }

        directory_contains(dir, "src", true) && {
            let src_dir = dir.join("src");
            directory_contains(&src_dir, "index.exo", false)
        }
    }

    // Find out how many levels up we need to go to get to the root of the project
    // Then, we can generate a relative path to the generated/contexts.d.ts file
    let mut relative_depth = 0;
    let mut current_dir = out_file_dir.canonicalize()?;
    while !is_exoproject(&current_dir) {
        relative_depth += 1;
        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Ok("../".repeat(relative_depth))
}

/// Collect all types used in the module matching the given selection criteria.
fn import_types(
    module: &AstModule<Typed>,
    selection: &impl Fn(&AstArgument<Typed>) -> bool,
) -> String {
    fn arguments(module: &AstModule<Typed>) -> impl Iterator<Item = &AstArgument<Typed>> {
        let method_arguments = module
            .methods
            .iter()
            .flat_map(|method| method.arguments.iter());

        let interceptor_arguments = module
            .interceptors
            .iter()
            .flat_map(|interceptor| interceptor.arguments.iter());

        method_arguments.chain(interceptor_arguments)
    }

    let mut types_used = arguments(module)
        .filter(|arg| selection(arg))
        .map(|arg| arg.typ.name())
        .collect::<Vec<_>>();
    types_used.dedup();
    types_used.sort(); // Sort to make the output deterministic

    types_used.join(", ")
}

/// Generate a exograph.d.ts, which exports everything from the type definition file from https://deno.land/x/exograph.
/// This level of indirection helps to avoid changing user code with each version of exograph.
fn generate_exograph_d_ts() -> Result<(), ModelBuildingError> {
    let generated_dir = PathBuf::from("generated");
    create_dir_all(&generated_dir)?;

    let file_path = generated_dir.join("exograph.d.ts");

    let package_version = env!("CARGO_PKG_VERSION");
    let mut file = std::fs::File::create(file_path)?;
    file.write_all(
        format!("export * from 'https://deno.land/x/exograph@v{package_version}/index.ts';")
            .as_bytes(),
    )?;

    Ok(())
}

fn generate_context_definitions(base_system: &BaseModelSystem) -> Result<(), ModelBuildingError> {
    let generated_dir = PathBuf::from("generated");

    create_dir_all(&generated_dir)?;

    // Assume that (currently satisfied by the cli) that the current working directory is the root of the project.
    let context_file = generated_dir.join("contexts.d.ts");

    if base_system.contexts.is_empty() {
        // Remove the file if it exists to ensure that the non-existence of contexts is reflected in the file system.
        if std::path::Path::exists(&context_file) {
            std::fs::remove_file(context_file)?;
        }
        return Ok(());
    }

    let mut file = std::fs::File::create(context_file)?;
    for (_, context) in base_system.contexts.iter() {
        generate_type_skeleton(context, &mut file)?;
    }

    Ok(())
}

fn generate_type_skeleton(model: &dyn Type, out_file: &mut File) -> Result<(), ModelBuildingError> {
    out_file.write_all(format!("export interface {} {{\n", model.name()).as_bytes())?;

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
        "Uuid" => "string".to_string(),
        "Exograph" => "Exograph".to_string(),
        "ExographPriv" => "ExographPriv".to_string(),
        t => t.to_string(),
    }
}
