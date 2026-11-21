//! Formats the files given on the command line and prints the result.
//! `cargo run -p weavejig_formatter --example fmt -- File.java`

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("readable file");
        match weavejig_formatter::format(&text, &Default::default()) {
            Ok(formatted) => print!("{formatted}"),
            Err(error) => eprintln!("{path}: {error}"),
        }
    }
}
