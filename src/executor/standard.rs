use crate::lexer::lex;
use crate::parser::parse_file;

use super::fast::exec;
use super::state::{Body, Pattern, State, Value};
use super::{execute, Interrupt};

const STD_DIR: &str = "./std/";

pub fn prepare_std(state: &mut State) {
    state.objects.insert(0, (0, 0));
    state.objects.insert(1, (0, 1));
    state.contexts.push((1, false));
    state.let_field(1, "Object".into(), Value::Pointer(0)); // `at <Context> let Object <Object>`
    state.op_count = 2;

    state.define_method(
        0,
        Pattern::Kw("exit".into()),
        Body::Rust(|state| Err(Interrupt::Exit(state.recipient().unwrap()))),
    );
    state.define_method(
        0,
        Pattern::Kw("return".into()),
        Body::Rust(|state| Err(Interrupt::Return(state.recipient().unwrap()))),
    );

    exec(
        state,
        "let Bool copy Object; let True copy Bool; let False copy Bool;",
    )
    .unwrap();

    {
        //  at Object on : ==; Object as other do [[ rust ]];
        state.define_method(
            0,
            Pattern::Kw("==".into()),
            Body::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let subcontext = state.copy(recipient_ptr).unwrap();
                state.contexts.push((subcontext, false));
                state.define_method(
                    state.here().unwrap(),
                    Pattern::PtA(0, "other".into()),
                    Body::Rust(|state| {
                        let first_recipient_ptr =
                            state.objects.get(&state.recipient().unwrap()).unwrap().0;
                        let other_ptr = state.get_field_ctx("other".into()).unwrap().unwrap_ptr();
                        if first_recipient_ptr == other_ptr {
                            Ok(state.get_field_ctx("True".into()).unwrap().unwrap_ptr())
                        } else {
                            Ok(state.get_field_ctx("False".into()).unwrap().unwrap_ptr())
                        }
                    }),
                );
                Ok(state.contexts.pop().unwrap().0)
            }),
        );
    }

    exec(
        state,
        "
        at (let None copy Object) on : none? do True;
        at Object on : none? do False;
        ",
    )
    .unwrap();

    execute(state, lex(parse_file(STD_DIR.to_string() + "list.proba"))).unwrap();
}
