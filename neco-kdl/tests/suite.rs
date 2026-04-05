use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// KDL 公式テストスイートのパース成功/失敗テスト。
#[test]
fn kdl_official_test_suite_parse() {
    let input_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kdl-test-cases/input");

    let mut pass = 0;
    let mut fail = 0;
    let mut errors = Vec::new();
    let mut hangs = Vec::new();

    let mut files: Vec<_> = fs::read_dir(&input_dir)
        .expect("input dir should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("kdl"))
        .collect();
    files.sort();

    let known_skip: [&str; 0] = [];
    let mut skipped = 0;

    for path in &files {
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        if known_skip.contains(&stem.as_str()) {
            skipped += 1;
            continue;
        }
        let should_fail = stem.ends_with("_fail");
        let input = fs::read_to_string(path).unwrap();

        // 各ファイルをスレッドで実行してタイムアウト
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = neco_kdl::parse(&input);
            let _ = tx.send(result.map(|_| ()).map_err(|e| format!("{e}")));
        });

        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => {
                if should_fail {
                    errors.push(format!("SHOULD FAIL but parsed OK: {stem}"));
                } else {
                    pass += 1;
                }
            }
            Ok(Err(e)) => {
                if should_fail {
                    fail += 1;
                } else {
                    errors.push(format!("SHOULD PASS but failed: {stem} — {e}"));
                }
            }
            Err(_) => {
                hangs.push(stem.clone());
            }
        }
    }

    eprintln!(
        "pass: {pass}, fail (expected): {fail}, errors: {}, hangs: {}, skipped: {skipped}",
        errors.len(),
        hangs.len()
    );
    for e in &errors {
        eprintln!("  {e}");
    }
    for h in &hangs {
        eprintln!("  HANG: {h}");
    }
    let total_problems = errors.len() + hangs.len();
    if total_problems > 0 {
        panic!(
            "{} error(s) + {} hang(s) = {} total problem(s)",
            errors.len(),
            hangs.len(),
            total_problems
        );
    }
}

/// 公式テストスイートの正規化出力比較テスト。
///
/// `input/` の各ファイルをパースし、`normalize()` で正規化した結果を
/// `expected_kdl/` の対応ファイルと比較する。
#[test]
fn kdl_official_test_suite_normalize() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kdl-test-cases");
    let expected_dir = base_dir.join("expected_kdl");
    let input_dir = base_dir.join("input");

    let mut expected_files: Vec<PathBuf> = fs::read_dir(&expected_dir)
        .expect("expected_kdl dir should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("kdl"))
        .collect();
    expected_files.sort();

    let mut pass = 0;
    let mut errors = Vec::new();

    for expected_path in &expected_files {
        let stem = expected_path.file_stem().unwrap().to_str().unwrap();
        let input_path = input_dir.join(format!("{stem}.kdl"));

        if !input_path.exists() {
            continue;
        }

        let input = fs::read_to_string(&input_path).unwrap();
        let expected = fs::read_to_string(expected_path).unwrap();

        let doc = match neco_kdl::parse(&input) {
            Ok(doc) => doc,
            Err(e) => {
                errors.push(format!("PARSE FAIL: {stem} — {e}"));
                continue;
            }
        };

        let normalized = neco_kdl::normalize(&doc);

        if normalized == expected {
            pass += 1;
        } else {
            let mut diff = String::new();
            diff.push_str(&format!("  MISMATCH: {stem}\n"));
            diff.push_str(&format!("    expected: {:?}\n", expected));
            diff.push_str(&format!("    got:      {:?}\n", normalized));
            errors.push(diff);
        }
    }

    eprintln!(
        "normalize: pass: {pass}, errors: {}, total: {}",
        errors.len(),
        pass + errors.len()
    );
    for e in &errors[..errors.len().min(10)] {
        eprintln!("{e}");
    }
    if errors.len() > 10 {
        eprintln!("  ... and {} more", errors.len() - 10);
    }
    if !errors.is_empty() {
        panic!("{} normalize mismatch(es)", errors.len());
    }
}
