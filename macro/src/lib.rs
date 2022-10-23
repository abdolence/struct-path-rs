//! A helper macros implementation to build a string that represents struct fields path at compile time.
//!
//! Library provides a tiny macro implementation to reference Rust struct fields at compile time to represent its string format.
//! This is needed to work with JSON paths, and some others protocols when we still want to rely on the compiler to avoid inconsistent changes.
//!
//! Features:
//! - Fast and no macro parsing without huge deps;
//! - Macro produces the code to verify if the specified path really exists;
//! - Multiple fields/arrays support
//! - Optional camelCase and PascalCase conversion support;
//! - Optional delimiter parameter;
//! Example:
//!
//! ```rust,no_run
//! use struct_path::*;
//!
//! fn example() {
//!
//! pub struct TestStructParent {
//!     pub value_str: String,
//!     pub value_num: u64,
//!     pub value_child: TestStructChild,
//! }
//!
//! pub struct TestStructChild {
//!     pub child_value_str: String,
//!     pub child_value_num: u64,
//! }
//!
//!// returns "value_str"
//!let s1: &str = path!(TestStructParent::value_str);
//!
//!// returns "value_child.child_value_str"
//!let s2: &str = path!(TestStructParent::value_child.child_value_str) ;
//!
//!// returns also "value_child.child_value_str"
//!let s3: &str = path!(TestStructParent::value_child,TestStructChild::child_value_str);
//!
//!// options, returns "valueChild/childValueStr"
//!let s4: &str = path!(TestStructParent::value_child.child_value_str; delim="/", case="camel") ;
//!
//!// returns ["value_str", "value_num"]
//!let arr: [&str; 2] = paths!(TestStructParent::{ value_str, value_num });
//!
//! }
//!
//! ```
//!

use convert_case::{Case, Casing};
use proc_macro::{TokenStream, TokenTree};
use std::collections::HashMap;

