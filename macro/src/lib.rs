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
//! - `#[derive(StructPath)]` to return all of a struct's declared fields via `Type::*`, without listing them by hand;
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
//! #[derive(StructPath)]
//! pub struct TestStructWithPrivate {
//!     pub value_str: String,
//!     value_internal: String,
//! }
//!
//!// `Type::*` needs `#[derive(StructPath)]` on `Type`; returns ["value_str"],
//!// only the field declared with plain `pub`
//!let pub_only: [&str; 1] = paths!(TestStructWithPrivate::*);
//!
//!// `visibility="all"` returns every declared field instead
//!let all_fields: [&str; 2] = paths!(TestStructWithPrivate::*; visibility="all");
//!
//! }
//!
//! ```
//!

use convert_case::{Case, Casing};
use proc_macro::{Delimiter, Group, Spacing, Span, TokenStream, TokenTree};
use std::collections::HashMap;

/// Converts a parse-time error into the token stream for a `compile_error!`
/// invocation, so malformed macro input surfaces as a normal compiler error
/// carrying the same message instead of a proc-macro panic.
fn compile_error_for(message: String) -> TokenStream {
    format!("compile_error!({:?})", message)
        .parse()
        .expect("a compile_error! invocation with an escaped string literal always parses")
}

/// Like `compile_error_for`, but for the derive parser, which walks its own
/// item tokens rather than building a `Result<_, String>` message: the error
/// is spanned on the offending token so it is reported there and not at the
/// derive's call site.
fn compile_error_at(message: &str, span: Span) -> TokenStream {
    let mut literal = proc_macro::Literal::string(message);
    literal.set_span(span);
    let message_tokens = TokenStream::from(TokenTree::Literal(literal));
    fill(
        "compile_error!(MESSAGE);",
        span,
        &[("MESSAGE", &message_tokens)],
    )
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
    if let Some(result) = try_parse_all_fields(&struct_path_stream) {
        return result;
    }

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

/// Recognizes the one shape `*` is allowed in: `Type::*` (optionally
/// `crate::mod::Type::*`), followed by nothing or `; options`. Returns
/// `None` when the input has no top-level `*` at all, so the caller falls
/// through to the ordinary field-list parser unchanged. Any other
/// placement of `*` (mixed with another struct or field group) is an error
/// here rather than a fall-through, because the combined array length
/// would then be unknown to the macro.
fn try_parse_all_fields(struct_path_stream: &TokenStream) -> Option<Result<TokenStream, String>> {
    let tokens: Vec<TokenTree> = struct_path_stream.clone().into_iter().collect();
    let star_pos = tokens
        .iter()
        .position(|t| matches!(t, TokenTree::Punct(p) if *p == '*'))?;
    Some(parse_all_fields_at(&tokens, star_pos))
}

/// The only options `Type::*` recognizes; any other key is a `compile_error!`
/// rather than a silently ignored typo, since there is no field list here for
/// the user to cross-check the result against.
const ALL_FIELDS_OPTION_KEYS: &[&str] = &["visibility", "case"];

/// Reported when the `*` sits beside a second struct or field group, whether
/// before it (`B::y, A::*`) or after (`A::*, B::y`): the combined array's
/// length would then be unknown to the macro.
const CANNOT_COMBINE_STAR_MESSAGE: &str = "`Type::*` cannot be combined with another struct or field group: its length is unknown to the macro";

/// Reported when the type-name position is not a plain `Type::*` /
/// `crate::module::Type::*` path and there is no comma marking a second
/// group either -- generics on the type, a `Type::<'a>::*` turbofish, a
/// `$t:ty` macro fragment, or a stray trailing separator.
const EXPECTED_TYPE_PATH_MESSAGE: &str =
    "expected a type path such as `Type::*` or `crate::module::Type::*`";

fn parse_all_fields_at(tokens: &[TokenTree], star_pos: usize) -> Result<TokenStream, String> {
    if star_pos < 2
        || !matches!(&tokens[star_pos - 1], TokenTree::Punct(p) if *p == ':')
        || !matches!(&tokens[star_pos - 2], TokenTree::Punct(p) if *p == ':')
    {
        return Err("`*` must directly follow `Type::`".to_string());
    }
    let type_tokens = &tokens[..star_pos - 2];
    if type_tokens.is_empty() {
        return Err("Unexpected `*` with an empty type!".to_string());
    }
    if !is_bare_type_path(type_tokens) {
        // A second `Type::field` or `Type::*` group before this one folds
        // into `type_tokens` rather than being rejected outright, since
        // nothing here stops at the group's own `,`; a top-level comma is
        // what tells that shape apart from a type path that is simply
        // malformed on its own (generics, a turbofish, a macro fragment),
        // which gets the other message instead.
        return Err(if contains_top_level_comma(type_tokens) {
            CANNOT_COMBINE_STAR_MESSAGE.to_string()
        } else {
            EXPECTED_TYPE_PATH_MESSAGE.to_string()
        });
    }
    let star_span = match &tokens[star_pos] {
        TokenTree::Punct(p) => p.span(),
        _ => unreachable!(),
    };

    let rest = &tokens[star_pos + 1..];
    let options = match rest {
        [] => Vec::new(),
        [TokenTree::Punct(p), remainder @ ..] if *p == ';' => parse_all_fields_options(remainder)?,
        // A second group after the `*` (`A::*, B::y`) is the same unknown-
        // length combination as one before it; a lone trailing comma with
        // nothing following it is not a second group, just malformed input.
        [TokenTree::Punct(p), more @ ..] if *p == ',' && !more.is_empty() => {
            return Err(CANNOT_COMBINE_STAR_MESSAGE.to_string())
        }
        _ => return Err(EXPECTED_TYPE_PATH_MESSAGE.to_string()),
    };
    if let Some((unknown, _)) = options
        .iter()
        .find(|(key, _)| !ALL_FIELDS_OPTION_KEYS.contains(&key.as_str()))
    {
        return Err(format!("Unknown option is specified: {}", unknown));
    }

    let option_value = |name: &str| {
        options
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let visibility_suffix = match option_value("visibility") {
        None | Some("pub") => "PUB",
        Some("all") => "ALL",
        Some(other) => return Err(format!("Unknown visibility is specified: {}", other)),
    };
    let case_suffix = match option_value("case") {
        None => "",
        Some("camel") => "_CAMEL",
        Some("pascal") => "_PASCAL",
        Some(other) => return Err(format!("Unknown case is specified: {}", other)),
    };

    let const_path = format!(
        "TYPE::__STRUCT_PATH_{}_FIELDS{}",
        visibility_suffix, case_suffix
    );
    let type_stream: TokenStream = type_tokens.iter().cloned().collect();
    Ok(fill(&const_path, star_span, &[("TYPE", &type_stream)]))
}

/// Whether `tokens` contains a `,` at the top level -- the mark of a second
/// struct or field group folded in beside `Type::*`, as opposed to a type
/// path that is simply malformed on its own.
fn contains_top_level_comma(tokens: &[TokenTree]) -> bool {
    tokens
        .iter()
        .any(|t| matches!(t, TokenTree::Punct(p) if *p == ','))
}

/// Reports whether `tokens` is a bare type path -- an optional leading `::`
/// followed by one or more identifiers joined by `::`, and nothing else.
/// `Type::*`'s type name must be exactly this shape; anything wider (a
/// trailing field group, a second `Type::*` group, generics, stray
/// punctuation) is rejected by the caller before it is spliced into the
/// generated code.
fn is_bare_type_path(tokens: &[TokenTree]) -> bool {
    let mut i = 0;
    if matches!(tokens.first(), Some(TokenTree::Punct(p)) if *p == ':')
        && matches!(tokens.get(1), Some(TokenTree::Punct(p)) if *p == ':')
    {
        i = 2;
    }
    loop {
        if !matches!(tokens.get(i), Some(TokenTree::Ident(_))) {
            return false;
        }
        i += 1;
        if i == tokens.len() {
            return true;
        }
        let is_double_colon = matches!(tokens.get(i), Some(TokenTree::Punct(p)) if *p == ':')
            && matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if *p == ':');
        if !is_double_colon {
            return false;
        }
        i += 2;
    }
}

/// Parses the `key = value, key = value, ...` option grammar accepted after
/// `Type::*;`, keeping source order so the caller can report the first
/// unknown key as written rather than in a hash map's arbitrary order. This
/// parser is only for `Type::*`: `path!`/`paths!`'s own option grammar is
/// parsed inline in their own loops, unchanged, because it has a field list
/// to cross-check its result against and a different tolerance for
/// malformed input; `Type::*` has no such field list, so a bare key with no
/// `= value` is always an error here.
fn parse_all_fields_options(tokens: &[TokenTree]) -> Result<Vec<(String, String)>, String> {
    let mut options: Vec<(String, String)> = Vec::new();
    let mut pending_key: Option<String> = None;
    let mut expect_value = false;
    for token_tree in tokens {
        match token_tree {
            TokenTree::Ident(id) if !expect_value => {
                if let Some(key) = pending_key.take() {
                    return Err(format!("Missing a value for option `{}`", key));
                }
                pending_key = Some(id.to_string());
            }
            TokenTree::Punct(p) if *p == '=' && pending_key.is_some() => {
                expect_value = true;
            }
            TokenTree::Ident(id) if expect_value => {
                let key = pending_key.take().expect("set when '=' was seen");
                options.push((key, id.to_string()));
                expect_value = false;
            }
            TokenTree::Literal(lit) if expect_value => {
                let key = pending_key.take().expect("set when '=' was seen");
                options.push((key, unquote_literal(lit)?));
                expect_value = false;
            }
            TokenTree::Punct(p) if *p == ',' && !expect_value => {
                if let Some(key) = pending_key.take() {
                    return Err(format!("Missing a value for option `{}`", key));
                }
            }
            other => {
                return Err(format!(
                    "Unexpected input for struct path parameters: {:?}",
                    other
                ))
            }
        }
    }
    if let Some(key) = pending_key {
        return Err(format!("Missing a value for option `{}`", key));
    }
    Ok(options)
}

#[proc_macro]
pub fn path(struct_path_stream: TokenStream) -> TokenStream {
    match path_impl(struct_path_stream) {
        Ok(stream) => stream,
        Err(message) => compile_error_for(message),
    }
}

fn path_impl(struct_path_stream: TokenStream) -> Result<TokenStream, String> {
    // `Type::*` (all declared fields) has an array result, so only `paths!`
    // can return it; `path!` always returns a single string.
    let has_star = struct_path_stream
        .clone()
        .into_iter()
        .any(|t| matches!(t, TokenTree::Punct(p) if p == '*'));
    if has_star {
        return Err(
            "`*` for all declared fields is only supported by paths!, not path!".to_string(),
        );
    }

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

/// The case conversion shared by `path!`/`paths!`'s `case = "..."` option
/// and `#[derive(StructPath)]`'s `_CAMEL`/`_PASCAL` field-name constants, so
/// the two can never disagree on what "camel" or "pascal" means for a
/// snake_case field name.
fn convert_case_by_name(field_name: &str, case_name: &str) -> Result<String, String> {
    match case_name {
        "camel" => Ok(field_name.from_case(Case::Snake).to_case(Case::Camel)),
        "pascal" => Ok(field_name.from_case(Case::Snake).to_case(Case::Pascal)),
        another => Err(format!("Unknown case is specified: {}", another)),
    }
}

/// Converts every name in `names` with `convert_case_by_name`, for the two
/// known case names the derive itself always passes; a case name it does
/// not recognize is a bug in the derive, not a user input to report.
fn convert_all(names: &[String], case_name: &str) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            convert_case_by_name(n, case_name)
                .unwrap_or_else(|_| panic!("{:?} is always a known case", case_name))
        })
        .collect()
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
        .map(|field_name| match case {
            Some(case_value) => convert_case_by_name(field_name, case_value),
            None => Ok(field_name.to_string()),
        })
        .collect::<Result<Vec<String>, String>>()?;
    Ok(segments.join(delim))
}

