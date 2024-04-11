use std::collections::HashMap;

use crate::lexer::Node;

use super::{standard::prepare_std, Interrupt};

#[derive(Debug, Clone, Copy)]
pub(super) enum Value {
    Pointer(usize),
    Int(isize),
    Float(f64),
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
pub(super) enum MethodBody {
    Be(usize),
    Do(Node),
    Rust(fn(&mut State) -> Result<usize, Interrupt>),
}

#[derive(Debug)]
pub(crate) struct State {
    pub(super) op_count: usize,
    pub(super) cstack: Vec<usize>,           // Context
    pub(super) astack: Vec<(String, usize)>, // Arguments (alias, ptr)

    // ptr => (parent, context)
    pub(super) objects: HashMap<usize, (usize, usize)>,
    // (owner, name) => { pointer | int | float }
    pub(super) fields: HashMap<(usize, String), Value>,
    // (owner, pattern) => { pointer | node }
    pub(super) methods: HashMap<(usize, Pattern), MethodBody>,
}

impl State {
    pub fn new() -> Self {
        Self {
            op_count: 0,
            cstack: vec![],
            astack: vec![],
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
        for p in &self.cstack {
            if *p == ptr {
                count += 1;
            }
        }

        count
    }

    pub(super) fn copy_object(&mut self, ptr: usize) -> Option<usize> {
        self.objects.get(&ptr)?;
        let new_ptr = self.op_count;
        self.op_count += 1;
        self.objects
            .insert(new_ptr, (ptr, *self.cstack.last().unwrap()));
        return Some(new_ptr);
    }

    fn relation(&self, ptr: usize, parent_ptr: usize) -> Option<usize> {
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

    /// Return true if success, else false.
    pub(super) fn set_field_value(&mut self, ptr: usize, name: String, value: Value) -> bool {
        if !self.objects.contains_key(&ptr) {
            return false;
        }
        match self.fields.get_mut(&(ptr, name.clone())) {
            Some(val) => *val = value,
            None => {
                self.fields.insert((ptr, name.clone()), value);
            }
        };
        true
    }
    fn get_field_value(&self, ptr: usize, name: &String) -> Option<Value> {
        let arg = self.astack.last().unwrap();
        if name == "me" {
            Some(Value::Pointer(*self.cstack.last().unwrap()))
        } else if arg.0 == *name {
            Some(Value::Pointer(arg.1))
        } else {
            match self.fields.get(&(ptr, name.clone())) {
                Some(val) => Some(*val),
                None => {
                    if ptr == 0 {
                        None?
                    } else {
                        let parent_ptr = self.objects.get(&ptr)?.0;
                        self.get_field_value(parent_ptr, name)
                    }
                }
            }
        }
    }
    pub(super) fn get_context_field_value(&self, name: &String) -> Option<Value> {
        let top_context = self.cstack.last().unwrap();
        for ptr in self.cstack.iter().rev() {
            self.relation(*top_context, *ptr)?;
            match self.get_field_value(*ptr, name) {
                Some(value) => return Some(value),
                None => continue,
            }
        } // NOTE: This loop may be useless because of inheritance.
        self.get_field_value(self.cstack[0], name)
    }

    /// Return true, if method is re-defined;
    /// return false, if new method is defined.
    pub(super) fn define_method(&mut self, ptr: usize, pattern: Pattern, body: MethodBody) -> bool {
        match self.methods.insert((ptr, pattern), body) {
            Some(_) => true,
            None => false,
        }
    }
    pub(super) fn get_method(
        &self,
        ptr: usize,
        pattern: &Pattern,
    ) -> Option<(&(usize, Pattern), &MethodBody)> {
        match self.methods.get_key_value(&(ptr, pattern.clone())) {
            Some(method) => Some(method),
            None => {
                if ptr == 0 {
                    None?
                } else {
                    let parent_ptr = self.objects.get(&ptr)?.0;
                    self.get_method(parent_ptr, pattern)
                }
            }
        }
    }
    pub(super) fn get_context_method(
        &self,
        pattern: &Pattern,
    ) -> Option<(&(usize, Pattern), &MethodBody)> {
        let ptr = self.cstack.last()?;
        match self.get_method(*ptr, pattern) {
            Some(method) => Some(method),
            None => self.get_method(self.cstack[0], pattern),
        }
    }
    pub(super) fn match_method(
        &self,
        ptr: usize,
        message: usize,
    ) -> Option<(&(usize, Pattern), &MethodBody)> {
        for method in self.methods.iter() {
            match method.0 .1 {
                Pattern::Eq(_pattern) => {
                    println!("Realize equalness");
                    continue;
                    // IDEA: If one of objects has keyword-method "==", call it, else compare pointers
                }
                Pattern::EqA(_pattern, _) => {
                    println!("Realize equalness");
                    continue;
                }
                Pattern::Pt(pattern)
                    if method.0 .0 == ptr && self.relation(message, pattern).is_some() =>
                {
                    return Some(method);
                }
                Pattern::PtA(pattern, _)
                    if method.0 .0 == ptr && self.relation(message, pattern).is_some() =>
                {
                    return Some(method);
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
