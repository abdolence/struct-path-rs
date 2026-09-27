use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStruct<'a> {
    value_str: &'a str,
}

fn main() {
    let _ = paths!(TestStruct<'static>::*);
}
