use std::process::exit;

mod executor;
mod lexer;
mod parser;

fn proba_error(message: &str) -> ! {
    println!("Prost error: {message}");
    exit(0)
}

fn main() {
    let test_file_path = "test.proba";
    let tokens = parser::parse_file(test_file_path.into());
    // dbg!(&tokens);

    let tree = lexer::lex(tokens);
    // dbg!(&tree);

    let mut state = executor::State::standard();
    let result = executor::execute(&mut state, tree);
    // dbg!(&state);
    dbg!(&result);
}
