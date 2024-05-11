// Rusty Proba-Module Tools
use crate::{
    executor::{execute, Interrupt, CURRENT_FILE_PATH},
    lexer::{self, lex},
    parser::{parse_file, parse_str},
    vmstate::State,
};

pub fn exec(state: &mut State, code: &str) -> Result<usize, Interrupt> {
    let tokens = parse_str(code);
    let node_tree = lexer::lex(tokens);
    execute(state, node_tree)
}

pub fn execf(state: &mut State, file_path: &str) -> Result<usize, Interrupt> {
    let tokens = parse_file(file_path.into()).unwrap();
    let node_tree = lex(tokens);
    let prev_file = unsafe { CURRENT_FILE_PATH.clone() };

    unsafe { CURRENT_FILE_PATH = file_path.into() };
    let result = execute(state, node_tree);
    unsafe { CURRENT_FILE_PATH = prev_file };
    result
}

pub fn at(
    state: &mut State,
    at_ptr: usize,
    func: fn(state: &mut State) -> Result<usize, Interrupt>,
) -> Result<usize, Interrupt> {
    state.contexts.push((at_ptr, false));
    let result = func(state);
    state.contexts.pop().unwrap();
    result
}
