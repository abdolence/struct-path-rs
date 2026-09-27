use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Parent {
    child: Child,
}

fn main() {
    let _ = paths!(Parent::child.(Child::*); visibility = "all", ;);
}
