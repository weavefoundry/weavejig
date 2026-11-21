//! Prints the syntax tree of Java source read from standard input.
//! `cargo run -p weavejig-syntax --example tree < File.java`

use std::io::Read;

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("readable input");
    let parse = weavejig_syntax::parse(&text);
    print!("{}", parse.debug_tree());
    for error in parse.errors() {
        eprintln!("{error:?}");
    }
}
