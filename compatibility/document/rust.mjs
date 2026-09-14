import { execFileSync } from "node:child_process";
import path from "node:path";

// Dagger supplies prebuilt helpers; local and legacy CI runs can still use Cargo.
export function runRust(repo, example, args = [], input) {
  const options = { cwd: repo, encoding: "utf8", input: input === undefined ? undefined : JSON.stringify(input) };
  const directory = process.env.DOCUMENT_EXAMPLES_DIR;
  if (directory) {
    return JSON.parse(execFileSync(path.resolve(directory, example), args, options));
  }
  const cargo = ["run", "--quiet", "--locked", "-p", "excalidraw-document"];
  if (example === "embedded") cargo.push("--features", "embedded");
  return JSON.parse(execFileSync("cargo", [...cargo, "--example", example, "--", ...args], options));
}
