mod eval;
mod fetch;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fetch-models") if args.len() == 1 => fetch::run(),
        Some("eval-ocr") => eval::run(&args[1..]),
        _ => Err("usage: cargo xtask <fetch-models|eval-ocr>".into()),
    }
}
