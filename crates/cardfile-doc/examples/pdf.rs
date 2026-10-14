//! Render a Markdown file to PDF: `cargo run -p cardfile-doc --example pdf -- in.md out.pdf`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("usage: pdf <in.md> <out.pdf>")?;
    let output = args.next().ok_or("usage: pdf <in.md> <out.pdf>")?;
    let doc = cardfile_doc::from_markdown(&std::fs::read_to_string(input)?);
    let bytes = cardfile_doc::pdf::to_pdf(&doc, "Sample card", &cardfile_doc::NoBlobs)?;
    std::fs::write(output, bytes)?;
    Ok(())
}
