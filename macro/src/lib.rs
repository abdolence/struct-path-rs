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
//!
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
//!     pub opt_value_child: Option<TestStructChild>,
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
//!// `~` steps through a collection field (Vec, Option, ...) while still
//!// verifying the path; returns "optValueChild/childValueStr"
//!let s5: &str = path!(TestStructParent::opt_value_child~child_value_str; delim="/", case="camel") ;
//!
//!// returns ["value_str", "value_num"]
//!let arr: [&str; 2] = paths!(TestStructParent::{ value_str, value_num });
//!
//! }
//!
//! ```
//!

use convert_case::{Case, Casing};
use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use std::collections::HashMap;

/// Converts a parse-time error into the token stream for a `compile_error!`
/// invocation, so malformed macro input surfaces as a normal compiler error
/// carrying the same message instead of a proc-macro panic.
fn compile_error_for(message: String) -> TokenStream {
    format!("compile_error!({:?})", message)
        .parse()
        .expect("a compile_error! invocation with an escaped string literal always parses")
}

/// One token of a parsed type path (`crate::tests::TestStructParent`).
/// `Sep` stands for the `::` between segments; unlike an `Ident`, it has no
/// single token in the user's input worth pointing an error at, so it is
/// always rendered at `Span::call_site()`.
#[derive(Clone)]
enum StructTok {
    Ident(Ident),
    Sep,
}

/// One token of a parsed field path (`value_child.child_value_str` or
/// `opt_value_child~child_value_str`). `Tilde` keeps the original `~`
/// punct's span so that a stepped-into type missing `iter()` is blamed on
/// the `~` the user wrote, not on the whole macro invocation.
#[derive(Clone)]
enum FieldTok {
    Ident(Ident),
    Dot,
    Tilde(Span),
}

/// A field path in both the rendered form used to build the macro's string
/// output (`text`, unchanged from before this fix) and the original tokens
/// used to build its type-check (`tokens`).
struct FieldEntry {
    text: String,
    tokens: Vec<FieldTok>,
}

/// A discovered `Type::field[, field...]` group. `name_tokens` is `None`
/// only when the type path could not be captured token-for-token (an
/// already-malformed spelling that would fail today too); the check for
/// such an entry falls back to the pre-existing string-built code at
/// `Span::call_site()` instead of losing the check altogether.
struct StructEntry {
    name: String,
    name_tokens: Option<Vec<StructTok>>,
    fields: Vec<FieldEntry>,
}

#[proc_macro]
pub fn paths(struct_path_stream: TokenStream) -> TokenStream {
    match paths_impl(struct_path_stream) {
        Ok(stream) => stream,
        Err(message) => compile_error_for(message),
    }
}

