// SCRATCH: reader parity check. Delete before push.
#[test]
fn scratch_reader_counts() {
    let dir = "/private/tmp/wt-class-golden/class";
    let mut ok = 0;
    let mut bad = vec![];
    for path in std::fs::read_dir(dir).unwrap().flatten() {
        let p = path.path();
        if p.extension().and_then(|e| e.to_str()) != Some("mmd") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        // grammar gate first, like render()
        if etassia::parse::parse(&src, "class").is_err() {
            bad.push(format!("GATE {}", p.display()));
            continue;
        }
        eprintln!("CASE {}", p.display());
        match etassia::render(&src) {
            Ok(_) => ok += 1,
            Err(e) => bad.push(format!("RENDER {} {e}", p.display())),
        }
    }
    println!("reader ok={ok} bad={:?}", bad.len());
    for b in &bad {
        println!("  {b}");
    }
}


#[test]
fn scratch_format_g() {
    let v = 4.0015f64 + 135.709;
    let r4 = (v * 10000.0).round() / 10000.0;
    println!("r4={r4} g={}", etassia::render::class_puml::format_g_pub(r4));
    println!("132.8955 g={}", etassia::render::class_puml::format_g_pub(132.8955));
}
