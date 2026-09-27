use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStruct<'a, 'b> {
    value_str: &'a str,
    value_other: &'b str,
}

fn main() {
    let _ = paths!(TestStruct<'a, 'b>::*);
}
