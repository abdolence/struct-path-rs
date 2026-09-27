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
use proc_macro::{Delimiter, Group, Span, TokenStream, TokenTree};
use std::collections::HashMap;

/// Converts a parse-time error into the token stream for a `compile_error!`
/// invocation, so malformed macro input surfaces as a normal compiler error
/// carrying the same message instead of a proc-macro panic.
fn compile_error_for(message: String) -> TokenStream {
    format!("compile_error!({:?})", message)
        .parse()
        .expect("a compile_error! invocation with an escaped string literal always parses")
}

/// The type path and field paths found for one `Type::field[, field...]`
/// group, kept as the user's own tokens so the generated check reports a
/// type error on the exact token that is wrong. A field path holds its
/// idents, `.` and `~` puncts as written; it is the only record kept of the
/// path, so the check and the returned string can never disagree.
type FoundStructs = Vec<(Vec<TokenTree>, Vec<Vec<TokenTree>>)>;

#[proc_macro]
pub fn paths(struct_path_stream: TokenStream) -> TokenStream {
    match paths_impl(struct_path_stream) {
        Ok(stream) => stream,
        Err(message) => compile_error_for(message),
    }
}

fn paths_impl(struct_path_stream: TokenStream) -> Result<TokenStream, String> {
    let mut current_struct_name_tokens: Vec<TokenTree> = Vec::new();
    let mut current_struct_fields: Vec<Vec<TokenTree>> = Vec::with_capacity(16);

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_tokens: Vec<TokenTree> = Vec::new();

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: FoundStructs = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name_tokens.is_empty() => {
                current_struct_name_tokens.push(TokenTree::Ident(id));
            }
            TokenTree::Punct(punct)
                if !current_struct_name_tokens.is_empty()
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
                current_field_tokens.push(TokenTree::Ident(id));
            }
            TokenTree::Punct(punct)
                if !current_struct_name_tokens.is_empty()
                    && opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                opened_struct = false;
                let field_tokens = std::mem::take(&mut current_field_tokens);
                if !fold_field_into_struct_name(field_tokens, &mut current_struct_name_tokens) {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            TokenTree::Punct(punct) if opened_struct && (punct == '.' || punct == '~') => {
                if current_field_tokens.is_empty() {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
                current_field_tokens.push(TokenTree::Punct(punct));
            }
            TokenTree::Group(group) if opened_struct && current_field_tokens.is_empty() => {
                parse_multiple_fields(group.stream(), &mut current_struct_fields)?
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if current_struct_name_tokens.is_empty() {
                    return Err("Unexpected comma with empty definitions!".to_string());
                }
                if !current_field_tokens.is_empty() {
                    current_struct_fields.push(std::mem::take(&mut current_field_tokens));
                }
                if current_struct_fields.is_empty() {
                    return Err(format!(
                        "Unexpected comma with empty fields for {}!",
                        render_text(&current_struct_name_tokens)
                    ));
                }
                found_structs.push((
                    std::mem::take(&mut current_struct_name_tokens),
                    std::mem::take(&mut current_struct_fields),
                ));
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

    if !current_field_tokens.is_empty() {
        current_struct_fields.push(std::mem::take(&mut current_field_tokens));
    }
    if current_struct_name_tokens.is_empty() {
        return Err("Unexpected comma with empty definitions!".to_string());
    }
    if current_struct_fields.is_empty() {
        return Err(format!(
            "Unexpected comma with empty fields for {}!",
            render_text(&current_struct_name_tokens)
        ));
    }
    found_structs.push((current_struct_name_tokens, current_struct_fields));

    let all_check_functions = generate_checks_code_for(&found_structs);

    let mut all_final_fields: Vec<String> = Vec::with_capacity(16);

    for (_, fields) in &found_structs {
        for field_tokens in fields {
            let mut final_field_path = render_text(field_tokens).replace('~', ".");
            if !options.is_empty() {
                final_field_path = apply_options(&options, final_field_path)?;
            }
            all_final_fields.push(format!("\"{}\"", final_field_path))
        }
    }

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
}

/// Reclassifies a field-path segment that turned out to be a further
/// `::`-qualified part of the type (`tests` in `crate::tests::Struct::field`)
/// back into the type path. Returns `false` when the pending segment is not
/// exactly one identifier, which has no valid representation as a type-path
/// segment; the caller then reports the input as malformed instead of
/// building a check around it.
fn fold_field_into_struct_name(
    field_tokens: Vec<TokenTree>,
    struct_name_tokens: &mut Vec<TokenTree>,
) -> bool {
    let is_single_ident = matches!(field_tokens.as_slice(), [TokenTree::Ident(_)]);
    if is_single_ident {
        struct_name_tokens.extend("::".parse::<TokenStream>().expect("`::` always parses"));
        struct_name_tokens.extend(field_tokens);
    }
    is_single_ident
}

/// Renders parsed path tokens back to their text as written, `~` included,
/// so a caller replacing it with a delimiter still sees each collection step.
fn render_text(tokens: &[TokenTree]) -> String {
    tokens.iter().map(|token| token.to_string()).collect()
}

fn parse_multiple_fields(
    group_stream: TokenStream,
    found_struct_fields: &mut Vec<Vec<TokenTree>>,
) -> Result<(), String> {
    let mut current_field_tokens: Vec<TokenTree> = Vec::new();

    for token_tree in group_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) => current_field_tokens.push(TokenTree::Ident(id)),
            TokenTree::Punct(punct) if punct == ',' => {
                if current_field_tokens.is_empty() {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
                found_struct_fields.push(std::mem::take(&mut current_field_tokens));
            }
            TokenTree::Punct(punct) if punct == '.' => {
                if current_field_tokens.is_empty() {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
                current_field_tokens.push(TokenTree::Punct(punct));
            }
            others => {
                return Err(format!(
                    "Unexpected input for struct path group parameters: {:?}",
                    others
                ));
            }
        }
    }

    if !current_field_tokens.is_empty() {
        found_struct_fields.push(current_field_tokens);
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
    let mut current_struct_name_tokens: Vec<TokenTree> = Vec::new();

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_tokens: Vec<TokenTree> = Vec::new();

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: FoundStructs = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name_tokens.is_empty() => {
                current_struct_name_tokens.push(TokenTree::Ident(id));
            }
            TokenTree::Punct(punct)
                if !current_struct_name_tokens.is_empty()
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
                current_field_tokens.push(TokenTree::Ident(id));
            }
            TokenTree::Punct(punct)
                if !current_struct_name_tokens.is_empty()
                    && opened_struct
                    && punct == ':'
                    && colons_counter < 2 =>
            {
                colons_counter += 1;
                opened_struct = false;
                let field_tokens = std::mem::take(&mut current_field_tokens);
                if !fold_field_into_struct_name(field_tokens, &mut current_struct_name_tokens) {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
            }
            TokenTree::Punct(punct) if opened_struct && (punct == '.' || punct == '~') => {
                if current_field_tokens.is_empty() {
                    return Err(format!(
                        "Unexpected punctuation input for struct path group parameters: {:?}",
                        punct
                    ));
                }
                current_field_tokens.push(TokenTree::Punct(punct));
            }
            TokenTree::Punct(punct) if !options_opened && opened_struct && punct == ',' => {
                opened_struct = false;
                colons_counter = 0;
                if current_struct_name_tokens.is_empty() {
                    return Err("Unexpected comma with empty definitions!".to_string());
                }
                if current_field_tokens.is_empty() {
                    return Err(format!(
                        "Unexpected comma with empty fields for {}!",
                        render_text(&current_struct_name_tokens)
                    ));
                }
                found_structs.push((
                    std::mem::take(&mut current_struct_name_tokens),
                    vec![std::mem::take(&mut current_field_tokens)],
                ));
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

    if !current_struct_name_tokens.is_empty() && !current_field_tokens.is_empty() {
        found_structs.push((current_struct_name_tokens, vec![current_field_tokens]));
    }

    if found_structs.is_empty() {
        return Err("Unexpected empty path definition!".to_string());
    }

    let all_check_functions = generate_checks_code_for(&found_structs);
    // `~` marks a collection step and must become the path separator before
    // options are applied, or a segment joined by `~` is treated as one
    // field name instead of two and never gets the chosen delim or
    // per-segment case conversion.
    let full_field_path = found_structs
        .iter()
        .flat_map(|(_, fields)| fields.iter())
        .map(|tokens| render_text(tokens))
        .collect::<Vec<_>>()
        .join(".")
        .replace('~', ".");
    let final_field_path = apply_options(&options, full_field_path)?;
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
}

/// Builds the `const _: fn(&Type) = |t: &Type| { let _ = &t.field; };`
/// check for every discovered `Type::field` pair. The user's type and field
/// tokens keep their spans, so a type error is reported on the exact token
/// that is wrong; each `~` becomes `.iter().next().unwrap().` spanned at that
/// `~`, so a stepped-into type without `iter()` is blamed on the `~`.
fn generate_checks_code_for(found_structs: &FoundStructs) -> TokenStream {
    let mut all_check_functions = TokenStream::new();
    for (struct_name_tokens, struct_fields) in found_structs {
        let struct_name: TokenStream = struct_name_tokens.iter().cloned().collect();
        for field_tokens in struct_fields {
            let field_path: TokenStream = field_tokens
                .iter()
                .map(|token| match token {
                    TokenTree::Punct(punct) if *punct == '~' => {
                        fill(".iter().next().unwrap().", punct.span(), &[])
                    }
                    token => token.clone().into(),
                })
                .collect();
            all_check_functions.extend(fill(
                "const _: fn(&STRUCT) = |t: &STRUCT| { let _ = &t.FIELD; };",
                Span::call_site(),
                &[("STRUCT", &struct_name), ("FIELD", &field_path)],
            ));
        }
    }
    all_check_functions
}

/// Parses `template` into tokens spanned at `span`, replacing each ident
/// named in `substitutions` with the given tokens, which keep their own
/// spans. `template` must be valid Rust tokens.
fn fill(template: &str, span: Span, substitutions: &[(&str, &TokenStream)]) -> TokenStream {
    fn walk(stream: TokenStream, span: Span, subs: &[(&str, &TokenStream)]) -> TokenStream {
        stream
            .into_iter()
            .map(|mut token| {
                match &token {
                    TokenTree::Ident(ident) => {
                        let name = ident.to_string();
                        if let Some((_, tokens)) = subs.iter().find(|(n, _)| *n == name) {
                            return (*tokens).clone();
                        }
                    }
                    TokenTree::Group(group) => {
                        let stream = walk(group.stream(), span, subs);
                        token = TokenTree::Group(Group::new(group.delimiter(), stream));
                    }
                    _ => {}
                }
                token.set_span(span);
                token.into()
            })
            .collect()
    }
    walk(
        template.parse().expect("templates are valid tokens"),
        span,
        substitutions,
    )
}

/// Strips the surrounding quotes from a plain string (`"..."`) or char
/// (`'.'`) literal. Escapes keep their source spelling because the value is
/// pasted back into a string literal in the generated code; decoding them
/// would change the output. Any other literal kind (numeric, byte string,
/// byte char, raw string) is rejected, since its text has no such quotes to
/// strip and would produce invalid generated code.
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
