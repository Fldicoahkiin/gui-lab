//! Every WGSL source the renderer compiles keeps the expression rules the WGSL grammar sets and naga does
//! not enforce, which Chrome's compiler (Tint) does: `||` and `&&` not mixed without parentheses; a bitwise
//! `&`, `|` or `^` only between unary expressions (not mixed with each other, with arithmetic, a comparison,
//! a shift or a logical operator); a shift only between unary expressions (no arithmetic beside it, no
//! second shift); and no comparison of a comparison (`a < b < c`, `a == b == c`). naga took
//! `x && (a || b && c)` and Tint threw out the module, so every pass using it was invalid in Chrome.
//!
//! The sources: the material pass's shader as the renderer builds it (`materials::shader`, every built-in
//! material in it); its specialised and gated passes and its override variants are pipelines of the same
//! module with override constants (`MAT`, `ONLY`), so the same text; the mesh pass; the particles and
//! their simulation.
//!
//! A small token scanner, not a parser: each run of an expression between separators at one depth of
//! brackets is checked, type template lists (`vec2<f32>`, `array<f32, 4>`, `bitcast<u32>`) skipped.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op { Or, And, BitAnd, BitOr, BitXor, Shift, Rel, Arith }

#[derive(Debug, PartialEq)]
enum Tok { Operand, Op(Op, &'static str), Open, Close, Sep, Other }

/// Words a `<` after which opens a template list, not a comparison.
fn templated(w: &str) -> bool {
    matches!(w, "var" | "vec2" | "vec3" | "vec4" | "array" | "ptr" | "atomic" | "bitcast" | "binding_array")
        || w.starts_with("mat") && w.len() == 6 && w.as_bytes()[4] == b'x'
        || w.starts_with("texture_")
}

/// The source without comments.
fn strip(src: &str) -> String {
    let (b, mut out, mut i) = (src.as_bytes(), String::with_capacity(src.len()), 0);
    while i < b.len() {
        if b[i..].starts_with(b"//") { while i < b.len() && b[i] != b'\n' { i += 1; } continue; }
        if b[i..].starts_with(b"/*") {
            let mut depth = 0;
            while i < b.len() {
                if b[i..].starts_with(b"/*") { depth += 1; i += 2; } else if b[i..].starts_with(b"*/") { depth -= 1; i += 2; if depth == 0 { break; } } else { if b[i] == b'\n' { out.push('\n'); } i += 1; }
            }
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

/// The tokens of `src` (no comments), each with its line.
fn tokens(src: &str) -> Vec<(Tok, usize)> {
    let b = src.as_bytes();
    let (mut out, mut i, mut line) = (vec![], 0, 1);
    let operand = |out: &Vec<(Tok, usize)>| matches!(out.last(), Some((Tok::Operand | Tok::Close, _)));
    while i < b.len() {
        let c = b[i];
        if c == b'\n' { line += 1; i += 1; continue; }
        if c.is_ascii_whitespace() { i += 1; continue; }
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') { i += 1; }
            let w = &src[s..i];
            // A template list: skipped whole, nested ones in it.
            let mut j = i;
            while j < b.len() && b[j] == b' ' { j += 1; }
            if templated(w) && j < b.len() && b[j] == b'<' {
                let mut depth = 0;
                while j < b.len() {
                    match b[j] { b'<' => depth += 1, b'>' => { depth -= 1; if depth == 0 { j += 1; break; } } b'\n' => line += 1, _ => {} }
                    j += 1;
                }
                i = j;
            }
            // Keywords that begin a statement end the expression before them.
            out.push((if matches!(w, "return" | "let" | "var" | "const" | "if" | "else" | "for" | "while" | "loop" | "switch" | "case" | "default" | "fn" | "struct" | "override" | "break" | "continue" | "discard") { Tok::Sep } else { Tok::Operand }, line));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let hex = b[i..].starts_with(b"0x") || b[i..].starts_with(b"0X");
            while i < b.len() {
                let d = b[i];
                let exp = (d == b'-' || d == b'+') && !hex && matches!(b[i - 1], b'e' | b'E');
                if d.is_ascii_alphanumeric() || d == b'.' || exp { i += 1; } else { break; }
            }
            out.push((Tok::Operand, line));
            continue;
        }
        let rest = &src[i..];
        const THREE: [&str; 2] = ["<<=", ">>="];
        const TWO: [&str; 19] = ["||", "&&", "<<", ">>", "<=", ">=", "==", "!=", "->", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^="];
        const ONE: [&str; 16] = ["<", ">", "&", "|", "^", "+", "-", "*", "/", "%", "(", "[", ")", "]", ",", ";"];
        // As a `'static` name of itself where the rules name it.
        let t: &'static str = match THREE.iter().chain(&TWO).chain(&ONE).find(|t| rest.starts_with(**t)) { Some(t) => t, None => if rest.as_bytes()[0] == b'{' { "{" } else if rest.as_bytes()[0] == b'}' { "}" } else if rest.as_bytes()[0] == b':' { ":" } else if rest.as_bytes()[0] == b'=' { "=" } else if rest.as_bytes()[0] == b'@' { "@" } else { "?" } };
        let len = if t == "?" { rest.chars().next().map_or(1, char::len_utf8) } else { t.len() };
        let bin = operand(&out);
        let tok = match t {
            "||" => Tok::Op(Op::Or, "||"),
            "&&" => Tok::Op(Op::And, "&&"),
            "<<" | ">>" => Tok::Op(Op::Shift, t),
            "<" | ">" | "<=" | ">=" | "==" | "!=" => Tok::Op(Op::Rel, t),
            "&" if bin => Tok::Op(Op::BitAnd, "&"),
            "|" => Tok::Op(Op::BitOr, "|"),
            "^" => Tok::Op(Op::BitXor, "^"),
            "+" | "-" | "*" | "/" | "%" if bin => Tok::Op(Op::Arith, t),
            "(" | "[" => Tok::Open,
            ")" | "]" => Tok::Close,
            "++" | "--" => Tok::Other,
            "," | ";" | "{" | "}" | ":" | "=" | "->" | "@" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" => Tok::Sep,
            // Unary `-`, `!`, `~`, `&`, `*`, a `.` of a member.
            _ => Tok::Other,
        };
        out.push((tok, line));
        i += len;
    }
    out
}

/// What breaks the rules in one run of an expression at one depth: its binary operators in order.
fn check_run(ops: &[(Op, &'static str)]) -> Option<String> {
    let has = |o: Op| ops.iter().any(|x| x.0 == o);
    let names = || ops.iter().map(|x| x.1).collect::<Vec<_>>().join(" ");
    if has(Op::Or) && has(Op::And) { return Some(format!("|| and && mixed without parentheses ({})", names())); }
    for b in [Op::BitAnd, Op::BitOr, Op::BitXor] {
        if has(b) && ops.iter().any(|x| x.0 != b) { return Some(format!("a bitwise operator beside another operator without parentheses ({})", names())); }
    }
    // Between the logical operators: one comparison at most, of shift expressions or sums.
    for part in ops.split(|x| matches!(x.0, Op::Or | Op::And)) {
        if part.iter().filter(|x| x.0 == Op::Rel).count() > 1 { return Some(format!("a comparison of a comparison ({})", names())); }
        for side in part.split(|x| x.0 == Op::Rel) {
            let shifts = side.iter().filter(|x| x.0 == Op::Shift).count();
            if shifts > 1 || (shifts == 1 && side.len() > 1) { return Some(format!("a shift beside arithmetic or another shift without parentheses ({})", names())); }
        }
    }
    None
}

/// Each place in `src` that breaks the rules, as its line and what.
fn scan(src: &str) -> Vec<(usize, String)> {
    let src = strip(src);
    let toks = tokens(&src);
    // A stack of open runs: their operators and the line each began on.
    let mut stack: Vec<(Vec<(Op, &'static str)>, usize)> = vec![(vec![], 1)];
    let mut bad = vec![];
    let end = |run: (Vec<(Op, &'static str)>, usize), bad: &mut Vec<(usize, String)>| { if let Some(w) = check_run(&run.0) { bad.push((run.1, w)); } };
    for (t, line) in toks {
        match t {
            Tok::Op(o, n) => { let top = stack.last_mut().unwrap(); if top.0.is_empty() { top.1 = line; } top.0.push((o, n)); }
            Tok::Open => stack.push((vec![], line)),
            Tok::Close => { if stack.len() > 1 { let r = stack.pop().unwrap(); end(r, &mut bad); } }
            Tok::Sep => { let top = stack.last_mut().unwrap(); let r = std::mem::replace(top, (vec![], line)); end(r, &mut bad); }
            Tok::Operand | Tok::Other => {}
        }
    }
    for r in stack { end(r, &mut bad); }
    bad
}

#[test]
fn the_scanner_finds_what_tint_rejects_and_passes_what_it_takes() {
    // The line Chrome rejected (fs_ink, before its fix), and its fix.
    let was = "if (i.look.x >= 127.5 && (s.y > select(0.02, 0.42, i.paint.w < 0.5) || i.paint.w < 0.5 && display_override(i.size, i.look.x))) { code = i.look.x / 255.0; }";
    let fixed = "if (i.look.x >= 127.5 && (s.y > select(0.02, 0.42, i.paint.w < 0.5) || (i.paint.w < 0.5 && display_override(i.size, i.look.x)))) { code = i.look.x / 255.0; }";
    assert_eq!(scan(was).len(), 1, "{:?}", scan(was));
    assert!(scan(fixed).is_empty(), "{:?}", scan(fixed));
    for bad in ["let a = x || y && z;", "let a = x < y && z >= w || !q;", "let a = (x & 1u) | y & 2u;", "let a = x & y + 1u;", "let a = x & 1u == 0u;", "let b = c && x & 1u;", "let a = x << 1u + y;",
        "let a = x + y << 2u;", "let a = x << 1u << 2u;", "let a = p < q < r;", "let a = p == q != r;", "return a || b && c;", "f(a, b || c && d);", "x = v[a && b || c];"] {
        assert!(!scan(bad).is_empty(), "not found: {bad}");
    }
    for good in ["let a = (x || y) && z;", "let a = x || (y && z);", "let a = (x < y && z >= w) || !q;", "let a = (x & 1u) | (y & 2u);", "let a = x & y & z;", "let a = (x << 1u) + y;",
        "let a = x << 1u < y;", "let a: vec2<f32> = vec2<f32>(a < b, c > d);", "var<private> m: array<vec4<f32>, 4>;", "let a = x < y && z >= w && !q;", "let p = &v; let q = *p + 1.0;",
        "for (var i = 0; i < n && ok; i++) { s += -x * 2.0; }", "let e = 1e-3 + 2.5e+2 - 0x1fu;", "fn f(a: ptr<function, f32>) -> vec3<f32> { return vec3<f32>(*a); }",
        "let t = textureSample(tex, smp, uv).r > 0.5 || a;", "let b = bitcast<u32>(x) & 0xffu;", "@group(0) @binding(1) var t: texture_2d<f32>;"] {
        assert!(scan(good).is_empty(), "{good}: {:?}", scan(good));
    }
}

#[test]
fn every_shipped_wgsl_source_keeps_the_rules_tint_enforces() {
    let sources = [
        ("the material pass (materials::shader)", amenui::materials::shader()),
        ("mesh.wgsl", include_str!("../src/mesh.wgsl").to_owned()),
        ("particles.wgsl", include_str!("../src/particles.wgsl").to_owned()),
        ("particles.wgsl + sim_cs.wgsl", format!("{}\n{}", include_str!("../src/particles.wgsl"), include_str!("../src/sim_cs.wgsl"))),
    ];
    let mut all = vec![];
    for (name, src) in &sources {
        let lines: Vec<&str> = src.lines().collect();
        for (line, what) in scan(src) { all.push(format!("{name}:{line}: {what}\n    {}", lines.get(line - 1).map_or("", |l| l.trim()))); }
    }
    assert!(all.is_empty(), "WGSL that Chrome's compiler rejects:\n{}", all.join("\n"));
    // Each built-in material is in the material pass's source.
    for m in ["plain", "dottr", "halftonr", "charr", "smudgr", "carvr", "glitchr", "near", "scribblr"] {
        assert!(sources[0].1.contains(&format!("fn m_{m}(")), "the material pass has no m_{m}");
    }
}
