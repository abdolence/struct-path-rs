use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Parent {
    o: Option<Child>,
}

fn main() {
    let _ = paths!(Parent::o~.(Child::*));
}
