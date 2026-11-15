use std::path::PathBuf;

fn example() -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../docs/dim/examples/flowchart-as-dim.json");
    std::fs::read_to_string(p).expect("the docs example is part of the repository")
}

#[test]
fn parses_the_specification_example() {
    let doc = etassia_dim::parse(&example()).expect("the example must parse");
    assert_eq!(doc.dim, "0.1");
    assert_eq!(doc.notation, "generic");
    assert_eq!(doc.graph.children.len(), 5);
}

#[test]
fn renders_the_example_to_svg() {
    let svg = etassia_dim::render(&example()).expect("render");
    assert!(svg.starts_with("<svg"));
    for id in ["a", "b", "c", "d", "e"] {
        assert!(
            svg.contains(&format!("<g id=\"{id}\">")),
            "node {id} missing"
        );
    }
    assert!(svg.contains(">Text<"), "the edge label must render");
    // layered RIGHT: the decision node sits in the middle layer
    let cx = svg.matches("<g id=\"c\">").count();
    assert_eq!(cx, 1);
}

#[test]
fn output_is_byte_deterministic() {
    let a = etassia_dim::render(&example()).unwrap();
    let b = etassia_dim::render(&example()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn golden_output_is_stable() {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/golden/flowchart-as-dim.svg");
    let golden = std::fs::read_to_string(&p).expect("the pinned golden exists");
    assert_eq!(etassia_dim::render(&example()).unwrap(), golden);
}

#[test]
fn wrong_version_is_unsupported() {
    let err = etassia_dim::parse(r#"{"dim":"9.9","notation":"g","graph":{"id":"r"}}"#)
        .expect_err("version mismatch must fail");
    assert!(err.to_string().contains("9.9"));
}

#[test]
fn hierarchical_documents_are_rejected_in_this_release() {
    let doc = r#"{"dim":"0.1","notation":"g","graph":{"id":"r","children":[{"id":"p","children":[{"id":"q"}]}]}}"#;
    let err = etassia_dim::parse(doc).expect_err("hierarchy must fail");
    assert!(err.to_string().contains("hierarchical"));
}

#[test]
fn estimated_size_from_content() {
    let doc = r#"{"dim":"0.1","notation":"g","graph":{"id":"r","children":[{"id":"n","content":[{"kind":"label","text":"A very long member line here","role":"member"}]}]}}"#;
    let parsed = etassia_dim::parse(doc).unwrap();
    let (w, h) = etassia_dim::estimated_size(&parsed.graph.children[0]);
    // 28 chars * 7.2 + 24 padding, one line * 18 + 16
    assert_eq!(w, 225.6);
    assert_eq!(h, 34.0);
}