fn paths_impl(struct_path_stream: TokenStream) -> Result<TokenStream, String> {
    let mut current_struct_name: Option<String> = None;
    let mut current_struct_name_tokens: Vec<StructTok> = Vec::new();
    let mut current_struct_name_tokens_ok = true;
    let mut current_struct_fields: Vec<String> = Vec::with_capacity(16);
    let mut current_struct_fields_tokens: Vec<Vec<FieldTok>> = Vec::with_capacity(16);

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_path: Option<String> = None;
    let mut current_field_tokens: Vec<FieldTok> = Vec::new();

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: Vec<StructEntry> = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name.is_none() => {
                current_struct_name = Some(id.to_string());
                current_struct_name_tokens.push(StructTok::Ident(id));
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
                let id_text = id.to_string();
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id_text.as_str())
                } else {
                    current_field_path = Some(id_text);
                }
                current_field_tokens.push(FieldTok::Ident(id));
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
                fold_field_into_struct_name(
                    std::mem::take(&mut current_field_tokens),
                    &mut current_struct_name_tokens,
                    &mut current_struct_name_tokens_ok,
                );
            }
            TokenTree::Punct(punct) if opened_struct && (punct == '.' || punct == '~') => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push(punct.as_char());
                    current_field_tokens.push(if punct == '~' {
                        FieldTok::Tilde(punct.span())
                    } else {
                        FieldTok::Dot
                    });
                } else {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            TokenTree::Group(group) if opened_struct && current_field_path.is_none() => {
                parse_multiple_fields(
                    group.stream(),
                    &mut current_struct_fields,
                    &mut current_struct_fields_tokens,
                )?
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if let Some(struct_name) = current_struct_name.take() {
                    if let Some(field_path) = current_field_path.take() {
                        current_struct_fields.push(field_path);
                        current_struct_fields_tokens
                            .push(std::mem::take(&mut current_field_tokens));
                    }
                    if !current_struct_fields.is_empty() {
                        found_structs.push(take_struct_entry(
                            struct_name,
                            &mut current_struct_name_tokens,
                            &mut current_struct_name_tokens_ok,
                            &mut current_struct_fields,
                            &mut current_struct_fields_tokens,
                        ));
                    } else {
                        return Err(format!(
                            "Unexpected comma with empty fields for {}!",
                            struct_name
                        ));
                    }
                } else {
                    return Err("Unexpected comma with empty definitions!".to_string());
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
                        return Err("Wrong options format".to_string());
                    }
                }
            }
            TokenTree::Literal(lit) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        options.insert(option_name, unquote_literal(&lit)?);
                    }
                    _ => {
                        return Err("Wrong options format".to_string());
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
                return Err(format!(
                    "Unexpected input for struct path parameters: {:?}",
                    others
                ));
            }
        }
    }

    if let Some(field_path) = current_field_path.take() {
        current_struct_fields.push(field_path);
        current_struct_fields_tokens.push(std::mem::take(&mut current_field_tokens));
    }

    if let Some(struct_name) = current_struct_name.take() {
        if let Some(field_path) = current_field_path.take() {
            current_struct_fields.push(field_path);
            current_struct_fields_tokens.push(std::mem::take(&mut current_field_tokens));
        }
        if !current_struct_fields.is_empty() {
            found_structs.push(take_struct_entry(
                struct_name,
                &mut current_struct_name_tokens,
                &mut current_struct_name_tokens_ok,
                &mut current_struct_fields,
                &mut current_struct_fields_tokens,
            ));
        } else {
            return Err(format!(
                "Unexpected comma with empty fields for {}!",
                struct_name
            ));
        }
    } else {
        return Err("Unexpected comma with empty definitions!".to_string());
    }

    let all_check_functions = generate_checks_code_for(&found_structs)?;

    let mut all_final_fields: Vec<String> = Vec::with_capacity(16);

    for entry in &found_structs {
        for field in &entry.fields {
            let mut final_field_path = field.text.replace('~', ".");
            if !options.is_empty() {
                final_field_path = apply_options(&options, final_field_path)?;
            }
            all_final_fields.push(format!("\"{}\"", final_field_path))
        }
    }

    if !all_final_fields.is_empty() {
        let fields_str = format!("[{}]", all_final_fields.join(","));
        let fields_tokens: TokenStream = fields_str
            .parse()
            .map_err(|_| format!("Generated code failed to parse: {}", fields_str))?;
        let mut block_contents = all_check_functions;
        block_contents.extend(fields_tokens);
        Ok(TokenStream::from(TokenTree::Group(Group::new(
            Delimiter::Brace,
            block_contents,
        ))))
    } else {
        Err("Empty struct fields".to_string())
    }
}

/// Moves a struct-path continuation (`b` in `crate::b::Struct`) that was
/// tentatively parsed as a field back into the type path tokens. Bails out
/// of token-based checks for the rest of this struct (leaving the
/// string-built fallback in charge) the moment the continuation is not a
/// single identifier, since that shape has no valid representation as a
/// type-path token and would already fail to compile today.
fn fold_field_into_struct_name(
    field_tokens: Vec<FieldTok>,
    struct_name_tokens: &mut Vec<StructTok>,
    struct_name_tokens_ok: &mut bool,
) {
    if !*struct_name_tokens_ok {
        return;
    }
    let mut iter = field_tokens.into_iter();
    match (iter.next(), iter.next()) {
        (Some(FieldTok::Ident(id)), None) => {
            struct_name_tokens.push(StructTok::Sep);
            struct_name_tokens.push(StructTok::Ident(id));
        }
        _ => *struct_name_tokens_ok = false,
    }
}

