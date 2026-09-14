use std::process::Command;

#[test]
fn runnable_mark_examples_emit_native_artifacts_and_protect_edited_files() {
    let dir = tempfile::tempdir().unwrap();
    for (name, kind, count) in [("bars", "rectangle", 4), ("scatter", "ellipse", 6)] {
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
                .filter(|e| e["type"] == kind)
                .count(),
            count
        );
        std::fs::write(&destination, b"manual edit").unwrap();
        assert!(!run(false).status.success());
        assert_eq!(std::fs::read(&destination).unwrap(), b"manual edit");
        assert!(run(true).status.success());
    }
}
