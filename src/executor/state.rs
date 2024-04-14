use std::collections::HashMap;

use crate::lexer::Node;

use super::{execute_method, standard::prepare_std, Interrupt};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Value {
    Pointer(usize),
    Int(isize),
    Float(f64),
}

impl Value {
    pub fn unwrap_ptr(&self) -> usize {
        match self {
            Value::Pointer(p) => *p,
            _ => panic!("Failed to unwrap pointer value"),
        }
    }
    pub fn unwrap_int(&self) -> isize {
        match self {
            Value::Int(i) => *i,
            _ => panic!("Failed to unwrap int value"),
        }
    }
    pub fn unwrap_float(&self) -> f64 {
        match self {
            Value::Float(f) => *f,
            _ => panic!("Failed to unwrap float value"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(super) enum Pattern {
    Kw(String),
    Eq(usize),
    EqA(usize, String),
    Pt(usize),
    PtA(usize, String),
}

#[derive(Debug, Clone)]
pub(super) enum Body {
    Do(Node),
    Rust(fn(&mut State) -> Result<usize, Interrupt>),
}

#[derive(Debug)]
pub(crate) struct State {
    pub(super) op_count: usize,
    pub(super) contexts: Vec<(usize, bool)>, // Context (pointer, is pushed for method?)

    // ptr => (parent, context)
    pub(super) objects: HashMap<usize, (usize, usize)>,
    // (owner, name) => { pointer | int | float }
    pub(super) fields: HashMap<(usize, String), Value>,
    // (owner, pattern) => { pointer | node }
    pub(super) methods: HashMap<(usize, Pattern), Body>,
}

impl State {
    pub fn new() -> Self {
        Self {
            op_count: 0,
            contexts: Vec::new(),
            objects: HashMap::new(),
            fields: HashMap::new(),
            methods: HashMap::new(),
        }
    }

    pub fn standard() -> Self {
        let mut state = Self::new();
        prepare_std(&mut state);
        state
    }
    pub fn here(&self) -> Option<usize> {
        Some(self.contexts.last()?.0)
    }
    pub fn recipient(&self) -> Option<usize> {
        for (ptr, is_for_method) in self.contexts.iter().rev() {
            if *is_for_method {
                return Some(self.objects.get(ptr)?.0);
            }
        }
        None // NOTE: In theory, this is unreachable, but I'm not sure.
    }

    pub(super) fn copy(&mut self, ptr: usize) -> Option<usize> {
        self.objects.get(&ptr)?;
        let new_ptr = self.op_count;
        self.op_count += 1;
        self.objects
            .insert(new_ptr, (ptr, self.contexts.last().unwrap().0));
        return Some(new_ptr);
    }

    pub fn relation(&self, ptr: usize, parent_ptr: usize) -> Option<usize> {
        if ptr == parent_ptr {
            return Some(0);
        }
        if ptr == 0 {
            return None;
        }
        match self.objects.get(&ptr) {
            Some((ptr, _)) => Some(self.relation(*ptr, parent_ptr)? + 1),
            None => None,
        }
    }

    /// Return Some if success, else None.
    pub(super) fn let_field(&mut self, ptr: usize, name: String, value: Value) -> Option<()> {
        if !self.objects.contains_key(&ptr) {
            None?
        }
        match self.fields.get_mut(&(ptr, name.clone())) {
            Some(val) => *val = value,
            None => {
                self.fields.insert((ptr, name.clone()), value);
            }
        };
        Some(())
    }
    pub(crate) fn set_field(&mut self, ptr: usize, name: String, value: Value) -> Option<()> {
        let field_value = self.fields.get_mut(&(ptr, name))?;
        *field_value = value;
        Some(())
    }
    pub fn get_field(&self, ptr: usize, name: String) -> Option<Value> {
        match self.fields.get(&(ptr, name.clone())) {
            Some(val) => Some(*val),
            None => {
                if ptr == 0 {
                    None?
                } else {
                    let parent_ptr = self.objects.get(&ptr)?.0;
                    self.get_field(parent_ptr, name)
                }
            }
        }
    }
    pub(super) fn get_field_ctx(&self, name: String) -> Option<Value> {
        // You can get field in a context-object,
        // only if you entered into it from another context,
        // that is a copy of the current context-object's creation context.
        // Exception: the global context.
        let heres_context = self.objects[&self.here().unwrap()].1;
        if self.here().unwrap() != 1
            && !self
                .relation(self.contexts[self.contexts.len() - 2].0, heres_context)
                .is_some()
        {
            None?
        }

        for (ptr, is_for_method) in self.contexts.iter().rev() {
            match self.get_field(*ptr, name.clone()) {
                Some(value) => return Some(value),
                None => (),
            }
            if *is_for_method {
                break;
            }
        }
        self.get_field(self.contexts[0].0, name)
    }

    /// Return true, if method is re-defined;
    /// return false, if new method is defined.
    pub(super) fn define_method(&mut self, ptr: usize, pattern: Pattern, body: Body) -> bool {
        match self.methods.insert((ptr, pattern), body) {
            Some(_) => true,
            None => false,
        }
    }
    /// Use when message is a name (word (keyword)).
    pub(super) fn get_method(
        &self,
        ptr: usize,
        keyword: String,
    ) -> Option<(&(usize, Pattern), &Body)> {
        match self
            .methods
            .get_key_value(&(ptr, Pattern::Kw(keyword.clone())))
        {
            Some(method) => Some(method),
            None => {
                if ptr == 0 {
                    None?
                } else {
                    let parent_ptr = self.objects.get(&ptr)?.0;
                    self.get_method(parent_ptr, keyword)
                }
            }
        }
    }
    /// Use when message is a name (word (keyword)).
    pub(super) fn get_method_ctx(&self, keyword: String) -> Option<(&(usize, Pattern), &Body)> {
        for &(ptr, is_for_method) in self.contexts.iter().rev() {
            match self.get_method(ptr, keyword.clone()) {
                Some(method) => return Some(method),
                None => (),
            }
            if is_for_method {
                break;
            }
        }
        self.get_method(self.contexts[0].0, keyword)
    }
    /// Use when message is an object.pub(super) fn match_method(
    pub(super) fn match_method(
        &mut self,
        ptr: usize,
        message: usize,
    ) -> Option<((usize, Pattern), Body)> {
        for (key, body) in self.methods.clone().iter() {
            match key.1 {
                Pattern::Eq(pattern_ptr) | Pattern::EqA(pattern_ptr, ..)
                    if key.0 == ptr && {
                        // pattern_ptr == message
                        let method = self.get_method(pattern_ptr, "==".into()).unwrap();
                        let ptr =
                            execute_method(self, pattern_ptr, method.1.clone(), ("".into(), 0))
                                .unwrap();
                        let method = self.match_method(ptr, message).unwrap();
                        let arg_name = match &method.0 .1 {
                            Pattern::Eq(_) | Pattern::Pt(_) => "".into(),
                            Pattern::EqA(_, name) | Pattern::PtA(_, name) => name.clone(),
                            _ => unreachable!(),
                        };
                        let ptr = execute_method(self, ptr, method.1.clone(), (arg_name, message));
                        ptr.unwrap() == self.get_field(1, "True".into()).unwrap().unwrap_ptr()
                    } =>
                {
                    return Some((key.clone(), body.clone()));
                }
                Pattern::Pt(pattern_ptr) | Pattern::PtA(pattern_ptr, ..)
                    if key.0 == ptr && self.relation(message, pattern_ptr).is_some() =>
                {
                    return Some((key.clone(), body.clone()));
                }
                _ => continue,
            }
        }
        if ptr == 0 {
            None?
        } else {
            let parent_ptr = self.objects.get(&ptr)?.0;
            self.match_method(parent_ptr, message)
        }
    }

    fn count_links(&self, ptr: usize) -> usize {
        let mut count = 0;
        // As parent and context owner
        for (_, (pn, c)) in &self.objects {
            if *pn == ptr {
                count += 1;
            }
            if *c == ptr {
                count += 1;
            }
        }
        // As field value
        for pair in &self.fields {
            if let Value::Pointer(p) = pair.1 {
                if *p == ptr {
                    count += 1;
                }
            }
        }
        // As context
        for (p, _) in &self.contexts {
            if *p == ptr {
                count += 1;
            }
        }

        count
    }

    pub(super) fn clear_garbage(&mut self, white_list: Vec<usize>) {
        let mut run = true;
        while run {
            run = false;
            for (ptr, _) in self.objects.clone() {
                if white_list.contains(&ptr) {
                    continue;
                }
                let n = self.count_links(ptr);
                if n == 0 {
                    self.delete_object(ptr);
                    run = true;
                }
            }
        }
    }
    fn delete_object(&mut self, ptr: usize) {
        self.methods.retain(|(p, _), _| *p != ptr);
        self.fields.retain(|(p, _), _| *p != ptr);
        self.objects.remove(&ptr).unwrap();
    }
}
