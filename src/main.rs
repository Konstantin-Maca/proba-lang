use std::process::exit;

mod executor;
mod lexer;
mod parser;

fn proba_error(message: &str) -> ! {
    println!("Proba error: {message}");
    exit(0)
}

fn main() {
    let test_file_path = "test.proba";
    let tokens = parser::parse_file(test_file_path.into());

    let tree = lexer::lex(tokens);

    let mut state = executor::State::standard();
    let result = executor::execute(&mut state, tree);
    // dbg!(&state);
    dbg!(&result);
}
