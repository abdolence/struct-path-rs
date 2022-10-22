[![Cargo](https://img.shields.io/crates/v/struct-path.svg)](https://crates.io/crates/struct-path)
![tests and formatting](https://github.com/abdolence/struct-path-rs/workflows/tests%20&amp;%20formatting/badge.svg)
![security audit](https://github.com/abdolence/struct-path-rs/workflows/security%20audit/badge.svg)

# struct-path for Rust

Library provides a tiny macro implementation to reference Rust struct fields at compile time to represent its string format.
This is needed to work with JSON paths and others protocols where the serialization like this is needed:

Features:
- Fast and no macro parsing without huge deps;
- Multiple fields/arrays support
- Optional camelCase and PascalCase conversion support;
- Optional delimiter parameter;

## Quick start

Cargo.toml:
```toml
[dependencies]
struct-path = "0.1"
```

Example code:
```rust

use struct_path::*;

pub struct TestStructParent {
    pub value_str: String,
    pub value_num: u64,
    pub value_child: TestStructChild,
}

pub struct TestStructChild {
    pub child_value_str: String,
    pub child_value_num: u64,
}

// returns "value_str"
let s1: &str = path!(TestStructParent::value_str);

// returns "value_child.child_value_str"
let s2: &str = path!(TestStructParent::value_child.child_value_str) ;

// returns ["value_str", "value_num"]
let arr: [&str] = path!(TestStructParent:: { value_str, value_num } );

// options, returns "valueChild/childValueStr"
let s2: &str = path!(TestStructParent::value_child.child_value_str; delim='/', case='camel') ;


```

## Licence
Apache Software License (ASL)

## Author
Abdulla Abdurakhmanov
