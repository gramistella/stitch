// benches/stitch_bench.rs
use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use walkdir::WalkDir;

use stitch::core::{
    clean_remove_regex, collapse_consecutive_blank_lines, compile_remove_regex_opt,
    parse_extension_filters, parse_hierarchy_text, path_to_unix, render_unicode_tree_from_paths,
    scan_dir_to_node, split_prefix_list, strip_lines_and_inline_comments,
};

// ---------- Fixture: synthetic repo tree we reuse across benches ----------
static FS_FIXTURE: Lazy<Fixture> = Lazy::new(|| {
    let tmp = TempDir::new().expect("tmp");
    let root = tmp.path().to_path_buf();

    // Create directories
    let dirs = &[
        "src", "src/codec", "src/ui", "tests", "examples", "vendor/dep1", "vendor/dep2",
        "assets/images", "assets/fonts", "scripts", "src/gen",
    ];
    for d in dirs {
        fs::create_dir_all(root.join(d)).unwrap();
    }

    // Seed files
    let files = [
        ("src/lib.rs", "pub mod core;"),
        ("src/core.rs", "fn main() {}"),
        ("src/ui/app.rs", "mod ui;"),
        ("tests/core_tests.rs", "/* tests */"),
        ("examples/demo.rs", "// demo"),
        ("scripts/build.sh", "#!/usr/bin/env bash\n# comment\n echo hi // inline"),
        ("assets/fonts/JetBrainsMono-Regular.ttf", ""),
        ("vendor/dep1/lib.c", "int main(){}"),
        ("vendor/dep2/lib.cpp", "int main(){}"),
        ("README.md", "# readme\n"),
    ];
    for (rel, body) in files {
        write_file(&root.join(rel), body);
    }

    // Generate many small files to stress scan/render
    for i in 0..1200 {
        write_file(&root.join(format!("src/gen/file_{i:04}.rs")), "fn f(){}\n");
    }

    // Collect file list
    let all_files: Vec<PathBuf> = WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    Fixture { _tmp: tmp, root, all_files }
});

struct Fixture {
    _tmp: TempDir,       // keep alive
    root: PathBuf,
    all_files: Vec<PathBuf>,
}

fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

// ---------- Benches ----------

