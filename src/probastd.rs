use crate::executor::{self, Interrupt, LIB_DIR};
use crate::rpmt::*;
use crate::vmstate::{Body, Pattern, State, Value};

pub(crate) fn define_standard(state: &mut State) -> Result<usize, Interrupt> {
    unsafe { executor::CURRENT_FILE_PATH = "<std>".into() }

    let my_objects = &mut state.objects;
    my_objects.push((0, 0, 0));
    state.objects.push((1, 0, 1));
    state.contexts.push((1, false));
    state.let_field(1, "Object".into(), Value::Pointer(0)); // `at <Context> let Object <Object>`
    state.op_count = 2;

    {
        // at Object
        state.define_method(
            0,
            Pattern::Kw("exit".into()),
            Body::Rust(|state| Err(Interrupt::Exit(state.recipient().unwrap()))),
        );
        state.define_method(
            0,
            Pattern::Kw("print".into()),
            Body::Rust(|state| {
                let ptr = state.recipient().unwrap();
                print!("[[Object#{ptr}]]");
                Ok(ptr)
            }),
        ); // TODO: Redo with convertation into string
        state.define_method(
            0,
            Pattern::Kw("println".into()),
            Body::Rust(|state| {
                let ptr = state.recipient().unwrap();
                println!("[[Object#{ptr}]]");
                Ok(ptr)
            }),
        ); // TODO: Redo with convertation into string
    }

    exec(
        state,
        "
        let Bool copy Object;
        let True copy Bool;
        let False copy Bool;
        at True on : then; Object as T; : else; Object do T;
        at False on : then; Object; : else; Object as F do F;
        ",
    )
    .unwrap();
    {
        // at True
        let true_ptr = state
            .get_field_value(1, "True".into())
            .unwrap()
            .unwrap_ptr();
        state.define_method(
            true_ptr,
            Pattern::Kw("print".into()),
            Body::Rust(|state| {
                print!("[[True]]");
                Ok(state.recipient().unwrap())
            }),
        );
        state.define_method(
            true_ptr,
            Pattern::Kw("println".into()),
            Body::Rust(|state| {
                println!("[[True]]");
                Ok(state.recipient().unwrap())
            }),
        );

        // at False
        let false_ptr = state
            .get_field_value(1, "False".into())
            .unwrap()
            .unwrap_ptr();
        state.define_method(
            false_ptr,
            Pattern::Kw("print".into()),
            Body::Rust(|state| {
                print!("[[False]]");
                Ok(state.recipient().unwrap())
            }),
        );
        let false_ptr = state
            .get_field_value(1, "False".into())
            .unwrap()
            .unwrap_ptr();
        state.define_method(
            false_ptr,
            Pattern::Kw("println".into()),
            Body::Rust(|state| {
                println!("[[False]]");
                Ok(state.recipient().unwrap())
            }),
        );
    } // TODO: Redo with convertation into string

    {
        //  at Object on : ==; Object as other do [[rust]];
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
                        let first_recipient_ptr = state.parent(state.recipient().unwrap()).unwrap();
                        let other_ptr = state
                            .get_field_value_ctx("other".into())
                            .unwrap()
                            .unwrap_ptr();
                        if first_recipient_ptr == other_ptr {
                            Ok(state
                                .get_field_value_ctx("True".into())
                                .unwrap()
                                .unwrap_ptr())
                        } else {
                            Ok(state
                                .get_field_value_ctx("False".into())
                                .unwrap()
                                .unwrap_ptr())
                        }
                    }),
                );
                Ok(state.contexts.pop().unwrap().0)
            }),
        );
    }

    exec(state, "let Number copy Object;").unwrap();

    {
        // at Int
        let int_ptr = exec(state, "let Int copy Number;").unwrap();
        state.contexts.push((int_ptr, false));

        state
            .let_field(int_ptr, "value".into(), Value::Int(0))
            .unwrap();
        // TODO: ++, --, +, -, *, /

        // on : == do { on Int as other do [[rust]]; on Object do False; here };
        state.define_method(
            int_ptr,
            Pattern::Kw("==".into()),
            Body::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let subcontext = state.copy(recipient_ptr).unwrap();
                state.contexts.push((subcontext, false));

                let int_ptr = state.get_field_value(1, "Int".into()).unwrap().unwrap_ptr();
                state.define_method(
                    state.here().unwrap(),
                    Pattern::PtA(int_ptr, "other".into()),
                    Body::Rust(|state| {
                        // first_recipient.value == message.value
                        let first_recipient_ptr = state.parent(state.recipient().unwrap()).unwrap();
                        let left_value = state
                            .get_field_value(first_recipient_ptr, "value".into())
                            .unwrap()
                            .unwrap_int();

                        let other_ptr = state
                            .get_field_value_ctx("other".into())
                            .unwrap()
                            .unwrap_ptr();
                        let right_value = state
                            .get_field_value(other_ptr, "value".into())
                            .unwrap()
                            .unwrap_int();

                        if left_value == right_value {
                            // TODO: Replace with get_field(1, ...)
                            Ok(state
                                .get_field_value(1, "True".into())
                                .unwrap()
                                .unwrap_ptr())
                        } else {
                            Ok(state
                                .get_field_value(1, "False".into())
                                .unwrap()
                                .unwrap_ptr())
                        }
                    }),
                );

                state.define_method(
                    state.here().unwrap(),
                    Pattern::PtA(0, "other".into()),
                    Body::Rust(|state| {
                        let first_recipient_ptr = state.parent(state.recipient().unwrap()).unwrap();
                        let other_ptr = state
                            .get_field_value_ctx("other".into())
                            .unwrap()
                            .unwrap_ptr();

                        if first_recipient_ptr == other_ptr {
                            Ok(state
                                .get_field_value_ctx("True".into())
                                .unwrap()
                                .unwrap_ptr())
                        } else {
                            Ok(state
                                .get_field_value_ctx("False".into())
                                .unwrap()
                                .unwrap_ptr())
                        }
                    }),
                );

                Ok(state.contexts.pop().unwrap().0)
            }),
        );

        state.define_method(
            int_ptr,
            Pattern::Kw("print".into()),
            Body::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let value = state
                    .get_field_value(recipient_ptr, "value".into())
                    .unwrap()
                    .unwrap_int();
                print!("{value}");
                Ok(recipient_ptr)
            }),
        );
        state.define_method(
            int_ptr,
            Pattern::Kw("println".into()),
            Body::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let value = state
                    .get_field_value(recipient_ptr, "value".into())
                    .unwrap()
                    .unwrap_int();
                println!("{value}");
                Ok(recipient_ptr)
            }),
        );

        state.define_method(
            int_ptr,
            Pattern::Kw("++".into()),
            Body::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let value = state
                    .get_field_value(recipient_ptr, "value".into())
                    .unwrap()
                    .unwrap_int();
                state.set_field(recipient_ptr, "value".into(), Value::Int(value + 1));
                Ok(recipient_ptr)
            }),
        );

        state.contexts.pop().unwrap();
    }

    exec(
        state,
        "let None at copy Object ( on : none? do True; );
        at Object on : none? do False;",
    )
    .unwrap();
    {
        let none_ptr = state
            .get_field_value(1, "None".into())
            .unwrap()
            .unwrap_ptr();
        state.define_method(
            none_ptr,
            Pattern::Kw("print".into()),
            Body::Rust(|state| {
                print!("");
                Ok(state.recipient().unwrap())
            }),
        );
        state.define_method(
            none_ptr,
            Pattern::Kw("println".into()),
            Body::Rust(|state| {
                println!("");
                Ok(state.recipient().unwrap())
            }),
        );
        state.define_method(
            none_ptr,
            Pattern::Kw("dbg".into()),
            Body::Rust(|state| {
                println!("[[None]]");
                Ok(state.recipient().unwrap())
            }),
        );
    }

    execf(state, &(LIB_DIR.to_string() + "/std.proba"))?;

    return Ok(0);
}