/// `#[derive(StructPath)]` records a struct's declared field names as
/// `#[doc(hidden)]` associated consts, so `paths!(Type::*)` can return them
/// without the caller listing every field by hand. Supports only a struct
/// with named fields and, at most, lifetime generic parameters; anything
/// else is a `compile_error!` on the offending item, spanned at the
/// specific token that disqualifies it.
#[proc_macro_derive(StructPath)]
pub fn derive_struct_path(input: TokenStream) -> TokenStream {
    match derive_struct_path_impl(input) {
        Ok(stream) => stream,
        Err(error_stream) => error_stream,
    }
}

/// One field found in a derived struct's body: its declared name, with any
/// `r#` prefix stripped since the generated string is a JSON-path-style
/// segment rather than a Rust identifier, and whether it was declared with
/// plain `pub` — `pub(crate)`, `pub(super)`, `pub(in ..)` and private
/// fields are all "not plain pub".
struct DerivedField {
    name: String,
    is_plain_pub: bool,
}

fn derive_struct_path_impl(input: TokenStream) -> Result<TokenStream, TokenStream> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let mut pos = 0;

    skip_attributes(&tokens, &mut pos);

    let visibility_start = pos;
    let (vis_len, item_is_plain_pub) = parse_visibility_prefix(&tokens[pos..]);
    pos += vis_len;
    let visibility_tokens: TokenStream = tokens[visibility_start..pos].iter().cloned().collect();

    let (keyword, keyword_span) = match tokens.get(pos) {
        Some(TokenTree::Ident(id)) => (id.to_string(), id.span()),
        _ => {
            return Err(compile_error_at(
                "StructPath: unable to parse the derived item",
                Span::call_site(),
            ))
        }
    };
    if keyword != "struct" {
        return Err(compile_error_at(
            &format!(
                "StructPath supports structs with named fields only, not {}s",
                keyword
            ),
            keyword_span,
        ));
    }
    pos += 1;

    let name_ident = match tokens.get(pos) {
        Some(TokenTree::Ident(id)) => id.clone(),
        _ => {
            return Err(compile_error_at(
                "StructPath: expected a struct name",
                keyword_span,
            ))
        }
    };
    let name_span = name_ident.span();
    pos += 1;

    let (generics_impl, generics_type) = parse_lifetime_generics(&tokens, &mut pos)?;
    let (where_clause, fields_group) = find_fields_group(&tokens[pos..])?;
    let fields = parse_derived_fields(fields_group.stream())?;

    let pub_names: Vec<String> = fields
        .iter()
        .filter(|f| f.is_plain_pub)
        .map(|f| f.name.clone())
        .collect();
    let all_names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();

    let name_tokens = TokenStream::from(TokenTree::Ident(name_ident));
    let pub_array = array_literal_tokens(&pub_names);
    let pub_camel_array = array_literal_tokens(&convert_all(&pub_names, "camel"));
    let pub_pascal_array = array_literal_tokens(&convert_all(&pub_names, "pascal"));
    let all_array = array_literal_tokens(&all_names);
    let all_camel_array = array_literal_tokens(&convert_all(&all_names, "camel"));
    let all_pascal_array = array_literal_tokens(&convert_all(&all_names, "pascal"));
    let n_pub: TokenStream = pub_names
        .len()
        .to_string()
        .parse()
        .expect("a field count always parses as a literal");
    let n_all: TokenStream = all_names
        .len()
        .to_string()
        .parse()
        .expect("a field count always parses as a literal");

    // `__STRUCT_PATH_ALL_FIELDS*` exposes every declared field's name,
    // private ones included. A plain `pub` struct's consts are capped to
    // `pub(crate)`, so a downstream crate cannot read private field names
    // through them and adding a private field is not a breaking change to a
    // `pub` array type. Any other struct visibility -- private, `pub(crate)`,
    // `pub(super)`, `pub(in ..)` -- reuses the struct's own visibility tokens
    // instead, so these consts are never wider than the struct they describe.
    // The `__STRUCT_PATH_PUB_FIELDS*` consts only ever list fields already
    // visible outside the crate, so they keep the struct's own visibility
    // unconditionally.
    let all_fields_visibility: TokenStream = if item_is_plain_pub {
        "pub(crate)".parse().expect("`pub(crate)` always parses")
    } else {
        visibility_tokens.clone()
    };

    let generated = fill(
        "impl<GENERICS_IMPL> NAME<GENERICS_TYPE> WHERE_CLAUSE {
            #[doc(hidden)]
            VIS const __STRUCT_PATH_PUB_FIELDS: [&'static str; N_PUB] = PUB_FIELDS;
            #[doc(hidden)]
            VIS const __STRUCT_PATH_PUB_FIELDS_CAMEL: [&'static str; N_PUB] = PUB_FIELDS_CAMEL;
            #[doc(hidden)]
            VIS const __STRUCT_PATH_PUB_FIELDS_PASCAL: [&'static str; N_PUB] = PUB_FIELDS_PASCAL;
            #[doc(hidden)]
            ALL_VIS const __STRUCT_PATH_ALL_FIELDS: [&'static str; N_ALL] = ALL_FIELDS;
            #[doc(hidden)]
            ALL_VIS const __STRUCT_PATH_ALL_FIELDS_CAMEL: [&'static str; N_ALL] = ALL_FIELDS_CAMEL;
            #[doc(hidden)]
            ALL_VIS const __STRUCT_PATH_ALL_FIELDS_PASCAL: [&'static str; N_ALL] = ALL_FIELDS_PASCAL;
        }",
        name_span,
        &[
            ("GENERICS_IMPL", &generics_impl),
            ("GENERICS_TYPE", &generics_type),
            ("NAME", &name_tokens),
            ("WHERE_CLAUSE", &where_clause),
            ("VIS", &visibility_tokens),
            ("ALL_VIS", &all_fields_visibility),
            ("N_PUB", &n_pub),
            ("N_ALL", &n_all),
            ("PUB_FIELDS", &pub_array),
            ("PUB_FIELDS_CAMEL", &pub_camel_array),
            ("PUB_FIELDS_PASCAL", &pub_pascal_array),
            ("ALL_FIELDS", &all_array),
            ("ALL_FIELDS_CAMEL", &all_camel_array),
            ("ALL_FIELDS_PASCAL", &all_pascal_array),
        ],
    );
    Ok(generated)
}

/// Skips a run of `#[...]` attributes (including `#[doc = "..."]` doc
/// comments), advancing `pos` past each one.
fn skip_attributes(tokens: &[TokenTree], pos: &mut usize) {
    while matches!(tokens.get(*pos), Some(TokenTree::Punct(p)) if *p == '#') {
        *pos += 1;
        if matches!(tokens.get(*pos), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket)
        {
            *pos += 1;
        }
    }
}

/// Reports how many raw tokens at the front of `tokens` are a `pub`/
/// `pub(...)` visibility prefix -- 0 for private, 1 for plain `pub`, 2 for
/// `pub(crate)`/`pub(super)`/`pub(in ..)` -- and whether that prefix is
/// plain `pub`. A `$v:vis`/`$fv:vis` macro_rules fragment reaches the derive
/// re-emitted as a `Group(Delimiter::None)`, empty when the fragment matched
/// private and containing the literal `pub`/`pub(...)` tokens otherwise;
/// this unwraps exactly one such group before classifying, and reports its
/// prefix length as the single outer token regardless of what was inside.
fn parse_visibility_prefix(tokens: &[TokenTree]) -> (usize, bool) {
    if let Some(TokenTree::Group(g)) = tokens.first() {
        if g.delimiter() == Delimiter::None {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            let (_, is_plain_pub) = parse_visibility_prefix(&inner);
            return (1, is_plain_pub);
        }
    }
    if matches!(tokens.first(), Some(TokenTree::Ident(id)) if id.to_string() == "pub") {
        if matches!(tokens.get(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
        {
            return (2, false);
        }
        return (1, true);
    }
    (0, false)
}

/// Advances `pos` past a field's visibility and reports whether it was
/// plain `pub`: `pub(crate)`, `pub(super)`, `pub(in ..)` and private fields
/// all return `false`, since none of them are visible outside the crate
/// the way plain `pub` is.
fn parse_field_visibility(tokens: &[TokenTree], pos: &mut usize) -> bool {
    let (len, is_plain_pub) = parse_visibility_prefix(&tokens[*pos..]);
    *pos += len;
    is_plain_pub
}

/// Splits `tokens` on commas that are not inside `<...>`, so a field's
/// type, e.g. `HashMap<String, u64>`, keeps its own comma out of the split.
/// Angle brackets are plain `Punct` tokens here, not a delimited `Group`, so
/// depth has to be tracked explicitly. A trailing comma produces no empty
/// trailing segment. Shared by generic-parameter and struct-field splitting.
///
/// The `>` of a `->` return-type arrow is not a closing angle bracket, so it
/// must not decrement `depth`; it is told apart from a real one by the `-`
/// immediately before it being tokenized `Joint` (no space before this `>`),
/// which is otherwise only true of operators Rust's grammar never places
/// right before `>`. Depth is also floored at 0, so a field list with no
/// generics at all (where an arrow's `>` would otherwise be the only `<`/`>`
/// seen) cannot be pushed negative and starve every comma split after it.
fn split_top_level_commas(tokens: &[TokenTree]) -> Vec<Vec<TokenTree>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    let mut depth: i32 = 0;
    let mut prev_was_arrow_dash = false;
    for token in tokens {
        match token {
            TokenTree::Punct(p) if *p == '<' => {
                depth += 1;
                current.push(token.clone());
            }
            TokenTree::Punct(p) if *p == '>' => {
                if !prev_was_arrow_dash {
                    depth = (depth - 1).max(0);
                }
                current.push(token.clone());
            }
            TokenTree::Punct(p) if *p == ',' && depth == 0 => {
                segments.push(std::mem::take(&mut current));
            }
            other => current.push(other.clone()),
        }
        prev_was_arrow_dash =
            matches!(token, TokenTree::Punct(p) if *p == '-' && p.spacing() == Spacing::Joint);
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

/// Recognizes a single generic parameter as a lifetime, optionally with
/// lifetime bounds (`'a`, `'a:`, or `'a: 'b + 'c`), and returns just the
/// bare `'a` tokens for use in the derived impl's `Name<'a>`. Returns `None`
/// for a type or const parameter, which `StructPath` does not support.
fn lifetime_param_name(segment: &[TokenTree]) -> Option<Vec<TokenTree>> {
    if segment.len() < 2 {
        return None;
    }
    if !matches!(&segment[0], TokenTree::Punct(p) if *p == '\'') {
        return None;
    }
    if !matches!(&segment[1], TokenTree::Ident(_)) {
        return None;
    }
    let bare = segment[0..2].to_vec();
    if segment.len() == 2 {
        return Some(bare);
    }
    if !matches!(&segment[2], TokenTree::Punct(p) if *p == ':') {
        return None;
    }
    if segment.len() == 3 {
        // `'a:` with no bound after the colon -- valid Rust, equivalent to
        // no colon at all.
        return Some(bare);
    }
    let mut i = 3;
    loop {
        if i + 1 >= segment.len() {
            return None;
        }
        let is_lifetime = matches!(&segment[i], TokenTree::Punct(p) if *p == '\'')
            && matches!(&segment[i + 1], TokenTree::Ident(_));
        if !is_lifetime {
            return None;
        }
        i += 2;
        if i == segment.len() {
            break;
        }
        if !matches!(&segment[i], TokenTree::Punct(p) if *p == '+') {
            return None;
        }
        i += 1;
    }
    Some(bare)
}

/// Parses the optional `<...>` generic parameter list starting at `*pos`,
/// advancing `pos` past it, and returns the tokens for `impl<GENERICS_IMPL>`
/// and the bare lifetime names for `Name<GENERICS_TYPE>`. Both are empty
/// when the struct has no generics.
fn parse_lifetime_generics(
    tokens: &[TokenTree],
    pos: &mut usize,
) -> Result<(TokenStream, TokenStream), TokenStream> {
    if !matches!(tokens.get(*pos), Some(TokenTree::Punct(p)) if *p == '<') {
        return Ok((TokenStream::new(), TokenStream::new()));
    }
    let open_span = match &tokens[*pos] {
        TokenTree::Punct(p) => p.span(),
        _ => unreachable!(),
    };
    *pos += 1;
    let start = *pos;
    let mut depth = 1;
    while depth > 0 {
        match tokens.get(*pos) {
            Some(TokenTree::Punct(p)) if *p == '<' => depth += 1,
            Some(TokenTree::Punct(p)) if *p == '>' => depth -= 1,
            Some(_) => {}
            None => {
                return Err(compile_error_at(
                    "StructPath: unterminated generic parameter list",
                    open_span,
                ))
            }
        }
        *pos += 1;
    }
    let generics_tokens = &tokens[start..*pos - 1];

    let mut bare_lifetimes: Vec<TokenTree> = Vec::new();
    for (i, segment) in split_top_level_commas(generics_tokens)
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            bare_lifetimes.extend(fill(",", open_span, &[]));
        }
        match lifetime_param_name(&segment) {
            Some(bare) => bare_lifetimes.extend(bare),
            None => {
                let span = segment.first().map(|t| t.span()).unwrap_or(open_span);
                return Err(compile_error_at(
                    "StructPath only supports lifetime generic parameters",
                    span,
                ));
            }
        }
    }

    let generics_impl: TokenStream = generics_tokens.iter().cloned().collect();
    let generics_type: TokenStream = bare_lifetimes.into_iter().collect();
    Ok((generics_impl, generics_type))
}

/// Finds the struct's field body in `remaining` (everything after its name
/// and generics) and returns the `where` clause tokens in front of it, if
/// any, alongside it. The body's shape is read off the item's *last* token:
/// a `{ ... }` group for named fields (returned with any `where` clause
/// before it), or a trailing `;` for a tuple or unit struct (both rejected,
/// since only named fields have names to record). Deciding this from the
/// end, rather than scanning for the first `(...)` or `{...}` seen, is what
/// keeps a `(...)` written inside the `where` clause itself (e.g.
/// `where (): Sized`) from being mistaken for a tuple-struct body: nothing
/// before the item's own last token can be that body.
fn find_fields_group(remaining: &[TokenTree]) -> Result<(TokenStream, Group), TokenStream> {
    match remaining.last() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
            let where_clause: TokenStream =
                remaining[..remaining.len() - 1].iter().cloned().collect();
            Ok((where_clause, g.clone()))
        }
        Some(TokenTree::Punct(p)) if *p == ';' => match remaining.first() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                Err(compile_error_at(
                    "StructPath supports structs with named fields only, not tuple structs",
                    g.span(),
                ))
            }
            _ => Err(compile_error_at(
                "StructPath supports structs with named fields only, not unit structs",
                p.span(),
            )),
        },
        _ => Err(compile_error_at(
            "StructPath: expected a struct body with named fields",
            Span::call_site(),
        )),
    }
}

/// Parses a named-field struct body into its field names and plain-`pub`
/// flags, in declaration order. A field's type is skipped rather than
/// parsed: the derive never needs to reference a field's value, only its
/// name, so the type tokens (however they nest, e.g. `HashMap<String,
/// u64>`) are dropped once the name before the first top-level `:` is
/// found.
fn parse_derived_fields(fields_stream: TokenStream) -> Result<Vec<DerivedField>, TokenStream> {
    let tokens: Vec<TokenTree> = fields_stream.into_iter().collect();
    let mut fields = Vec::new();
    for segment in split_top_level_commas(&tokens) {
        let mut pos = 0;
        skip_attributes(&segment, &mut pos);
        let is_plain_pub = parse_field_visibility(&segment, &mut pos);
        let name_ident = match segment.get(pos) {
            Some(TokenTree::Ident(id)) => id,
            _ => {
                return Err(compile_error_at(
                    "StructPath: expected a field name",
                    segment
                        .first()
                        .map(|t| t.span())
                        .unwrap_or_else(Span::call_site),
                ))
            }
        };
        let name = name_ident.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name).to_string();
        fields.push(DerivedField { name, is_plain_pub });
    }
    Ok(fields)
}

/// Builds a `[&'static str; N]` initializer literal from field-name
/// strings, which are always valid identifiers (or their camelCase/
/// PascalCase conversions), so no escaping beyond wrapping each one in
/// quotes is needed.
fn array_literal_tokens(values: &[String]) -> TokenStream {
    let joined = values
        .iter()
        .map(|v| format!("\"{}\"", v))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", joined)
        .parse()
        .expect("field names always produce a valid array literal")
}
