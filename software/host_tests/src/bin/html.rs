use std::process::Command;

fn main() {
    let coverage = Command::new("cargo")
        .args([
            "llvm-cov",
            "--test",
            "actors",
            "--html",
            // Host helpers only; imported firmware is recorded under host_tests/src/../../test_bench.
            "--ignore-filename-regex",
            "/host_tests/src/(bin/|lib[.]rs$)",
        ])
        .status()
        .expect("failed to run cargo llvm-cov");
    assert!(coverage.success(), "coverage generation failed");

    let report_path = Command::new("wslpath")
        .args(["-w", "target/llvm-cov/html/index.html"])
        .output()
        .expect("failed to convert report path for Windows");
    assert!(report_path.status.success(), "wslpath failed");

    let report_path = String::from_utf8(report_path.stdout).expect("invalid report path");
    let opened = Command::new("cmd.exe")
        .args(["/c", "start", "", report_path.trim()])
        .current_dir("/mnt/c/Windows")
        .status()
        .expect("failed to launch report in Windows browser");
    assert!(opened.success(), "failed to launch report in Windows browser");
}