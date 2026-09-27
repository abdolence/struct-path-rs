use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Foo {
    value: u64,
}

struct Node {
    next: Foo,
}

impl Node {
    fn names(&self) -> [&'static str; 1] {
        paths!(Node::next.(Self::Foo::*))
    }
}

fn main() {}
