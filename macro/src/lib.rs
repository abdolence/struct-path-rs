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
enum StructTok {
    Ident(Ident),
    Sep,
}

/// One token of a parsed field path (`value_child.child_value_str` or
/// `opt_value_child~child_value_str`). `Tilde` keeps the original `~`
/// punct's span so that a stepped-into type missing `iter()` is blamed on
/// the `~` the user wrote, not on the whole macro invocation. This is the
/// only record kept of a field path: both the type-check and the macro's
/// returned string are rendered from it, so the two can never disagree.
enum FieldTok {
    Ident(Ident),
    Dot,
    Tilde(Span),
}

/// The type path and field paths found for one `Type::field[, field...]`
/// group.
type FoundStructs = Vec<(Vec<StructTok>, Vec<Vec<FieldTok>>)>;

#[proc_macro]
pub fn paths(struct_path_stream: TokenStream) -> TokenStream {
    match paths_impl(struct_path_stream) {
        Ok(stream) => stream,
        Err(message) => compile_error_for(message),
    }
}

fn paths_impl(struct_path_stream: TokenStream) -> Result<TokenStream, String> {
    let mut current_struct_name_tokens: Vec<StructTok> = Vec::new();
    let mut current_struct_fields: Vec<Vec<FieldTok>> = Vec::with_capacity(16);

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_tokens: Vec<FieldTok> = Vec::new();

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: FoundStructs = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name_tokens.is_empty() => {
                current_struct_name_tokens.push(StructTok::Ident(id));
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
                current_field_tokens.push(FieldTok::Ident(id));
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
                current_field_tokens.push(if punct == '~' {
                    FieldTok::Tilde(punct.span())
                } else {
                    FieldTok::Dot
                });
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
                        render_struct_name(&current_struct_name_tokens)
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
            render_struct_name(&current_struct_name_tokens)
        ));
    }
    found_structs.push((current_struct_name_tokens, current_struct_fields));

    let all_check_functions = generate_checks_code_for(&found_structs);

    let mut all_final_fields: Vec<String> = Vec::with_capacity(16);

    for (_, fields) in &found_structs {
        for field_tokens in fields {
            let mut final_field_path = render_field_text(field_tokens).replace('~', ".");
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
    field_tokens: Vec<FieldTok>,
    struct_name_tokens: &mut Vec<StructTok>,
) -> bool {
    let mut iter = field_tokens.into_iter();
    match (iter.next(), iter.next()) {
        (Some(FieldTok::Ident(id)), None) => {
            struct_name_tokens.push(StructTok::Sep);
            struct_name_tokens.push(StructTok::Ident(id));
            true
        }
        _ => false,
    }
}

/// Renders a parsed type path back to text, for diagnostics only; the
/// tokens are the only representation kept while parsing.
fn render_struct_name(tokens: &[StructTok]) -> String {
    let mut out = String::new();
    for tok in tokens {
        match tok {
            StructTok::Ident(id) => out.push_str(&id.to_string()),
            StructTok::Sep => out.push_str("::"),
        }
    }
    out
}

/// Renders a parsed field path back to its text as written, `~` included, so
/// a caller replacing it with a delimiter still sees each collection step.
fn render_field_text(tokens: &[FieldTok]) -> String {
    let mut out = String::new();
    for tok in tokens {
        match tok {
            FieldTok::Ident(id) => out.push_str(&id.to_string()),
            FieldTok::Dot => out.push('.'),
            FieldTok::Tilde(_) => out.push('~'),
        }
    }
    out
}

fn parse_multiple_fields(
    group_stream: TokenStream,
    found_struct_fields: &mut Vec<Vec<FieldTok>>,
) -> Result<(), String> {
    let mut current_field_tokens: Vec<FieldTok> = Vec::new();

    for token_tree in group_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) => current_field_tokens.push(FieldTok::Ident(id)),
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
                current_field_tokens.push(FieldTok::Dot);
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
    let mut current_struct_name_tokens: Vec<StructTok> = Vec::new();

    let mut opened_struct = false;
    let mut colons_counter = 0;
    let mut options_opened = false;

    let mut current_field_tokens: Vec<FieldTok> = Vec::new();

    let mut current_option_name: Option<String> = None;
    let mut expect_option_value: bool = false;

    let mut options: HashMap<String, String> = HashMap::new();
    let mut found_structs: FoundStructs = Vec::new();

    for token_tree in struct_path_stream.into_iter() {
        match token_tree {
            TokenTree::Ident(id) if current_struct_name_tokens.is_empty() => {
                current_struct_name_tokens.push(StructTok::Ident(id));
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
                current_field_tokens.push(FieldTok::Ident(id));
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
                current_field_tokens.push(if punct == '~' {
                    FieldTok::Tilde(punct.span())
                } else {
                    FieldTok::Dot
                });
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
                        render_struct_name(&current_struct_name_tokens)
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
        .map(|tokens| render_field_text(tokens))
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
/// check for every discovered `Type::field` pair. Type-path and field
/// identifiers keep the user's own spans, so a type error is reported on
/// the exact token that is wrong; `~` is expanded to
/// `.iter().next().unwrap().` at the `~` token's span; everything else in
/// the check (`const`, `_`, `fn`, `t`, `let`) has no counterpart in the
/// user's input and stays at `Span::call_site()`.
fn generate_checks_code_for(found_structs: &FoundStructs) -> TokenStream {
    let mut output = TokenStream::new();
    for (struct_tokens, fields) in found_structs {
        for field_tokens in fields {
            output.extend(build_check_tokens(struct_tokens, field_tokens));
        }
    }
    output
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
