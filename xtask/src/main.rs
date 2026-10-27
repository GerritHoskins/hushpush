mod corpus_eval;
mod eval;
mod fetch;
mod fuzz;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fetch-models") if args.len() == 1 => fetch::run(),
        Some("eval-ocr") => eval::run(&args[1..]),
        Some("eval") => corpus_eval::run(&args[1..]),
        Some("fuzz") => fuzz::run(&args[1..]),
        _ => Err("usage: cargo xtask <fetch-models|eval-ocr|eval|fuzz>".into()),
    }
}
