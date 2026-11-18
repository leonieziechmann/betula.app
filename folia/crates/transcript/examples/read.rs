//! What a Notenübersicht is read as, without a browser: `cargo run -p folia-transcript --example read
//! -- <file.pdf>` prints the lines the Web Worker answers; with `--runs` every piece of text and
//! every rule of its pages, as `pdf::content` reads them. Nothing is written anywhere.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let runs = args.iter().any(|arg| arg == "--runs");
    let Some(path) = args.iter().find(|arg| !arg.starts_with("--")) else {
        eprintln!("usage: read [--runs] <file.pdf>");
        std::process::exit(2);
    };
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{path}: {error}");
            std::process::exit(1);
        }
    };
    if runs {
        let document = match folia_transcript::pdf::Document::open(&data) {
            Ok(document) => document,
            Err(refusal) => {
                eprintln!("{path}: {refusal:?}");
                std::process::exit(1);
            }
        };
        for (number, page) in document.pages().iter().enumerate() {
            let page = folia_transcript::pdf::content::read(&document, page);
            println!("page {}", number + 1);
            for rule in &page.rules {
                println!("  rule y {:7.2}  x {:7.2} – {:7.2}", rule.y, rule.x0, rule.x1);
            }
            for run in &page.runs {
                let style = match (run.bold, run.italic) {
                    (true, true) => "BI",
                    (true, false) => "B ",
                    (false, true) => " I",
                    (false, false) => "  ",
                };
                println!("  text y {:7.2}  x {:7.2} – {:7.2}  {:4.1} {style} {:?}", run.y, run.x, run.end, run.size, run.text);
            }
        }
        return;
    }
    match folia_transcript::read(&data) {
        Ok(transcript) => print!("{}", transcript.lines()),
        Err(error) => {
            eprintln!("{path}: {}", error.code());
            std::process::exit(1);
        }
    }
}
