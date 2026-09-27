use struct_path::StructPath;

#[derive(StructPath)]
struct TestStruct<T> {
    value: T,
}

fn main() {}
