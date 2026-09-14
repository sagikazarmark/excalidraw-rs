use std::process::Command;

#[test]
fn filled_examples_produce_native_paths_and_protect_manual_edits() {
    let dir = tempfile::tempdir().unwrap();
    for (name, count) in [("area", 1), ("pie", 3), ("donut", 3)] {
        let destination = dir.path().join(format!("{name}.excalidraw"));
        let run = |overwrite: bool| {
            let mut command = Command::new(env!("CARGO"));
            command.args(["run", "--locked", "--example", name, "--"]);
            if overwrite {
                command.arg("--overwrite");
            }
            command.arg(&destination).output().unwrap()
        };
        let output = run(false);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
        assert_eq!(
            doc["elements"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["type"] == "line" && e["backgroundColor"] != "transparent")
                .count(),
            count
        );
        std::fs::write(&destination, b"manual edit").unwrap();
        assert!(!run(false).status.success());
        assert_eq!(std::fs::read(&destination).unwrap(), b"manual edit");
        assert!(run(true).status.success());
    }
}
