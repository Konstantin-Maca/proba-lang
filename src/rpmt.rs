// Rusty Proba-Module Tools

pub fn exec(
    state: &mut crate::vmstate::State,
    code: &str,
) -> Result<usize, crate::executor::Interrupt> {
    let tokens = crate::parser::parse_str(code);
    let node_tree = crate::lexer::lex(tokens);
    crate::executor::execute(state, node_tree)
}

pub fn execf(
    state: &mut crate::vmstate::State,
    file_path: &str,
) -> Result<usize, crate::executor::Interrupt> {
    let tokens = crate::parser::parse_file(file_path.into()).unwrap();
    let node_tree = crate::lexer::lex(tokens);
    crate::executor::execute(state, node_tree)
}

pub fn at(
    state: &mut crate::vmstate::State,
    at_ptr: usize,
    func: fn(state: &mut crate::vmstate::State) -> Result<usize, crate::executor::Interrupt>,
) -> Result<usize, crate::executor::Interrupt> {
    state.contexts.push((at_ptr, false));
    let result = func(state);
    state.contexts.pop().unwrap();
    result
}
