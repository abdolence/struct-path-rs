[![Cargo](https://img.shields.io/crates/v/struct-path.svg)](https://crates.io/crates/struct-path)
[![tests & formatting](https://github.com/abdolence/struct-path-rs/actions/workflows/tests.yml/badge.svg)](https://github.com/abdolence/struct-path-rs/actions/workflows/tests.yml)
[![security audit](https://github.com/abdolence/struct-path-rs/actions/workflows/security-audit.yml/badge.svg)](https://github.com/abdolence/struct-path-rs/actions/workflows/security-audit.yml)

# struct-path for Rust

Library provides a tiny macro implementation to reference Rust struct fields at compile time to represent its string format.
This is needed to work with JSON paths, and some others protocols when we still want to rely on the compiler to avoid inconsistent changes.

Features:
- Fast parsing without huge deps;
- Macro produces the code to verify if the specified path really exists;
- Multiple fields/arrays support
- Optional camelCase and PascalCase conversion support;
- Optional delimiter parameter;
- Support for `Iter`-based (Option, Vec, etc) paths using `~` delimiter;
- `#[derive(StructPath)]` to return all of a struct's declared fields via `Type::*`, without listing them by hand;
- `Type::*` nested under a field path, `Parent::child.(Child::*)`, to prefix every one of `Child`'s fields with `child`;

## Quick start

Cargo.toml:
```toml
[dependencies]
struct-path = "0.2"
```

Example code:
```rust

use struct_path::*;

pub struct TestStructParent {
    pub value_str: String,
    pub value_num: u64,
    pub value_child: TestStructChild,
    pub opt_value_child: Option<TestStructChild>,
}

pub struct TestStructChild {
    pub child_value_str: String,
    pub child_value_num: u64,
}

// returns "value_str"
let s1: &str = path!(TestStructParent::value_str);

// returns "value_child.child_value_str"
let s2: &str = path!(TestStructParent::value_child.child_value_str) ;

// returns also "value_child.child_value_str"
let s3: &str = path!(TestStructParent::value_child,TestStructChild::child_value_str);

// returns "opt_value_child.child_value_str" using trait `Iter`
let s4: &str = path!(TestStructParent::opt_value_child~child_value_str);

// options, returns "valueChild/childValueStr"
let s5: &str = path!(TestStructParent::value_child.child_value_str; delim="/", case="camel") ;

// returns ["value_str", "value_num"]
let arr: [&str; 2] = paths!(TestStructParent::{ value_str, value_num });


```

## All fields with `Type::*`

`#[derive(StructPath)]` is only needed for `Type::*`; none of the calls above need it.

```rust
use struct_path::*;

#[derive(StructPath)]
pub struct TestStructWithPrivate {
    pub value_str: String,
    value_internal: String,
}

// returns ["value_str"], only the field declared with plain `pub`
let pub_only: [&str; 1] = paths!(TestStructWithPrivate::*);

// returns ["value_str", "value_internal"], every declared field
let all_fields: [&str; 2] = paths!(TestStructWithPrivate::*; visibility="all");

#[derive(StructPath)]
pub struct TestStructChild {
    pub child_value_str: String,
    pub child_value_num: u64,
}

pub struct TestStructParent {
    pub value_child: TestStructChild,
    pub opt_value_child: Option<TestStructChild>,
}

// nested `Type::*`: `TestStructChild` also needs `#[derive(StructPath)]`, and
// is named again inside the parens; returns
// ["value_child.child_value_str", "value_child.child_value_num"]
let nested: [&str; 2] = paths!(TestStructParent::value_child.(TestStructChild::*));

// `~` before the parens steps through the `Option`; returns
// ["opt_value_child.child_value_str", "opt_value_child.child_value_num"]
let nested_opt: [&str; 2] = paths!(TestStructParent::opt_value_child~(TestStructChild::*));

```

The inner type named inside the parens needs `#[derive(StructPath)]` too.

### Lint gates on the derive

`#[derive(StructPath)]` reuses the struct's own visibility for the generated consts:

- a `pub` struct in a private module trips `#![deny(unreachable_pub)]` on those consts too, and an `#[allow]` on the struct does not reach them;
- a struct with its own `impl` block fails `#![deny(clippy::multiple_inherent_impl)]`, since the derive adds one more.

Workaround: declare the struct `pub(crate)`, or add the `#[allow]` at module level instead of on the struct. The derive never emits the `#[allow]` itself, since that would break a crate using `#[forbid(...)]` instead of `#[deny(...)]`.

## Options

- `delim = "<str>"`: sets the path separator, defaults to `.`;
- `case = "camel"` or `case = "pascal"`: converts each segment to camelCase or PascalCase;
- `~`: walks into a type with an `iter()` method, such as `Option` or `Vec`, instead of `.`;
- `visibility = "all"`: with `Type::*`, returns every declared field instead of only the ones declared plain `pub`;
- Several `;` groups are accepted, so a wrapper macro can append its own options; a key repeated across groups takes the last value in a field list and is a compile error with `Type::*`;

## Licence
Apache Software License (ASL)

## Author
Abdulla Abdurakhmanov