#[proc_macro]
pub fn paths(struct_path_stream: TokenStream) -> TokenStream {
    let mut current_struct_name: Option<String> = None;
    let mut current_struct_fields: Vec<String> = Vec::with_capacity(16);

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_path: Option<String> = None;

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: Vec<(String, Vec<String>)> = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name.is_none() => {
                current_struct_name = Some(id.to_string());
            }
            TokenTree::Punct(punct)
                if current_struct_name.is_some()
                    && !opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                if colons_counter > 1 {
                    opened_struct = true;
                }
            }
            TokenTree::Ident(id) if opened_struct => {
                colons_counter = 0;
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id.to_string().as_str())
                } else {
                    current_field_path = Some(id.to_string());
                }
            }
            TokenTree::Punct(punct)
                if current_struct_name.is_some()
                    && opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                opened_struct = false;
                if let Some(ref mut field_path) = current_field_path.take() {
                    if let Some(ref mut struct_name) = &mut current_struct_name {
                        struct_name.push_str("::");
                        struct_name.push_str(field_path);
                    }
                }
            }
            TokenTree::Punct(punct) if opened_struct && punct == '.' => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push('.');
                } else {
                    panic!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    )
                }
            }
            TokenTree::Group(group) if opened_struct && current_field_path.is_none() => {
                parse_multiple_fields(group.stream(), &mut current_struct_fields)
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if let Some(struct_name) = current_struct_name.take() {
                    if let Some(field_path) = current_field_path.take() {
                        current_struct_fields.push(field_path);
                    }
                    if !current_struct_fields.is_empty() {
                        found_structs
                            .push((struct_name, current_struct_fields.drain(..).collect()));
                    } else {
                        panic!("Unexpected comma with empty fields for {}!", struct_name);
                    }
                } else {
                    panic!("Unexpected comma with empty definitions!");
                }
            }
            TokenTree::Punct(punct) if punct == ';' && opened_struct && !options_opened => {
                options_opened = true;
                opened_struct = false;
            }
            TokenTree::Ident(id) if options_opened && !expect_option_value => {
                current_option_name = Some(id.to_string())
            }
            TokenTree::Ident(id) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        options.insert(option_name, id.to_string());
                    }
                    _ => {
                        panic!("Wrong options format")
                    }
                }
            }
            TokenTree::Literal(lit) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        let lit_str = lit.to_string();
                        options.insert(
                            option_name,
                            lit_str.as_str()[1..lit_str.len() - 1].to_string(),
                        );
                    }
                    _ => {
                        panic!("Wrong options format")
                    }
                }
            }
            TokenTree::Punct(punct) if options_opened && punct == '=' => {
                expect_option_value = true;
            }
            TokenTree::Punct(punct) if options_opened && punct == ',' => {
                expect_option_value = false;
            }
            others => {
                panic!("Unexpected input for struct path parameters: {:?}", others)
            }
        }
    }

    if let Some(field_path) = current_field_path.take() {
        current_struct_fields.push(field_path);
    }

    if let Some(struct_name) = current_struct_name.take() {
        if let Some(field_path) = current_field_path.take() {
            current_struct_fields.push(field_path);
        }
        if !current_struct_fields.is_empty() {
            found_structs.push((struct_name, current_struct_fields.drain(..).collect()));
        } else {
            panic!("Unexpected comma with empty fields for {}!", struct_name);
        }
    } else {
        panic!("Unexpected comma with empty definitions!");
    }

    let all_check_functions = generate_checks_code_for(&found_structs);

    let mut all_final_fields: Vec<String> = Vec::with_capacity(16);

    for (_, struct_fields) in &found_structs {
        for field_path in struct_fields {
            let mut final_field_path = field_path.clone();
            if !options.is_empty() {
                final_field_path = apply_options(&options, final_field_path);
            }
            all_final_fields.push(format!("\"{}\"", final_field_path))
        }
    }

    if !all_final_fields.is_empty() {
        format!(
            "{{{}\n[{}]}}",
            all_check_functions,
            all_final_fields.join(",")
        )
        .parse()
        .unwrap()
    } else {
        panic!("Empty struct fields")
    }
}

#[inline]
fn parse_multiple_fields(group_stream: TokenStream, found_struct_fields: &mut Vec<String>) {
    let mut current_field_path: Option<String> = None;

    for token_tree in group_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id.to_string().as_str())
                } else {
                    current_field_path = Some(id.to_string());
                }
            }
            TokenTree::Punct(punct) if punct == ',' => {
                if let Some(field_path) = current_field_path.take() {
                    found_struct_fields.push(field_path);
                    current_field_path = None;
                } else {
                    panic!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    )
                }
            }
            TokenTree::Punct(punct) if punct == '.' => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push('.');
                } else {
                    panic!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    )
                }
            }
            others => {
                panic!(
                    "Unexpected input for struct path group parameters: {:?}",
                    others
                )
            }
        }
    }

    if let Some(field_path) = current_field_path.take() {
        found_struct_fields.push(field_path);
    }
}

