use crate::lexer::{Node, NodeKind, PatternKind};
use crate::vmstate::{Body, Pattern, State, Value};
use crate::PROG_CONFIG;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Interrupt {
    Exit(usize),
    Return(usize),
    Repeat,
    Err(String),
    Error(usize, String),
}

pub const LIB_DIR: &str = "/home/mazza/dev/proba-lang/lib"; // CHAGE THIS CONSTANT TO WHERE YOU WANT TO STORE LIBS

pub static mut CURRENT_FILE_PATH: String = String::new();

pub fn execute(state: &mut State, node: Node) -> Result<usize, Interrupt> {
    match node.data.deref() {
        NodeKind::Here => Ok(state.here().unwrap()),
        NodeKind::Me => match state.recipient() {
            Some(ptr) => Ok(ptr),
            None => Ok(state.contexts.first().unwrap().0),
        },
        NodeKind::Return => Err(Interrupt::Return(state.recipient().unwrap())),
        NodeKind::Repeat => Err(Interrupt::Repeat),
        NodeKind::Message(rec_node, msg_node) => {
            // Execute recipient
            let recipient = execute(state, rec_node.clone())?;
            let message = match msg_node.data.deref() {
                NodeKind::Name(ref name) => {
                    let some_method = state.get_method(recipient, name.clone());
                    if let Some((_, _, body)) = some_method {
                        // Try call method of the recipient-object
                        return execute_method(
                            state,
                            recipient,
                            body.clone(),
                            (format!("[:{name}]").into(), recipient),
                        );
                    }
                    execute(state, msg_node.clone())?
                }
                _ => execute(state, msg_node.clone())?,
            };
            let method = match match_method(state, recipient, message) {
                Some(method) => method,
                None => Err(Interrupt::Error(
                    node.line,
                    format!(
                        "Failed to match method for recipient {recipient} and message {message}"
                    ),
                ))?,
            };
            let name = match method.1 {
                Pattern::Kw(_) => unreachable!(),
                Pattern::Eq(_) | Pattern::Pt(_) => format!("[[no as]]").into(),
                Pattern::EqA(_, name) | Pattern::PtA(_, name) => name.clone(),
            };
            execute_method(state, recipient, method.2, (name, message))
        }
        NodeKind::Name(name) => {
            let some_method = state.get_method_ctx(name.clone());
            let context = match state.contexts.last() {
                Some(c) => c.0,
                None => Err(Interrupt::Error(
                    node.line,
                    format!("There is no field or key-method named `{name}'"),
                ))?,
            };
            if let Some((_, pattern, body)) = some_method {
                // Try call method of the context-object
                let name = match pattern {
                    Pattern::Kw(_) | Pattern::Eq(_) | Pattern::Pt(_) => format!("[[no as]]").into(),
                    Pattern::EqA(_, name) | Pattern::PtA(_, name) => name.clone(),
                };
                execute_method(state, context, body.clone(), (name, context))
            } else if let Some(value) = state.get_field_value_ctx(name.into()) {
                // Try get field of a context-object
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
        NodeKind::Int(value) => {
            let ptr = state
                .copy(state.get_field_value(1, "Int".into()).unwrap().unwrap_ptr())
                .unwrap();
            state.let_field(ptr, "value".into(), Value::Int(*value));
            Ok(ptr)
        }
        NodeKind::Float(_) => todo!("Create float object"),
        NodeKind::String(_) => todo!("Create string object"),
        NodeKind::Pattern(..) => unreachable!(),
        NodeKind::As(..) => unreachable!(),
        NodeKind::Queue(queue) => execute_queue(state, &queue, node.line),
        NodeKind::QuickContext(queue) => {
            let context = state.contexts.last().unwrap().0;
            let sub_context = state.copy(context).unwrap();
            state.contexts.push((sub_context, false));
            let result = execute_queue(state, &queue, node.line);
            state.contexts.pop().unwrap();
            state.clear_garbage(if let Ok(p) = result { vec![p] } else { vec![] });
            result
        }
        NodeKind::Copy(node) => {
            let ptr = execute(state, node.clone())?;
            match state.copy(ptr) {
                Some(p) => Ok(p),
                None => Err(Interrupt::Error(
                    node.line,
                    "Fatal system error: Failed to copy object, because it does not exists".into(),
                )),
            }
        }
        NodeKind::At(context_node, body_node) => {
            let context_ptr = execute(state, context_node.clone())?;
            state.contexts.push((context_ptr, false));
            let result = execute(state, body_node.clone());
            state.contexts.pop().unwrap();
            state.clear_garbage(if let Ok(p) = result { vec![p] } else { vec![] });
            result
        }
        NodeKind::Let(name, value_node) => {
            // You can let new field or re-let existing one in a context-object,
            // only if you entered into it from another context,
            // that is a copy of the current context-object's creation context.
            // Exception: the global context.
            let here = state.here().unwrap();
            let heres_context = state.objects.iter().find(|obj| obj.0 == here).unwrap().2;
            if state.here().unwrap() != 1
                && !state
                    .relation(state.contexts[state.contexts.len() - 2].0, heres_context)
                    .is_some()
            {
                Err(Interrupt::Error(
                    node.line,
                    "Unable to access fileds of the context object here.".into(),
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
        NodeKind::Set(name, value_node) => {
            // You can set existing field in a context-object,
            // only if you entered into it from another context,
            // that is a copy of the current context-object's creation context.
            // Exception: the global context.
            let super_context = if state.contexts.len() >= 2 {
                Some(state.contexts.get(state.contexts.len() - 2).unwrap().0)
            } else {
                None
            };
            let here = state.here().unwrap();
            let heres_context = state.objects.iter().find(|obj| obj.0 == here).unwrap().2;
            if state.here().unwrap() != 1
                && super_context.is_some()
                && !state
                    .relation(super_context.unwrap(), heres_context)
                    .is_some()
            {
                Err(Interrupt::Error(
                    node.line,
                    "Unable to access fileds of the context object here.".into(),
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
        NodeKind::OnDo(patterns, body) => {
            if patterns.len() != 1 {
                unreachable!()
            }
            let pattern = match patterns[0].data.deref() {
                NodeKind::Pattern(PatternKind::Keyword, name_node) => {
                    let name = match name_node.data.deref() {
                        NodeKind::Name(name) => name,
                        _ => unreachable!(),
                    };
                    Pattern::Kw(name.clone())
                }
                NodeKind::Pattern(PatternKind::Prototype, node) => {
                    Pattern::Pt(execute(state, node.clone())?)
                }
                NodeKind::Pattern(PatternKind::Equalness, node) => {
                    Pattern::Eq(execute(state, node.clone())?)
                }
                NodeKind::As(pattern_node, alias) => match pattern_node.data.deref() {
                    NodeKind::Pattern(PatternKind::Prototype, node) => {
                        Pattern::PtA(execute(state, node.clone())?, alias.clone())
                    }
                    NodeKind::Pattern(PatternKind::Equalness, node) => {
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
        // NOTE: Maybe useless
        // NodeKind::OnRust(patterns, body_func) => {
        //     if patterns.len() != 1 {
        //         unreachable!()
        //     }
        //     let pattern = match patterns[0].data.deref() {
        //         NodeKind::Pattern(PatternKind::Keyword, name_node) => {
        //             let name = match name_node.data.deref() {
        //                 NodeKind::Name(name) => name,
        //                 _ => unreachable!(),
        //             };
        //             Pattern::Kw(name.clone())
        //         }
        //         NodeKind::Pattern(PatternKind::Prototype, node) => {
        //             Pattern::Pt(execute(state, node.clone())?)
        //         }
        //         NodeKind::Pattern(PatternKind::Equalness, node) => {
        //             Pattern::Eq(execute(state, node.clone())?)
        //         }
        //         NodeKind::As(pattern_node, alias) => match pattern_node.data.deref() {
        //             NodeKind::Pattern(PatternKind::Prototype, node) => {
        //                 Pattern::PtA(execute(state, node.clone())?, alias.clone())
        //             }
        //             NodeKind::Pattern(PatternKind::Equalness, node) => {
        //                 Pattern::EqA(execute(state, node.clone())?, alias.clone())
        //             }
        //             _ => unreachable!(),
        //         },
        //         _ => unreachable!(),
        //     };
        //     state.define_method(
        //         state.contexts.last().unwrap().0,
        //         pattern,
        //         Body::Rust(*body_func),
        //     );
        //     Ok(state.contexts.last().unwrap().0)
        // }
        NodeKind::Import(name, node) => {
            // TODO: Put here dir of the executed file
            let target_object_ptr = execute(state, node.clone())?;
            import_module(
                state,
                target_object_ptr,
                name.into(),
                vec![
                    LIB_DIR.into(),
                    PathBuf::from(unsafe { &PROG_CONFIG }.file_path.clone().unwrap())
                        .parent()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .into(),
                ],
            )
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

pub fn execute_method(
    state: &mut State,
    owner_ptr: usize,
    body: Body,
    arg: (String, usize),
) -> Result<usize, Interrupt> {
    let context = state.copy(owner_ptr).unwrap();
    state.contexts.push((context, true));
    state.let_field(context, arg.0, Value::Pointer(arg.1));
    let result = loop {
        let result = match body {
            Body::Do(ref body) => execute(state, body.clone()),
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

pub fn match_method(
    state: &mut State,
    ptr: usize,
    message: usize,
) -> Option<(usize, Pattern, Body)> {
    for (owner_ptr, pattern, body) in state.methods.clone().iter() {
        match pattern {
            Pattern::Eq(pattern_ptr) | Pattern::EqA(pattern_ptr, ..)
                if *owner_ptr == ptr && {
                    // pattern_ptr == message
                    let method = state.get_method(*pattern_ptr, "==".into()).unwrap();
                    let ptr = execute_method(
                        state,
                        *pattern_ptr,
                        method.2.clone(),
                        ("[[no as *MM]]".into(), 0),
                    )
                    .unwrap();

                    let method = match_method(state, ptr, message).unwrap();
                    let arg_name = match &method.1 {
                        Pattern::Eq(_) | Pattern::Pt(_) => "[[no as *MM2]]".into(),
                        Pattern::EqA(_, name) | Pattern::PtA(_, name) => name.clone(),
                        _ => unreachable!(),
                    };
                    let result_ptr =
                        execute_method(state, ptr, method.2.clone(), (arg_name, message));

                    result_ptr.unwrap()
                        == state
                            .get_field_value(1, "True".into())
                            .unwrap()
                            .unwrap_ptr()
                } =>
            {
                return Some((*owner_ptr, pattern.clone(), body.clone()));
            }
            Pattern::Pt(pattern_ptr) | Pattern::PtA(pattern_ptr, ..)
                if *owner_ptr == ptr && state.relation(message, pattern_ptr.clone()).is_some() =>
            {
                return Some((*owner_ptr, pattern.clone(), body.clone()))
            }
            _ => continue,
        }
    }
    match ptr {
        0 => None,
        _ => match_method(state, state.parent(ptr)?, message),
    }
}

pub fn import_module(
    state: &mut State,
    target_object_ptr: usize,
    module_name: String,
    dirs: Vec<String>,
) -> Result<usize, Interrupt> {
    // Save context stack
    let ctx_save = state.contexts.clone();
    state.contexts = vec![(target_object_ptr, false)];

    // TODO once: Try to find rusty module
    // let mut lib_filename = None;
    // for d in &dirs {
    //     let fp = format!("{d}/lib{module_name}.so");
    //     if std::path::Path::new(&fp).is_file() {
    //         lib_filename = Some(fp);
    //         break;
    //     }
    //     let fp = format!("{d}/{module_name}/target/debug/lib{module_name}.so");
    //     if std::path::Path::new(&fp).is_file() {
    //         lib_filename = Some(fp);
    //         break;
    //     }
    // }
    // let result = match lib_filename {
    //     None => None,
    //     Some(lib_filename) => {
    //         // Load the dylib
    //         unsafe {
    //             let lib = libloading::Library::new(lib_filename.clone());
    //             let lib = match lib {
    //                 Ok(lib) => lib,
    //                 Err(_) => {
    //                     Err(Interrupt::Error(0, format!("Import error: Failed to import existing rust-written module `{module_name}' ({lib_filename}).")))?
    //                 }
    //             };
    //             let module_main = lib.get(b"main");
    //             let module_main: libloading::Symbol<unsafe extern fn(State) -> (State, Result<usize, Interrupt>)> =
    //                 match module_main {
    //                     Ok(module_main) => module_main,
    //                     Err(_) => {
    //                         Err(Interrupt::Error(0, format!("Import error: Failed to import existing rust-written module `{module_name}' ({lib_filename}).")))?
    //                     }
    //                 };
    //             // dbg!(&state);
    //             let (new_state, result) = module_main(state.clone());
    //             state.clone_from(&new_state);
    //             // dbg!(&state);
    //             Some(result)
    //         }
    //     }
    // };
    let result = None;

    // Try to find Proba-module
    let mut file_path = None;
    for d in &dirs {
        let fp = format!("{d}/{module_name}.proba");
        if std::path::Path::new(&fp).is_file() {
            file_path = Some(fp);
            break;
        }
    }
    let result = match file_path {
        Some(file_path) => match crate::parser::parse_file(file_path) {
            Ok(tokens) => {
                let tree_node = crate::lexer::lex(tokens);
                execute(state, tree_node)
            }
            Err(_) => Err(Interrupt::Err(format!(
                "Import error: There is no method with name `{module_name}'."
            ))),
        },
        None if result.is_some() => result.unwrap(),
        None => Err(Interrupt::Err(format!(
            "Import error: There is no method with name `{module_name}'."
        ))),
    };

    // Restore context stack
    state.contexts = ctx_save;

    result
}