fn take_struct_entry(
    name: String,
    name_tokens: &mut Vec<StructTok>,
    name_tokens_ok: &mut bool,
    fields_text: &mut Vec<String>,
    fields_tokens: &mut Vec<Vec<FieldTok>>,
) -> StructEntry {
    let resolved_name_tokens = if *name_tokens_ok {
        Some(std::mem::take(name_tokens))
    } else {
        None
    };
    *name_tokens_ok = true;
    name_tokens.clear();

    let fields = std::mem::take(fields_text)
        .into_iter()
        .zip(std::mem::take(fields_tokens))
        .map(|(text, tokens)| FieldEntry { text, tokens })
        .collect();

    StructEntry {
        name,
        name_tokens: resolved_name_tokens,
        fields,
    }
}

fn parse_multiple_fields(
    group_stream: TokenStream,
    found_struct_fields: &mut Vec<String>,
    found_struct_field_tokens: &mut Vec<Vec<FieldTok>>,
) -> Result<(), String> {
    let mut current_field_path: Option<String> = None;
    let mut current_field_tokens: Vec<FieldTok> = Vec::new();

    for token_tree in group_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) => {
                let id_text = id.to_string();
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id_text.as_str())
                } else {
                    current_field_path = Some(id_text);
                }
                current_field_tokens.push(FieldTok::Ident(id));
            }
            TokenTree::Punct(punct) if punct == ',' => {
                if let Some(field_path) = current_field_path.take() {
                    found_struct_fields.push(field_path);
                    found_struct_field_tokens.push(std::mem::take(&mut current_field_tokens));
                } else {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            TokenTree::Punct(punct) if punct == '.' => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push('.');
                    current_field_tokens.push(FieldTok::Dot);
                } else {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            others => {
                return Err(format!(
                    "Unexpected input for struct path group parameters: {:?}",
                    others
                ));
            }
        }
    }

    if let Some(field_path) = current_field_path.take() {
        found_struct_fields.push(field_path);
        found_struct_field_tokens.push(std::mem::take(&mut current_field_tokens));
    }
    Ok(())
}

#[proc_macro]
pub fn path(struct_path_stream: TokenStream) -> TokenStream {
    match path_impl(struct_path_stream) {
        Ok(stream) => stream,
        Err(message) => compile_error_for(message),
    }
}

fn path_impl(struct_path_stream: TokenStream) -> Result<TokenStream, String> {
    let mut current_struct_name: Option<String> = None;
    let mut current_struct_name_tokens: Vec<StructTok> = Vec::new();
    let mut current_struct_name_tokens_ok = true;

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_path: Option<String> = None;
    let mut current_field_tokens: Vec<FieldTok> = Vec::new();
    let mut current_full_field_path: Option<String> = None;

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: Vec<StructEntry> = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name.is_none() => {
                current_struct_name = Some(id.to_string());
                current_struct_name_tokens.push(StructTok::Ident(id));
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
                let id_text = id.to_string();
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push_str(id_text.as_str())
                } else {
                    current_field_path = Some(id_text);
                }
                current_field_tokens.push(FieldTok::Ident(id));
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
                fold_field_into_struct_name(
                    std::mem::take(&mut current_field_tokens),
                    &mut current_struct_name_tokens,
                    &mut current_struct_name_tokens_ok,
                );
            }
            TokenTree::Punct(punct) if opened_struct && (punct == '.' || punct == '~') => {
                if let Some(ref mut field_path) = &mut current_field_path {
                    field_path.push(punct.as_char());
                    current_field_tokens.push(if punct == '~' {
                        FieldTok::Tilde(punct.span())
                    } else {
                        FieldTok::Dot
                    });
                } else {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if let Some(struct_name) = current_struct_name.take() {
                    if let Some(field_path) = current_field_path.take() {
                        found_structs.push(take_struct_entry(
                            struct_name,
                            &mut current_struct_name_tokens,
                            &mut current_struct_name_tokens_ok,
                            &mut vec![field_path.clone()],
                            &mut vec![std::mem::take(&mut current_field_tokens)],
                        ));

                        if let Some(full_field_path) = &mut current_full_field_path {
                            full_field_path.push('.');
                            full_field_path.push_str(field_path.as_str());
                        } else {
                            current_full_field_path = Some(field_path)
                        }
                    } else {
                        return Err(format!(
                            "Unexpected comma with empty fields for {}!",
                            struct_name
                        ));
                    }
                } else {
                    return Err("Unexpected comma with empty definitions!".to_string());
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
                        return Err("Wrong options format".to_string());
                    }
                }
            }
            TokenTree::Literal(lit) if options_opened && expect_option_value => {
                expect_option_value = false;
                match current_option_name.take() {
                    Some(option_name) => {
                        options.insert(option_name, unquote_literal(&lit)?);
                    }
                    _ => {
                        return Err("Wrong options format".to_string());
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
                return Err(format!(
                    "Unexpected input for struct path parameters: {:?}",
                    others
                ));
            }
        }
    }

    if let Some(struct_name) = current_struct_name.take() {
        if let Some(field_path) = current_field_path.take() {
            found_structs.push(take_struct_entry(
                struct_name,
                &mut current_struct_name_tokens,
                &mut current_struct_name_tokens_ok,
                &mut vec![field_path.clone()],
                &mut vec![std::mem::take(&mut current_field_tokens)],
            ));

            if let Some(full_field_path) = &mut current_full_field_path {
                full_field_path.push('.');
                full_field_path.push_str(field_path.as_str());
            } else {
                current_full_field_path = Some(field_path)
            }
        }
    }

    if let Some(full_field_path) = current_full_field_path.take() {
        let all_check_functions = generate_checks_code_for(&found_structs)?;
        // `~` marks a collection step and must become the path separator
        // before options are applied, or a segment joined by `~` is treated
        // as one field name instead of two and never gets the chosen delim
        // or per-segment case conversion.
        let final_field_path = apply_options(&options, full_field_path.replace('~', "."))?;
        let fields_str = format!("\"{}\"", final_field_path);
        let fields_tokens: TokenStream = fields_str
            .parse()
            .map_err(|_| format!("Generated code failed to parse: {}", fields_str))?;
        let mut block_contents = all_check_functions;
        block_contents.extend(fields_tokens);
        Ok(TokenStream::from(TokenTree::Group(Group::new(
            Delimiter::Brace,
            block_contents,
        ))))
    } else {
        Err("Unexpected empty path definition!".to_string())
    }
}

