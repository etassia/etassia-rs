fn main() {
    let example = std::fs::read_to_string("docs/dim/examples/flowchart-as-dim.json").unwrap();
    let svg = etassia_dim::render(&example).unwrap();
    std::fs::write("crates/etassia-dim/tests/golden/flowchart-as-dim.svg", svg).unwrap();
    println!("golden written");
}
