//! Language table: file extensions, tree-sitter grammars and the queries that
//! extract definitions, calls, imports and heritage references.
//!
//! Capture conventions used by every query:
//! - `@def.<kind>` the whole definition node, `@name` its name
//! - `@owner` optional owning type (Go receivers, C++ `Foo::bar`)
//! - `@scope.impl` + `@scope.type` a Rust `impl` block that owns nested functions
//! - `@call` the callee name of a bare call; `@mcall` + `@qual` a member or
//!   qualified call (`obj.m()`, `Type::f()`) and its receiver
//! - `@ref` a heritage reference (extends / implements)
//! - `@import` a module specifier

use std::sync::OnceLock;
use tree_sitter::{Language, Query};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Rust,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Kotlin,
    Swift,
}

impl Lang {
    pub const ALL: [Lang; 14] = [
        Lang::TypeScript,
        Lang::Tsx,
        Lang::JavaScript,
        Lang::Python,
        Lang::Go,
        Lang::Rust,
        Lang::Java,
        Lang::C,
        Lang::Cpp,
        Lang::CSharp,
        Lang::Ruby,
        Lang::Php,
        Lang::Kotlin,
        Lang::Swift,
    ];

    pub fn from_path(path: &str) -> Option<Lang> {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".d.ts") {
            return Some(Lang::TypeScript);
        }
        let ext = lower.rsplit_once('.').map(|(_, e)| e)?;
        Some(match ext {
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "py" | "pyi" => Lang::Python,
            "go" => Lang::Go,
            "rs" => Lang::Rust,
            "java" => Lang::Java,
            "c" | "h" => Lang::C,
            "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "ipp" | "inl" => Lang::Cpp,
            "cs" => Lang::CSharp,
            "rb" | "rake" => Lang::Ruby,
            "php" => Lang::Php,
            "kt" | "kts" => Lang::Kotlin,
            "swift" => Lang::Swift,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::TypeScript => "TypeScript",
            Lang::Tsx => "TSX",
            Lang::JavaScript => "JavaScript",
            Lang::Python => "Python",
            Lang::Go => "Go",
            Lang::Rust => "Rust",
            Lang::Java => "Java",
            Lang::C => "C",
            Lang::Cpp => "C++",
            Lang::CSharp => "C#",
            Lang::Ruby => "Ruby",
            Lang::Php => "PHP",
            Lang::Kotlin => "Kotlin",
            Lang::Swift => "Swift",
        }
    }

    pub fn grammar(self) -> Language {
        match self {
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Java => tree_sitter_java::LANGUAGE.into(),
            Lang::C => tree_sitter_c::LANGUAGE.into(),
            Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Lang::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Lang::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Lang::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Lang::Swift => tree_sitter_swift::LANGUAGE.into(),
        }
    }

    fn source(self) -> String {
        match self {
            Lang::TypeScript | Lang::Tsx => {
                let mut q = String::from(JS_COMMON);
                q.push_str(TS_EXTRA);
                if self == Lang::Tsx {
                    q.push_str(JSX);
                }
                q
            }
            Lang::JavaScript => {
                let mut q = String::from(JS_COMMON);
                q.push_str(JS_EXTRA);
                q.push_str(JSX);
                q
            }
            Lang::Python => PYTHON.into(),
            Lang::Go => GO.into(),
            Lang::Rust => RUST.into(),
            Lang::Java => JAVA.into(),
            Lang::C => format!("{C_COMMON}{C_ONLY}"),
            Lang::Cpp => format!("{C_COMMON}{CPP}"),
            Lang::CSharp => CSHARP.into(),
            Lang::Ruby => RUBY.into(),
            Lang::Php => PHP.into(),
            Lang::Kotlin => KOTLIN.into(),
            Lang::Swift => SWIFT.into(),
        }
    }

    /// Compiled query, built lazily once per language per process.
    pub fn query(self) -> &'static Query {
        const EMPTY: OnceLock<Query> = OnceLock::new();
        static CACHE: [OnceLock<Query>; 14] = [EMPTY; 14];
        let idx = Lang::ALL.iter().position(|l| *l == self).expect("lang in table");
        CACHE[idx].get_or_init(|| {
            Query::new(&self.grammar(), &self.source())
                .unwrap_or_else(|err| panic!("query for {} failed: {err}", self.name()))
        })
    }
}