/// Builds the `const _: fn(&Type) = |t: &Type| { let _ = &t.field; };`
/// check for every discovered `Type::field` pair. Where the original tokens
/// are available, the type path and field identifiers keep the user's own
/// spans (so a type error is reported on the exact token that is wrong) and
/// `~` is expanded to `.iter().next().unwrap().` at the `~` token's span;
/// everything else in the check (`const`, `_`, `fn`, `t`, `let`) has no
/// counterpart in the user's input and stays at `Span::call_site()`.
fn generate_checks_code_for(found_structs: &[StructEntry]) -> Result<TokenStream, String> {
    let mut output = TokenStream::new();
    for entry in found_structs {
        for field in &entry.fields {
            output.extend(build_check(entry, field)?);
        }
    }
    Ok(output)
}

fn build_check(entry: &StructEntry, field: &FieldEntry) -> Result<TokenStream, String> {
    match &entry.name_tokens {
        Some(name_tokens) => Ok(build_check_tokens(name_tokens, &field.tokens)),
        None => {
            let field_path_result = field.text.replace('~', ".iter().next().unwrap().");
            let code = format!(
                "const _: fn(&{0}) = |t: &{0}| {{ let _ = &t.{1}; }};",
                entry.name, field_path_result
            );
            code.parse()
                .map_err(|_| format!("Generated code failed to parse: {}", code))
        }
    }
}

fn build_check_tokens(struct_tokens: &[StructTok], field_tokens: &[FieldTok]) -> TokenStream {
    let call_site = Span::call_site();
    let type_tokens = render_struct_type(struct_tokens);

    let mut fn_param_tokens = TokenStream::new();
    fn_param_tokens.extend([TokenTree::Punct(Punct::new('&', Spacing::Alone))]);
    fn_param_tokens.extend(type_tokens.clone());

    let mut closure_param_tokens = TokenStream::new();
    closure_param_tokens.extend([
        TokenTree::Ident(Ident::new("t", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Punct(Punct::new('&', Spacing::Alone)),
    ]);
    closure_param_tokens.extend(type_tokens);

    let mut body_tokens = TokenStream::new();
    body_tokens.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Punct(Punct::new('&', Spacing::Alone)),
        TokenTree::Ident(Ident::new("t", call_site)),
        TokenTree::Punct(Punct::new('.', Spacing::Alone)),
    ]);
    body_tokens.extend(render_field_expr(field_tokens));
    body_tokens.extend([TokenTree::Punct(Punct::new(';', Spacing::Alone))]);

    let mut closure_tokens = TokenStream::new();
    closure_tokens.extend([TokenTree::Punct(Punct::new('|', Spacing::Alone))]);
    closure_tokens.extend(closure_param_tokens);
    closure_tokens.extend([TokenTree::Punct(Punct::new('|', Spacing::Alone))]);
    closure_tokens.extend([TokenTree::Group(Group::new(Delimiter::Brace, body_tokens))]);

    let mut result = TokenStream::new();
    result.extend([
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("fn", call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, fn_param_tokens)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
    ]);
    result.extend(closure_tokens);
    result.extend([TokenTree::Punct(Punct::new(';', Spacing::Alone))]);
    result
}

