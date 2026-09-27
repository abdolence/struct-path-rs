use struct_path::paths;

struct T {
    a: String,
}

fn main() {
    let _ = paths!(T::(a, *));
}
