//! Native multiline typography corpus, including whitespace-only notes.
use excaliplot::Scene;
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("note_gallery", || {
        let mut scene = Scene::new();
        for (index, text) in [
            "AV\nCafé −2",
            "\nAV",
            "AV\n\nTo",
            "AV\n",
            "\r\nAV\r\n\r\nTo\r\n",
            "\n\n",
            ".\n", // Blank-line space is wider than this short visible line.
            "  AV  \n To ",
        ]
        .into_iter()
        .enumerate()
        {
            scene.add_note(text, (-12.5, index as f64 * 160.), 20.)?;
        }
        Ok(scene)
    })
}