fn render_struct_type(tokens: &[StructTok]) -> TokenStream {
    let mut out = TokenStream::new();
    for tok in tokens {
        match tok {
            StructTok::Ident(id) => out.extend([TokenTree::Ident(id.clone())]),
            StructTok::Sep => out.extend([
                TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            ]),
        }
    }
    out
}

fn render_field_expr(tokens: &[FieldTok]) -> TokenStream {
    let mut out = TokenStream::new();
    for tok in tokens {
        match tok {
            FieldTok::Ident(id) => out.extend([TokenTree::Ident(id.clone())]),
            FieldTok::Dot => out.extend([TokenTree::Punct(Punct::new('.', Spacing::Alone))]),
            FieldTok::Tilde(span) => out.extend(tilde_step_tokens(*span)),
        }
    }
    out
}

/// Every token here stands in for one `~`, so all of them carry the `~`
/// punct's own span: a stepped-into type missing `iter()` is then blamed on
/// the `~` the user wrote, matching where the collection step is spelled in
/// the source, rather than on the whole macro invocation.
fn tilde_step_tokens(span: Span) -> TokenStream {
    fn spanned_dot(span: Span) -> TokenTree {
        let mut dot = Punct::new('.', Spacing::Alone);
        dot.set_span(span);
        TokenTree::Punct(dot)
    }
    fn spanned_call(method: &str, span: Span) -> [TokenTree; 2] {
        let mut group = Group::new(Delimiter::Parenthesis, TokenStream::new());
        group.set_span(span);
        [
            TokenTree::Ident(Ident::new(method, span)),
            TokenTree::Group(group),
        ]
    }

    let mut out = TokenStream::new();
    for method in ["iter", "next", "unwrap"] {
        out.extend([spanned_dot(span)]);
        out.extend(spanned_call(method, span));
    }
    out.extend([spanned_dot(span)]);
    out
}

/// Strips the surrounding quotes from a plain string (`"..."`) or char
/// (`'.'`) literal, keeping the source spelling of any escapes untouched
/// rather than decoding them, so `delim`/`case` values compile to the exact
/// same output they always have. Any other literal kind — numeric, byte
/// string, byte char, raw string — is rejected here: none of them slice into
/// a valid value at this offset, and accepting one used to either panic on
/// the slice bound or hand the raw token text on to generate invalid code.
fn unquote_literal(lit: &proc_macro::Literal) -> Result<String, String> {
    let text = lit.to_string();
    let bytes = text.as_bytes();
    let is_quoted = bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''));
    if is_quoted {
        Ok(text[1..text.len() - 1].to_string())
    } else {
        Err(format!(
            "Unsupported option value {}: expected a string (\"...\") or char ('.') literal",
            text
        ))
    }
}

fn apply_options(options: &HashMap<String, String>, field_path: String) -> Result<String, String> {
    let delim = options
        .get("delim")
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(".");
    let case = options.get("case");
    let segments = field_path
        .split('.')
        .map(|field_name| {
            if let Some(case_value) = case {
                match case_value.as_str() {
                    "camel" => Ok(field_name.from_case(Case::Snake).to_case(Case::Camel)),
                    "pascal" => Ok(field_name.from_case(Case::Snake).to_case(Case::Pascal)),
                    another => Err(format!("Unknown case is specified: {}", another)),
                }
            } else {
                Ok(field_name.to_string())
            }
        })
        .collect::<Result<Vec<String>, String>>()?;
    Ok(segments.join(delim))
}
