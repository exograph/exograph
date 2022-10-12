/// Specification for an annotation.
pub struct AnnotationSpec {
    /// List of targets the annotation is allowed to be applied to.
    pub targets: &'static [AnnotationTarget],
    /// Is this annotation allowed to have no parameters?
    pub no_params: bool,
    /// Is this annotation allowed to have a single parameter?
    pub single_params: bool,
    /// List of mapped parameters if mapped parameters are allowed (`None` if not).
    pub mapped_params: Option<&'static [MappedAnnotationParamSpec]>,
}

/// Target for an annotation.
#[derive(Debug, PartialEq, Eq)]
pub enum AnnotationTarget {
    Model,
    Field,
    Argument,
    Service,
    Method,
    Interceptor,
}

/// Specification for a mapped parameter of an annotation.
pub struct MappedAnnotationParamSpec {
    /// Name of the parameter.
    pub name: &'static str,
    /// Is this parameter optional?
    pub optional: bool,
}