#[proc_macro]
pub fn path(struct_path_stream: TokenStream) -> TokenStream {
    let mut current_struct_name: Option<String> = None;

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_path: Option<String> = None;
    let mut current_full_field_path: Option<String> = None;

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: Vec<(String, Vec<String>)> = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name.is_none() => {
                current_struct_name = Some(id.to_string());
            }
            TokenTree::Punct(punct)
                if current_struct_name.is_some()
                    && !opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                if colons_counter > 1 {
                    opened_struct = true;
                }
            }
            TokenTree::Ident(id) if opened_struct => {
                colons_counter = 0;
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id.to_string().as_str())
                } else {
                    current_field_path = Some(id.to_string());
                }
            }
            TokenTree::Punct(punct)
                if current_struct_name.is_some()
                    && opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                opened_struct = false;
                if let Some(ref mut field_path) = current_field_path.take() {
                    if let Some(ref mut struct_name) = &mut current_struct_name {
                        struct_name.push_str("::");
                        struct_name.push_str(field_path);
                    }
                }
            }
            TokenTree::Punct(punct) if opened_struct && punct == '.' => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push('.');
                } else {
                    panic!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    )
                }
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if let Some(struct_name) = current_struct_name.take() {
                    if let Some(field_path) = current_field_path.take() {
                        found_structs.push((struct_name, vec![field_path.clone()]));

                        if let Some(full_field_path) = &mut current_full_field_path {
                            full_field_path.push('.');
                            full_field_path.push_str(field_path.as_str());
                        } else {
                            current_full_field_path = Some(field_path)
                        }
                    } else {
                        panic!("Unexpected comma with empty fields for {}!", struct_name);
                    }
                } else {
                    panic!("Unexpected comma with empty definitions!");
                }
            }
            TokenTree::Punct(punct) if punct == ';' && opened_struct && !options_opened => {
                options_opened = true;
                opened_struct = false;
            }
            TokenTree::Ident(id) if options_opened && !expect_option_value => {
                current_option_name = Some(id.to_string())
            }
            TokenTree::Ident(id) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        options.insert(option_name, id.to_string());
                    }
                    _ => {
                        panic!("Wrong options format")
                    }
                }
            }
            TokenTree::Literal(lit) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        let lit_str = lit.to_string();
                        options.insert(
                            option_name,
                            lit_str.as_str()[1..lit_str.len() - 1].to_string(),
                        );
                    }
                    _ => {
                        panic!("Wrong options format")
                    }
                }
            }
            TokenTree::Punct(punct) if options_opened && punct == '=' => {
                expect_option_value = true;
            }
            TokenTree::Punct(punct) if options_opened && punct == ',' => {
                expect_option_value = false;
            }
            others => {
                panic!("Unexpected input for struct path parameters: {:?}", others)
            }
        }
    }

    if let Some(struct_name) = current_struct_name.take() {
        if let Some(field_path) = current_field_path.take() {
            found_structs.push((struct_name, vec![field_path.clone()]));

            if let Some(full_field_path) = &mut current_full_field_path {
                full_field_path.push('.');
                full_field_path.push_str(field_path.as_str());
            } else {
                current_full_field_path = Some(field_path)
            }
        }
    }

    if let Some(full_field_path) = current_full_field_path.take() {
        let all_check_functions = generate_checks_code_for(&found_structs);
        let final_field_path = apply_options(&options, full_field_path);
        let result_str = format!("{{{}\n\"{}\"}}", all_check_functions, final_field_path);
        result_str.parse().unwrap()
    } else {
        panic!("Unexpected empty path definition!");
    }
}

#[inline]
fn generate_checks_code_for(found_structs: &Vec<(String, Vec<String>)>) -> String {
    let mut all_check_functions = String::new();

    for (struct_name, struct_fields) in found_structs {
        let check_functions = struct_fields
            .iter()
            .map(|field_path| {
                format!(
                    r#"
                {{
                    #[allow(dead_code, unused_variables)]
                    fn _test_struct_field(test_struct: &{}) {{
                        let _t = &test_struct.{};
                    }}
                }}
            "#,
                    struct_name, field_path
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        all_check_functions.push_str(&check_functions);
    }
    all_check_functions
}

#[inline]
fn apply_options(options: &HashMap<String, String>, field_path: String) -> String {
    let delim = options
        .get("delim")
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| ".");
    let case = options.get("case");
    field_path
        .split('.')
        .map(|field_name| {
            if let Some(case_value) = case {
                match case_value.as_str() {
                    "camel" => field_name.from_case(Case::Snake).to_case(Case::Camel),
                    "pascal" => field_name.from_case(Case::Snake).to_case(Case::Pascal),
                    another => panic!("Unknown case is specified: {}", another),
                }
            } else {
                field_name.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join(delim)
}
