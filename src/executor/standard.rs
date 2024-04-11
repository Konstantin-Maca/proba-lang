use super::fast::exec;
use super::state::{MethodBody, Pattern, State, Value};
use super::Interrupt;

pub fn prepare_std(state: &mut State) {
    state.objects.insert(0, (0, 0));
    state.objects.insert(1, (0, 1));
    state.contexts.push((1, false));
    state.let_field_value(1, "Object".into(), Value::Pointer(0)); // `at <Context> let Object <Object>`
    state.op_count = 2;

    // TODO: "==" method for Object

    state.define_method(
        0,
        Pattern::Kw("exit".into()),
        MethodBody::Rust(|_| Err(Interrupt::Exit)),
    );

    exec(
        state,
        "
        at (let None copy Object) on Object do None;

        let Bool copy Object;
        let True copy Bool;
        let False copy Bool;

        at (let List copy Object) (
            let Node copy Object;
            let End copy Node;
            at Node (
                let data Object;
                let next End;

                on : get do data;
                on : empty? do (
                    (at copy Object (
                        on End do True; [ TODO: do => be ]
                        on Object do False;
                    )) next;
                );
            );

            let first Node;

            on : first do (
                (at copy Object (
                    on True; Node as _ do None;
                    on False; Node as first do first get;
                )) (empty?) first;
            );
            on : empty? do first empty?;
        );
        exit
        ",
    )
    .unwrap();
}
