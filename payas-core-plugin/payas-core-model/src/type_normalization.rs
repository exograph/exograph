use async_graphql_parser::{
    types::{
        BaseType, EnumValueDefinition, FieldDefinition, InputValueDefinition, Type, TypeDefinition,
        TypeKind,
    },
    Pos, Positioned,
};
use async_graphql_value::Name;

pub trait FieldDefinitionProvider<S> {
    fn field_definition(&self, system: &S) -> FieldDefinition;
}

pub trait TypeDefinitionProvider<S> {
    fn type_definition(&self, system: &S) -> TypeDefinition;
}

pub trait InputValueProvider {
    fn input_value(&self) -> InputValueDefinition;
}

pub fn default_positioned<T>(value: T) -> Positioned<T> {
    Positioned::new(value, Pos::default())
}

pub fn default_positioned_name(value: &str) -> Positioned<Name> {
    default_positioned(Name::new(value))
}

pub trait ParameterType {
    fn name(&self) -> &String;
}

pub enum TypeModifier {
    List,
    NonNull,
    Optional,
}

pub trait Parameter {
    fn name(&self) -> &str;
    fn type_name(&self) -> &str;
    fn type_modifier(&self) -> TypeModifier;
}

macro_rules! parameter_input_value_provider {
    () => {
        fn input_value(&self) -> InputValueDefinition {
            let field_type =
                default_positioned(value_type(self.type_name(), &self.type_modifier()));

            InputValueDefinition {
                description: None,
                name: default_positioned_name(self.name()),
                ty: field_type,
                default_value: None,
                directives: vec![],
            }
        }
    };
}

impl<T: Parameter> InputValueProvider for T {
    parameter_input_value_provider!();
}

// TODO: Derive this from the one above
impl InputValueProvider for &dyn Parameter {
    parameter_input_value_provider!();
}

pub fn value_type(name: &str, type_modifier: &TypeModifier) -> Type {
    let base_field_type = BaseType::Named(Name::new(name));
    match type_modifier {
        TypeModifier::Optional => Type {
            base: base_field_type,
            nullable: true,
        },
        TypeModifier::NonNull => Type {
            base: base_field_type,
            nullable: false,
        },
        TypeModifier::List => Type {
            base: BaseType::List(Box::new(Type {
                base: base_field_type,
                nullable: true,
            })),
            nullable: true,
        },
    }
}

/// Deal with variants of `TypeDefinition` to give a uniform view suitable for introspection
pub trait TypeDefinitionIntrospection {
    fn name(&self) -> String;
    fn kind(&self) -> String;
    fn description(&self) -> Option<String>;
    fn fields(&self) -> Option<&Vec<Positioned<FieldDefinition>>>;
    fn interfaces(&self) -> Option<&Vec<Positioned<Name>>>;
    fn possible_types(&self) -> Option<&Vec<Positioned<Name>>>;
    fn enum_values(&self) -> Option<&Vec<Positioned<EnumValueDefinition>>>;
    fn input_fields(&self) -> Option<&Vec<Positioned<InputValueDefinition>>>;
}

impl TypeDefinitionIntrospection for TypeDefinition {
    fn name(&self) -> String {
        self.name.node.to_string()
    }

    fn kind(&self) -> String {
        match self.kind {
            TypeKind::Scalar => "SCALAR".to_owned(),
            TypeKind::Object(_) => "OBJECT".to_owned(),
            TypeKind::Interface(_) => "INTERFACE".to_owned(),
            TypeKind::Union(_) => "UNION".to_owned(),
            TypeKind::Enum(_) => "ENUM".to_owned(),
            TypeKind::InputObject(_) => "INPUT_OBJECT".to_owned(),
        }
    }

    fn description(&self) -> Option<String> {
        self.description.as_ref().map(|d| d.node.to_owned())
    }

    fn fields(&self) -> Option<&Vec<Positioned<FieldDefinition>>> {
        // Spec: return null except for ObjectType
        // TODO: includeDeprecated arg
        match &self.kind {
            TypeKind::Object(value) => Some(&value.fields),
            _ => None,
        }
    }

    fn interfaces(&self) -> Option<&Vec<Positioned<Name>>> {
        // Spec: return null except for ObjectType
        match &self.kind {
            TypeKind::Object(value) => Some(&value.implements),
            _ => None,
        }
    }

    fn possible_types(&self) -> Option<&Vec<Positioned<Name>>> {
        // Spec: return null except for UnionType and Interface
        match &self.kind {
            TypeKind::Union(value) => Some(&value.members),
            TypeKind::Interface(_value) => todo!(),
            _ => None,
        }
    }

    fn enum_values(&self) -> Option<&Vec<Positioned<EnumValueDefinition>>> {
        // Spec: return null except for EnumType
        match &self.kind {
            TypeKind::Enum(value) => Some(&value.values),
            _ => None,
        }
    }

    fn input_fields(&self) -> Option<&Vec<Positioned<InputValueDefinition>>> {
        // Spec: return null except for InputObjectType
        match &self.kind {
            TypeKind::InputObject(value) => Some(&value.fields),
            _ => None,
        }
    }
}

pub trait Operation {
    fn name(&self) -> &String;
    fn parameters(&self) -> Vec<&dyn Parameter>;
    fn return_type_name(&self) -> &str;
    fn return_type_modifier(&self) -> TypeModifier;
}

// Field definition for the query such as `venue(id: Int!): Venue`, combining such fields will form
// the Query, Mutation, and Subscription object definition
impl<T: Operation, S> FieldDefinitionProvider<S> for T {
    fn field_definition(&self, _system: &S) -> FieldDefinition {
        let fields = self
            .parameters()
            .iter()
            .map(|parameter| default_positioned(parameter.input_value()))
            .collect();

        FieldDefinition {
            description: None,
            name: default_positioned_name(self.name()),
            arguments: fields,
            directives: vec![],
            ty: default_positioned(value_type(
                &self.return_type_name(),
                &self.return_type_modifier(),
            )),
        }
    }
}
