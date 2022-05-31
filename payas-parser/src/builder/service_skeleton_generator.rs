use std::io::Write;
use std::{fs::File, path::Path};

use crate::ast::ast_types::AstArgument;
use crate::{
    ast::ast_types::{AstFieldType, AstModel, AstService},
    error::ParserError,
    typechecker::Typed,
};

// Temporary. Eventually, we will have a published artifact (at https://deno.land/x/claytip@<version>) that contains this code.
// Then, we will have this imported in each generated service code (currently, it suffices to just have it in the same directory as the service code).
static CLAYTIP_D_TS: &str = r#"
interface Claytip {
  executeQuery(query: string, variable?: { [key: string]: any }): Promise<any>;
  addResponseHeader(name: string, value: string ): Promise<void>;
  setCookie(cookie: {
    name: string,
    value: string,
    expires: Date,
    maxAge: number,
    domain: string,
    path: string,
    secure: boolean,
    httpOnly: boolean,
    sameSite: "Lax" | "Strict" | "None"
  }): Promise<void>;
}
 
interface Operation {
  name(): Promise<string>;
  proceed<T>(): Promise<T>;
}
"#;

/// Generates a service skeleton based on service definitions in the clay file so that users can have a good starting point.
///
/// # Example:
/// For a service definition in a clay file as follows:
/// ```clay
/// @external("todo.ts")
/// service TodoService {
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
/// If the `@external("todo.js") was specified, the generated code will look like this:
/// ```javascript
/// export /*async*/ function todo(id) {
///     // TODO
///     throw new Error('not implemented');
/// }
/// ```
/// We also generate a claytip.d.ts file that contains the Claytip interface.
///
pub fn generate_service_skeleton(
    service: &AstService<Typed>,
    out_file: impl AsRef<Path>,
) -> Result<(), ParserError> {
    let is_typescript = out_file
        .as_ref()
        .extension()
        .map(|ext| ext == "ts")
        .unwrap_or(false);

    let out_file = Path::new(out_file.as_ref());

    // Generated a typescript definition file even for Javscript, so that user can know
    // the exepected interface and IDEs can assist with code completion (if they use jsdoc, for).
    let claytip_d_path = out_file.parent().unwrap().join("claytip.d.ts");
    if !claytip_d_path.exists() {
        let mut claytip_d_file = File::create(&claytip_d_path)?;
        claytip_d_file.write_all(CLAYTIP_D_TS.as_bytes())?;
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

    // Types (defined in `service`) matter only if the target is a typescript file.
    if is_typescript {
        for model in service.models.iter() {
            generate_type_skeleton(model, &mut file)?;
        }
    }

    for method in service.methods.iter() {
        generate_method_skeleton(
            &method.name,
            &method.arguments,
            Some(&method.return_type),
            &mut file,
            is_typescript,
        )?;
    }

    for interceptor in service.interceptors.iter() {
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

fn generate_type_skeleton(model: &AstModel<Typed>, out_file: &mut File) -> Result<(), ParserError> {
    out_file.write_all(format!("interface {} {{\n", model.name).as_bytes())?;

    for field in model.fields.iter() {
        out_file.write_all(
            format!("\t{}\n", generate_field(&field.name, &field.typ, true)).as_bytes(),
        )?;
    }

    out_file.write_all("}\n\n".as_bytes())?;

    Ok(())
}

fn generate_field(name: &str, tpe: &AstFieldType<Typed>, is_typescript: bool) -> String {
    if is_typescript {
        format!("{}: {}", name, typescript_type(tpe))
    } else {
        name.to_string()
    }
}

fn generate_method_skeleton(
    name: &str,
    arguments: &[AstArgument<Typed>],
    return_type: Option<&AstFieldType<Typed>>,
    out_file: &mut File,
    is_typescript: bool,
) -> Result<(), ParserError> {
    // We put `async` in a comment as an indication to the user that it is okay to have async functions
    out_file.write_all("export async function ".as_bytes())?;
    out_file.write_all(name.as_bytes())?;
    out_file.write_all("(".as_bytes())?;

    generate_arguments_skeleton(arguments, out_file, is_typescript)?;

    out_file.write_all(")".as_bytes())?;

    if is_typescript {
        if let Some(return_type) = return_type {
            out_file.write_all(": Promise<".as_bytes())?;
            out_file.write_all(typescript_type(return_type).as_bytes())?;
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
) -> Result<(), ParserError> {
    let args_str = arguments
        .iter()
        .map(|argument| generate_field(&argument.name, &argument.typ, is_typescript))
        .collect::<Vec<_>>()
        .join(", ");

    out_file.write_all(args_str.as_bytes())?;

    Ok(())
}

fn typescript_type(tpe: &AstFieldType<Typed>) -> String {
    match tpe {
        AstFieldType::Optional(tpe) => format!("{}?", typescript_type(tpe)),
        AstFieldType::Plain(name, ..) => typescript_base_type(name),
    }
}

fn typescript_base_type(clay_type_name: &str) -> String {
    match clay_type_name {
        "String" => "string".to_string(),
        "Int" => "number".to_string(),
        "Float" => "number".to_string(),
        "Boolean" => "boolean".to_string(),
        "DateTime" => "Date".to_string(),
        "ClaytipInjected" => "Claytip".to_string(),
        t => t.to_string(),
    }
}
