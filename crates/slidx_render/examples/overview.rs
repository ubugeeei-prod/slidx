//! Renders a slides directory's overview page, for `scripts/overview.mjs`.

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("a slides directory");
    let out = args.next().expect("an output file");

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the slides directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();

    let source = files
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("a slide file"))
        .collect::<Vec<_>>()
        .join("\n---\n\n");

    let deck = slidx_core::parse_deck(&source, &slidx_core::DeckParseOptions::default());
    let html =
        slidx_render::overview::render_overview(&deck, &slidx_render::ShellOptions::default());

    std::fs::write(&out, html).expect("writing the overview");
    println!("{} slides -> {out}", deck.slides.len());
}
