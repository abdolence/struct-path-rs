use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Node {
    value: u64,
    next: Option<Box<Node>>,
}

impl Node {
    fn names(&self) -> [&'static str; 2] {
        paths!(Node::next~(Self::*))
    }
}

fn main() {}
