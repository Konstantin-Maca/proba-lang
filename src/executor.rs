use std::ops::Deref;

use crate::lexer::{Node, NodeData, PatternKind};

pub(crate) use self::state::State;
use self::state::{Body, Pattern, Value};

mod standard;
pub(super) mod state;
mod fast {
    pub(crate) fn exec(state: &mut super::State, code: &str) -> Result<usize, super::Interrupt> {
        let tokens = crate::parser::parse_str(code);
        let tree = crate::lexer::lex(tokens);
        super::execute(state, tree)
    }
}

#[derive(Debug)]
pub enum Interrupt {
    Exit(usize),
    Return(usize),
    Repeat,
    Error(usize, String),
}

pub fn execute(state: &mut State, node: Node) -> Result<usize, Interrupt> {
    match node.data.deref() {
        NodeData::Here => Ok(state.contexts.last().unwrap().0),
        NodeData::Message(rec_node, msg_node) => {
            // Execute recipient
            let recipient = execute(state, rec_node.clone())?;
            let message = match msg_node.data.deref() {
                NodeData::Name(ref name) => {
                    let some_method = state.get_method(recipient, name.clone());
                    if let Some((_, body)) = some_method {
                        // Try call method of the recipient-object
                        return execute_method(
                            state,
                            recipient,
                            body.clone(),
                            ("".into(), recipient),
                        );
                    }
                    execute(state, msg_node.clone())?
                }
                _ => execute(state, msg_node.clone())?,
            };
            let method = match state.match_method(recipient, message) {
                Some(method) => method,
                None => Err(Interrupt::Error(
                    node.line,
                    format!(
                        "Failed to match method for recipient {recipient} and message {message}"
                    ),
                ))?,
            };
            let name = match &method.0 .1 {
                Pattern::Kw(_) => unreachable!(),
                Pattern::Eq(_) => "".into(),
                Pattern::EqA(_, name) => name.clone(),
                Pattern::Pt(_) => "".into(),
                Pattern::PtA(_, name) => name.clone(),
            };
            let body = method.1.clone();
            execute_method(state, recipient, body, (name, message))
        }
        NodeData::Name(name) => {
            let some_method = state.get_method_ctx(name.clone());
            let context = state.contexts.last().unwrap().0;
            if let Some(((_, pattern), body)) = some_method {
                // Try call method of the context-object
                let name = match pattern {
                    Pattern::Kw(_) => "".into(),
                    Pattern::Eq(_) => "".into(),
                    Pattern::EqA(_, name) => name.clone(),
                    Pattern::Pt(_) => "".into(),
                    Pattern::PtA(_, name) => name.clone(),
                };
                execute_method(state, context, body.clone(), (name, context))
            } else if let Some(value) = state.get_field_ctx(name.into()) {
                // Try get field of a context-object and then react to it's answer
                match value {
                    Value::Pointer(ptr) => Ok(ptr),
                    Value::Int(_) | Value::Float(_) => todo!("Do something with system values"),
                }
            } else {
                Err(Interrupt::Error(
                    node.line,
                    format!("Undefined keyword-method or field name: {}", name),
                ))?
            }
        }
        NodeData::Int(_) => todo!("Create int object"),
        NodeData::Float(_) => todo!("Create float object"),
        NodeData::String(_) => todo!("Create string object"),
        NodeData::Pattern(..) => unreachable!(),
        NodeData::As(..) => unreachable!(),
        NodeData::Queue(queue) => execute_queue(state, &queue, node.line),
        NodeData::QuickContext(queue) => {
            let context = state.contexts.last().unwrap().0;
            let sub_context = state.copy(context).unwrap();
            state.contexts.push((sub_context, false));
            let result = execute_queue(state, &queue, node.line);
            state.contexts.pop().unwrap();
            state.clear_garbage(if let Ok(p) = result { vec![p] } else { vec![] });
            result
        }
        NodeData::Copy(node) => {
            let ptr = execute(state, node.clone())?;
            match state.copy(ptr) {
                Some(p) => Ok(p),
                None => Err(Interrupt::Error(
                    node.line,
                    "Fatal system error: Failed to copy object, because it does not exists".into(),
                )),
            }
        }
        NodeData::At(context_node, body_node) => {
            let context_ptr = execute(state, context_node.clone())?;
            state.contexts.push((context_ptr, false));
            let result = execute(state, body_node.clone());
            state.contexts.pop().unwrap();
            state.clear_garbage(if let Ok(p) = result { vec![p] } else { vec![] });
            result
        }
        NodeData::Let(name, value_node) => {
            // You can let new field or re-let existing one in a context-object,
            // only if you entered into it from another context,
            // that is a copy of the current context-object's creation context.
            // Exception: the global context.
            let heres_context = state.objects[&state.here().unwrap()].1;
            if state.here().unwrap() != 1
                && !state
                    .relation(state.contexts[state.contexts.len() - 2].0, heres_context)
                    .is_some()
            {
                Err(Interrupt::Error(
                    node.line,
                    "You can not define a field in this context.".into(),
                ))?
            }

            let value = execute(state, value_node.clone())?;
            let success = state.let_field(
                state.contexts.last().unwrap().0,
                name.clone(),
                Value::Pointer(value),
            );
            match success {
                Some(_) => Ok(value),
                None => Err(Interrupt::Error(node.line, "Unexpected error".into())),
            }
        }
        NodeData::Set(name, value_node) => {
            // You can set existing field in a context-object,
            // only if you entered into it from another context,
            // that is a copy of the current context-object's creation context.
            // Exception: the global context.
            let super_context = state.contexts[state.contexts.len() - 2].0;
            let heres_context = state.objects[&state.here().unwrap()].1;
            if state.here().unwrap() != 1 && !state.relation(super_context, heres_context).is_some()
            {
                Err(Interrupt::Error(
                    node.line,
                    "You can not define a field in this context.".into(),
                ))?
            }

            let value = execute(state, value_node.clone())?;
            let success = state.set_field(
                state.contexts.last().unwrap().0,
                name.clone(),
                Value::Pointer(value),
            );
            match success {
                Some(_) => Ok(value),
                None => Err(Interrupt::Error(
                    node.line,
                    format!("There is no field with name {name}"),
                )),
            }
        }
        NodeData::OnDo(patterns, body) => {
            if patterns.len() != 1 {
                unreachable!()
            }
            let pattern = match patterns[0].data.deref() {
                NodeData::Pattern(PatternKind::Keyword, name_node) => {
                    let name = match name_node.data.deref() {
                        NodeData::Name(name) => name,
                        _ => unreachable!(),
                    };
                    Pattern::Kw(name.clone())
                }
                NodeData::Pattern(PatternKind::Prototype, node) => {
                    Pattern::Pt(execute(state, node.clone())?)
                }
                NodeData::Pattern(PatternKind::Equalness, node) => {
                    Pattern::Eq(execute(state, node.clone())?)
                }
                NodeData::As(pattern_node, alias) => match pattern_node.data.deref() {
                    NodeData::Pattern(PatternKind::Prototype, node) => {
                        Pattern::PtA(execute(state, node.clone())?, alias.clone())
                    }
                    NodeData::Pattern(PatternKind::Equalness, node) => {
                        Pattern::EqA(execute(state, node.clone())?, alias.clone())
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            state.define_method(
                state.contexts.last().unwrap().0,
                pattern,
                Body::Do(body.clone()),
            );
            Ok(state.contexts.last().unwrap().0)
        }
        NodeData::OnRust(patterns, body_func) => {
            if patterns.len() != 1 {
                unreachable!()
            }
            let pattern = match patterns[0].data.deref() {
                NodeData::Pattern(PatternKind::Keyword, name_node) => {
                    let name = match name_node.data.deref() {
                        NodeData::Name(name) => name,
                        _ => unreachable!(),
                    };
                    Pattern::Kw(name.clone())
                }
                NodeData::Pattern(PatternKind::Prototype, node) => {
                    Pattern::Pt(execute(state, node.clone())?)
                }
                NodeData::Pattern(PatternKind::Equalness, node) => {
                    Pattern::Eq(execute(state, node.clone())?)
                }
                NodeData::As(pattern_node, alias) => match pattern_node.data.deref() {
                    NodeData::Pattern(PatternKind::Prototype, node) => {
                        Pattern::PtA(execute(state, node.clone())?, alias.clone())
                    }
                    NodeData::Pattern(PatternKind::Equalness, node) => {
                        Pattern::EqA(execute(state, node.clone())?, alias.clone())
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            state.define_method(
                state.contexts.last().unwrap().0,
                pattern,
                Body::Rust(*body_func),
            );
            Ok(state.contexts.last().unwrap().0)
        }
    }
}

fn execute_queue(state: &mut State, queue: &Vec<Node>, line: usize) -> Result<usize, Interrupt> {
    if queue.len() == 0 {
        Err(Interrupt::Error(line, "Empty block of code".into()))?
    }
    let mut result = 0;
    for node in queue {
        match execute(state, node.clone()) {
            Ok(ptr) => result = ptr,
            Err(int) => Err(int)?,
        }
    }
    Ok(result)
}

fn execute_method(
    state: &mut State,
    owner_ptr: usize,
    body: Body,
    arg: (String, usize),
) -> Result<usize, Interrupt> {
    let context = state.copy(owner_ptr).unwrap();
    state.contexts.push((context, true));
    state.let_field(context, arg.0, Value::Pointer(arg.1));
    let result = loop {
        let result = match &body {
            Body::Do(body) => execute(state, body.clone()),
            Body::Rust(body_func) => body_func(state),
        };
        match result {
            Ok(ptr) => break Ok(ptr),
            Err(Interrupt::Return(ptr)) => break Ok(ptr),
            Err(Interrupt::Repeat) => continue,
            Err(int) => Err(int)?,
        }
    };
    state.contexts.pop().unwrap();
    result
}
