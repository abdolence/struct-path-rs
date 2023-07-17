[![Cargo](https://img.shields.io/crates/v/struct-path.svg)](https://crates.io/crates/struct-path)
![tests and formatting](https://github.com/abdolence/struct-path-rs/workflows/tests%20&amp;%20formatting/badge.svg)
![security audit](https://github.com/abdolence/struct-path-rs/workflows/security%20audit/badge.svg)

# struct-path for Rust

Library provides a tiny macro implementation to reference Rust struct fields at compile time to represent its string format.
This is needed to work with JSON paths, and some others protocols when we still want to rely on the compiler to avoid inconsistent changes.

Features:
- Fast and no macro parsing without huge deps;
- Macro produces the code to verify if the specified path really exists;
- Multiple fields/arrays support
- Optional camelCase and PascalCase conversion support;
- Optional delimiter parameter;
- Support for `Iter`-based (Option, Vec, etc) paths using `~` delimiter;

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
    pub opt_value_str: Option<TestStructChild>,
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

// returns also "value_child.child_value_str" using trait `Iter`
let s3: &str = path!(TestStructParent::opt_value_child~child_value_str);

// options, returns "valueChild/childValueStr"
let s4: &str = path!(TestStructParent::value_child.child_value_str; delim="/", case="camel") ;

// returns ["value_str", "value_num"]
let arr: [&str; 2] = paths!(TestStructParent::{ value_str, value_num });


```

## Licence
Apache Software License (ASL)

## Author
Abdulla Abdurakhmanov