/// Text files that are indexed for search (but not parsed into symbols).
pub fn is_searchable_text(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if matches!(
        name,
        "dockerfile" | "makefile" | "justfile" | "gemfile" | "rakefile" | "procfile" | "readme"
    ) {
        return true;
    }
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        ext,
        "md" | "mdx"
            | "txt"
            | "rst"
            | "adoc"
            | "yaml"
            | "yml"
            | "toml"
            | "json"
            | "jsonc"
            | "ini"
            | "cfg"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "ps1"
            | "sql"
            | "proto"
            | "graphql"
            | "gql"
            | "prisma"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "vue"
            | "svelte"
            | "astro"

            | "scala"
            | "lua"
            | "ex"
            | "exs"
            | "erl"
            | "dart"
            | "zig"
            | "nim"
            | "hs"
            | "ml"
            | "clj"
            | "r"
            | "jl"
            | "tf"
            | "hcl"
            | "nix"
            | "gradle"
            | "cmake"
            | "xml"
    )
}

const JS_COMMON: &str = r#"
(function_declaration name: (identifier) @name) @def.function
(generator_function_declaration name: (identifier) @name) @def.function
(method_definition name: [(property_identifier) (private_property_identifier)] @name) @def.method
(variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @def.function
(assignment_expression left: (member_expression property: (property_identifier) @name) right: [(arrow_function) (function_expression)]) @def.function
(pair key: (property_identifier) @name value: [(arrow_function) (function_expression)]) @def.method
(call_expression function: (identifier) @call)
(call_expression function: (member_expression object: (_) @qual property: [(property_identifier) (private_property_identifier)] @mcall))
(new_expression constructor: (identifier) @call)
(import_statement source: (string (string_fragment) @import))
(export_statement source: (string (string_fragment) @import))
(call_expression function: (identifier) @_req arguments: (arguments . (string (string_fragment) @import)) (#eq? @_req "require"))
(call_expression function: (import) arguments: (arguments . (string (string_fragment) @import)))
"#;

const JS_EXTRA: &str = r#"
(class_declaration name: (identifier) @name) @def.class
(class_heritage (identifier) @ref)
(class_heritage (member_expression property: (property_identifier) @ref))
(field_definition property: (property_identifier) @name value: [(arrow_function) (function_expression)]) @def.method
"#;

const TS_EXTRA: &str = r#"
(class_declaration name: (type_identifier) @name) @def.class
(abstract_class_declaration name: (type_identifier) @name) @def.class
(interface_declaration name: (type_identifier) @name) @def.interface
(type_alias_declaration name: (type_identifier) @name) @def.type
(enum_declaration name: (identifier) @name) @def.enum
(function_signature name: (identifier) @name) @def.function
(abstract_method_signature name: (property_identifier) @name) @def.method
(public_field_definition name: (property_identifier) @name value: [(arrow_function) (function_expression)]) @def.method
(extends_clause value: (identifier) @ref)
(implements_clause (type_identifier) @ref)
(extends_type_clause type: (type_identifier) @ref)
"#;

const JSX: &str = r#"
(jsx_opening_element name: (identifier) @call)
(jsx_self_closing_element name: (identifier) @call)
"#;

const PYTHON: &str = r#"
(function_definition name: (identifier) @name) @def.function
(class_definition name: (identifier) @name) @def.class
(call function: (identifier) @call)
(call function: (attribute object: (_) @qual attribute: (identifier) @mcall))
(class_definition superclasses: (argument_list (identifier) @ref))
(class_definition superclasses: (argument_list (attribute attribute: (identifier) @ref)))
(import_statement name: (dotted_name) @import)
(import_statement name: (aliased_import name: (dotted_name) @import))
(import_from_statement module_name: [(dotted_name) (relative_import)] @import)
(import_from_statement module_name: [(dotted_name) (relative_import)] @import.from name: (dotted_name) @import.name)
(import_from_statement module_name: [(dotted_name) (relative_import)] @import.from name: (aliased_import name: (dotted_name) @import.name))
"#;

const GO: &str = r#"
(function_declaration name: (identifier) @name) @def.function
(method_declaration receiver: (parameter_list (parameter_declaration type: [(type_identifier) @owner (pointer_type (type_identifier) @owner) (generic_type type: (type_identifier) @owner) (pointer_type (generic_type type: (type_identifier) @owner))])) name: (field_identifier) @name) @def.method
(type_spec name: (type_identifier) @name type: (struct_type)) @def.struct
(type_spec name: (type_identifier) @name type: (interface_type)) @def.interface
(type_spec name: (type_identifier) @name type: [(type_identifier) (qualified_type) (function_type) (map_type) (slice_type) (pointer_type) (channel_type) (array_type) (generic_type)]) @def.type
(call_expression function: (identifier) @call)
(call_expression function: (selector_expression operand: (_) @qual field: (field_identifier) @mcall))
(composite_literal type: (type_identifier) @call)
(import_spec path: (interpreted_string_literal) @import)
"#;

const RUST: &str = r#"
(function_item name: (identifier) @name) @def.function
(function_signature_item name: (identifier) @name) @def.function
(struct_item name: (type_identifier) @name) @def.struct
(enum_item name: (type_identifier) @name) @def.enum
(union_item name: (type_identifier) @name) @def.struct
(trait_item name: (type_identifier) @name) @def.trait
(type_item name: (type_identifier) @name) @def.type
(macro_definition name: (identifier) @name) @def.macro
(mod_item name: (identifier) @name body: (declaration_list)) @def.module
(impl_item type: [(type_identifier) @scope.type (generic_type type: (type_identifier) @scope.type) (scoped_type_identifier name: (type_identifier) @scope.type)]) @scope.impl
(impl_item trait: [(type_identifier) @ref (generic_type type: (type_identifier) @ref) (scoped_type_identifier name: (type_identifier) @ref)])
(call_expression function: (identifier) @call)
(call_expression function: (field_expression value: (_) @qual field: (field_identifier) @mcall))
(call_expression function: (scoped_identifier path: (_) @qual name: (identifier) @mcall))
(call_expression function: (generic_function function: [(identifier) @call (scoped_identifier path: (_) @qual name: (identifier) @mcall) (field_expression value: (_) @qual field: (field_identifier) @mcall)]))
(struct_expression name: [(type_identifier) @call (scoped_type_identifier name: (type_identifier) @call)])
(macro_invocation macro: (identifier) @call)
(use_declaration argument: (_) @import)
(mod_item name: (identifier) @import.mod !body)
"#;

const JAVA: &str = r#"
(class_declaration name: (identifier) @name) @def.class
(interface_declaration name: (identifier) @name) @def.interface
(enum_declaration name: (identifier) @name) @def.enum
(record_declaration name: (identifier) @name) @def.class
(annotation_type_declaration name: (identifier) @name) @def.interface
(method_declaration name: (identifier) @name) @def.method
(constructor_declaration name: (identifier) @name) @def.method
(method_invocation !object name: (identifier) @call)
(method_invocation object: (_) @qual name: (identifier) @mcall)
(object_creation_expression type: (type_identifier) @call)
(superclass (type_identifier) @ref)
(super_interfaces (type_list (type_identifier) @ref))
(extends_interfaces (type_list (type_identifier) @ref))
(import_declaration (scoped_identifier) @import)
"#;

const C_COMMON: &str = r#"
(function_definition declarator: (function_declarator declarator: (identifier) @name)) @def.function
(function_definition declarator: (pointer_declarator declarator: (function_declarator declarator: (identifier) @name))) @def.function
(struct_specifier name: (type_identifier) @name body: (_)) @def.struct
(union_specifier name: (type_identifier) @name body: (_)) @def.struct
(enum_specifier name: (type_identifier) @name body: (_)) @def.enum
(type_definition declarator: (type_identifier) @name) @def.type
(preproc_function_def name: (identifier) @name) @def.macro
(call_expression function: (identifier) @call)
(call_expression function: (field_expression argument: (_) @qual field: (field_identifier) @mcall))
(preproc_include path: (string_literal) @import)
(preproc_include path: (system_lib_string) @import)
"#;

const C_ONLY: &str = "";

const CPP: &str = r#"
(function_definition declarator: (function_declarator declarator: (field_identifier) @name)) @def.method
(function_definition declarator: (function_declarator declarator: (qualified_identifier scope: (_) @owner name: (identifier) @name))) @def.method
(function_definition declarator: (reference_declarator (function_declarator declarator: (qualified_identifier scope: (_) @owner name: (identifier) @name)))) @def.method
(function_definition declarator: (pointer_declarator declarator: (function_declarator declarator: (qualified_identifier scope: (_) @owner name: (identifier) @name)))) @def.method
(class_specifier name: (type_identifier) @name body: (_)) @def.class
(call_expression function: (qualified_identifier scope: (_) @qual name: (identifier) @mcall))
(call_expression function: (template_function name: (identifier) @call))
(base_class_clause (type_identifier) @ref)
(base_class_clause (qualified_identifier name: (type_identifier) @ref))
"#;

const CSHARP: &str = r#"
(class_declaration name: (identifier) @name) @def.class
(interface_declaration name: (identifier) @name) @def.interface
(struct_declaration name: (identifier) @name) @def.struct
(enum_declaration name: (identifier) @name) @def.enum
(record_declaration name: (identifier) @name) @def.class
(method_declaration name: (identifier) @name) @def.method
(constructor_declaration name: (identifier) @name) @def.method
(local_function_statement name: (identifier) @name) @def.function
(invocation_expression function: (identifier) @call)
(invocation_expression function: (member_access_expression expression: (_) @qual name: (identifier) @mcall))
(invocation_expression function: (generic_name (identifier) @call))
(object_creation_expression type: (identifier) @call)
(base_list (identifier) @ref)
(base_list (generic_name (identifier) @ref))
(using_directive (qualified_name) @import)
(using_directive (identifier) @import)
"#;

const RUBY: &str = r#"
(method name: (_) @name) @def.method
(singleton_method name: (_) @name) @def.method
(class name: (constant) @name) @def.class
(class name: (scope_resolution name: (constant) @name)) @def.class
(module name: (constant) @name) @def.module
(module name: (scope_resolution name: (constant) @name)) @def.module
(call !receiver method: (identifier) @call)
(call receiver: (_) @qual method: (identifier) @mcall)
(superclass (constant) @ref)
(superclass (scope_resolution name: (constant) @ref))
(call method: (identifier) @_req arguments: (argument_list . (string (string_content) @import)) (#match? @_req "^(require|require_relative|load)$"))
"#;

const PHP: &str = r#"
(function_definition name: (name) @name) @def.function
(method_declaration name: (name) @name) @def.method
(class_declaration name: (name) @name) @def.class
(interface_declaration name: (name) @name) @def.interface
(trait_declaration name: (name) @name) @def.trait
(enum_declaration name: (name) @name) @def.enum
(function_call_expression function: (name) @call)
(function_call_expression function: (qualified_name (name) @call))
(member_call_expression object: (_) @qual name: (name) @mcall)
(nullsafe_member_call_expression object: (_) @qual name: (name) @mcall)
(scoped_call_expression scope: (_) @qual name: (name) @mcall)
(object_creation_expression (name) @call)
(object_creation_expression (qualified_name (name) @call))
(base_clause (name) @ref)
(base_clause (qualified_name (name) @ref))
(class_interface_clause (name) @ref)
(class_interface_clause (qualified_name (name) @ref))
(namespace_use_clause (qualified_name) @import)
"#;

const KOTLIN: &str = r#"
(class_declaration name: (identifier) @name) @def.class
(object_declaration name: (identifier) @name) @def.class
(function_declaration name: (identifier) @name) @def.function
(type_alias type: (identifier) @name) @def.type
(call_expression . (identifier) @call)
(call_expression . (navigation_expression (_) @qual (identifier) @mcall .))
(delegation_specifier (user_type (identifier) @ref))
(delegation_specifier (constructor_invocation (user_type (identifier) @ref)))
(import (qualified_identifier) @import)
"#;

const SWIFT: &str = r#"
(class_declaration name: (type_identifier) @name) @def.class
(protocol_declaration name: (type_identifier) @name) @def.interface
(function_declaration name: (simple_identifier) @name) @def.function
(protocol_function_declaration name: (simple_identifier) @name) @def.method
(typealias_declaration name: (type_identifier) @name) @def.type
(class_declaration name: (user_type (type_identifier) @scope.type)) @scope.impl
(inheritance_specifier inherits_from: (user_type (type_identifier) @ref))
(call_expression . (simple_identifier) @call)
(call_expression . (navigation_expression target: (_) @qual suffix: (navigation_suffix suffix: (simple_identifier) @mcall)))
(import_declaration (identifier) @import)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_query_compiles() {
        for lang in Lang::ALL {
            let _ = lang.query();
        }
    }

    #[test]
    fn detects_languages() {
        assert_eq!(Lang::from_path("a/b.tsx"), Some(Lang::Tsx));
        assert_eq!(Lang::from_path("x.d.ts"), Some(Lang::TypeScript));
        assert_eq!(Lang::from_path("x.py"), Some(Lang::Python));
        assert_eq!(Lang::from_path("README.md"), None);
        assert!(is_searchable_text("README.md"));
    }
}
