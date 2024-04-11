mod executor;
mod lexer;
mod parser;

fn main() {
    let test_file_path = "test.proba";
    let tokens = parser::parse_file(test_file_path.into());
    // dbg!(&tokens);

    let tree = lexer::lex(tokens);
    // dbg!(&tree);

    let mut state = executor::State::standard();
    let result = executor::execute(&mut state, tree);
    dbg!(&result);
    // dbg!(&state);
}
