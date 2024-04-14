use super::fast::exec;
use super::state::{MethodBody, Pattern, State, Value};
use super::Interrupt;

pub fn prepare_std(state: &mut State) {
    state.objects.insert(0, (0, 0));
    state.objects.insert(1, (0, 1));
    state.contexts.push((1, false));
    state.let_field(1, "Object".into(), Value::Pointer(0)); // `at <Context> let Object <Object>`
    state.op_count = 2;

    state.define_method(
        0,
        Pattern::Kw("exit".into()),
        MethodBody::Rust(|state| Err(Interrupt::Exit(state.recipient().unwrap()))),
    );
    state.define_method(
        0,
        Pattern::Kw("return".into()),
        MethodBody::Rust(|state| Err(Interrupt::Return(state.recipient().unwrap()))),
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
            MethodBody::Rust(|state| {
                let recipient_ptr = state.recipient().unwrap();
                let subcontext = state.copy(recipient_ptr).unwrap();
                state.contexts.push((subcontext, false));
                state.define_method(
                    state.here().unwrap(),
                    Pattern::PtA(0, "other".into()),
                    MethodBody::Rust(|state| {
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

        at (let List copy Object) (
            let Node copy Object;
            let End copy Node;
            at Node (
                let data Object;
                let next End;

                on : get do data;
                on : getNext do next;
                on : end? do False;
            );
            at End (
                let next End;

                on : get do None;
                on : end? do True;
            );

            let first End;

            on : first do first get;
            on : empty? do first end?;
        );
        ",
    )
    .unwrap();
}