fn bench_relative_paths(c: &mut Criterion) {
    let fx = &*FS_FIXTURE;
    let root = fx.root.as_path();

    let mut g = c.benchmark_group("relative_path");
    g.sample_size(50);
    g.measurement_time(Duration::from_secs(4));

    // strip_prefix (current code)
    g.bench_function("strip_prefix", |b| {
        b.iter(|| {
            let mut count = 0usize;
            for p in fx.all_files.iter() {
                let rel = p.strip_prefix(root).unwrap();
                let s = rel
                    .iter()
                    .map(|c| c.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                count = count.wrapping_add(s.len());
            }
            black_box(count)
        });
    });

    // Optional: compare to pathdiff if you enable the feature
    #[cfg(feature = "bench_pathdiff")]
    g.bench_function("pathdiff::diff_paths", |b| {
        b.iter(|| {
            let mut count = 0usize;
            for p in fx.all_files.iter() {
                let rel = pathdiff::diff_paths(p, root).unwrap();
                let s = rel
                    .iter()
                    .map(|c| c.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                count = count.wrapping_add(s.len());
            }
            black_box(count)
        });
    });

    g.finish();
}

fn bench_scan_dir_to_node(c: &mut Criterion) {
    let fx = &*FS_FIXTURE;

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = [".lock".into()].into_iter().collect();
    let exclude_dirs: HashSet<String> = ["vendor".into(), "target".into(), "node_modules".into()]
        .into_iter()
        .collect();
    let exclude_files: HashSet<String> = ["LICENSE".into(), "Cargo.lock".into()].into_iter().collect();

    c.bench_function("scan_dir_to_node", |b| {
        b.iter_batched(
            || (),
            |_| {
                let node = scan_dir_to_node(
                    fx.root.as_path(),
                    &include,
                    &exclude_exts,
                    &exclude_dirs,
                    &exclude_files,
                );
                black_box(node);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_render_tree(c: &mut Criterion) {
    let fx = &*FS_FIXTURE;
    let root = fx.root.as_path();

    let mut rels: Vec<String> = fx
        .all_files
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok())
        .map(|r| r.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join("/"))
        .collect();

    rels.sort();
    rels.dedup();

    c.bench_function("render_unicode_tree_from_paths", |b| {
        b.iter(|| {
            let s = render_unicode_tree_from_paths(black_box(&rels), Some("project"));
            black_box(s);
        })
    });
}

fn bench_strip_and_collapse(c: &mut Criterion) {
    // Synthetic source with lots of candidates
    let src = {
        let line = "let url = \"http://x\";  // keep inside\nname = 'path // keep'\nvalue = 1   # strip me\n";
        line.repeat(20_000) // ~2–3MB
    };
    let prefixes = split_prefix_list("#,//,--");

    let mut g = c.benchmark_group("text_cleanup");
    g.sample_size(20);
    g.measurement_time(Duration::from_secs(10));
    g.warm_up_time(Duration::from_secs(3));

    g.throughput(Throughput::Bytes(src.len() as u64));
    g.bench_function("strip_lines_and_inline_comments", |b| {
        b.iter(|| {
            let out = strip_lines_and_inline_comments(black_box(&src), black_box(&prefixes));
            black_box(out);
        })
    });

    // collapse on the (unchanged) src to isolate just collapse cost
    g.throughput(Throughput::Bytes(src.len() as u64));
    g.bench_function("collapse_consecutive_blank_lines", |b| {
        b.iter(|| {
            let out = collapse_consecutive_blank_lines(black_box(&src));
            black_box(out);
        })
    });

    g.finish();
}

fn bench_remove_regex(c: &mut Criterion) {
    // Biggish text with markers to remove
    let src = "PRE\nSTART\nmiddle\nEND\nPOST\n".repeat(200_000);
    let cleaned = clean_remove_regex("\"START.*?END\""); // ensure same path as app
    let re = compile_remove_regex_opt(Some(&cleaned)).unwrap();

    let mut g = c.benchmark_group("remove_regex");
    g.sample_size(25);
    g.measurement_time(Duration::from_secs(8));

    g.throughput(Throughput::Bytes(src.len() as u64));
    g.bench_function("regex_replace", |b| {
        b.iter(|| {
            let out = re.replace_all(black_box(&src), "");
            black_box(out);
        })
    });

    g.finish();
}

fn bench_tokenization(c: &mut Criterion) {
    let snippet = r#"fn main() { println!("hello // not a comment # here"); }"#;
    let sizes = [256 * 1024, 1 * 1024 * 1024, 3 * 1024 * 1024];
    let bpe = tiktoken_rs::o200k_base().expect("load o200k_base");

    let mut g = c.benchmark_group("tokenize_o200k_base_with_specials");
    g.sample_size(20);
    g.measurement_time(Duration::from_secs(15));
    g.warm_up_time(Duration::from_secs(3));

    for &bytes in &sizes {
        let text = snippet.repeat(bytes / snippet.len().max(1));
        g.throughput(Throughput::Bytes(text.len() as u64));
        g.bench_with_input(BenchmarkId::from_parameter(format!("{}KB", bytes / 1024)), &text, |b, txt| {
            b.iter(|| {
                let tokens = bpe.encode_with_special_tokens(black_box(txt));
                black_box(tokens.len())
            });
        });
    }

    g.finish();
}

fn bench_hierarchy_parse_render(c: &mut Criterion) {
    // Build a tree text via renderer, then parse it back
    let paths: Vec<String> = (0..500)
        .map(|i| format!("src/pkg_{:03}/mod_{:03}.rs", i % 50, i))
        .collect();
    let tree = render_unicode_tree_from_paths(&paths, Some("root"));

    let mut g = c.benchmark_group("hierarchy_roundtrip");
    g.sample_size(40);
    g.measurement_time(Duration::from_secs(5));

    g.bench_function("parse_hierarchy_text", |b| {
        b.iter(|| {
            let set = parse_hierarchy_text(black_box(&tree)).unwrap();
            black_box(set);
        })
    });

    // Also test extension filter parsing (fast path sanity)
    g.bench_function("parse_extension_filters", |b| {
        b.iter(|| {
            let (_inc, _exc) = parse_extension_filters(black_box(".rs, .md, -.lock, -png, js, -.tmp"));
        })
    });

    g.finish();
}

criterion_group!(
    benches,
    bench_relative_paths,
    bench_scan_dir_to_node,
    bench_render_tree,
    bench_strip_and_collapse,
    bench_remove_regex,
    bench_tokenization,
    bench_hierarchy_parse_render
);
criterion_main!(benches);
