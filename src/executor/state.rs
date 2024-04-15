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

#[derive(Debug, Clone, Eq, Hash)]
pub(super) enum Pattern {
    Kw(String),
    Eq(usize),
    EqA(usize, String),
    Pt(usize),
    PtA(usize, String),
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Kw(l0), Self::Kw(r0)) => l0 == r0,
            (Self::Eq(l0) | Self::EqA(l0, _), Self::Eq(r0) | Self::EqA(r0, _)) => l0 == r0,
            (Self::Pt(l0) | Self::PtA(l0, _), Self::Pt(r0) | Self::PtA(r0, _)) => l0 == r0,
            _ => false,
        }
    }
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

    pub(crate) objects: Vec<(usize, usize, usize)>, // (ptr, parent_ptr, cotnext_ptr)
    pub(super) fields: Vec<(usize, String, Value)>, // (owner_ptr, name, ptr|int|float)
    pub(super) methods: Vec<(usize, Pattern, Body)>, // (owner_ptr, pattern, body)
}

impl State {
    pub fn new() -> Self {
        Self {
            op_count: 0,
            contexts: Vec::new(),
            objects: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
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
                return Some(self.objects.iter().find(|obj| obj.0 == *ptr)?.1);
            }
        }
        None // NOTE: In theory, this is unreachable, but I'm not sure.
    }
    pub(super) fn copy(&mut self, ptr: usize) -> Option<usize> {
        self.objects.iter().find(|obj| obj.0 == ptr)?;
        let new_ptr = self.op_count;
        self.op_count += 1;
        self.objects
            .push((new_ptr, ptr, self.contexts.last().unwrap().0));
        return Some(new_ptr);
    }
    pub fn relation(&self, ptr: usize, parent_ptr: usize) -> Option<usize> {
        if ptr == parent_ptr {
            return Some(0);
        }
        if ptr == 0 {
            return None;
        }
        match self.objects.iter().find(|obj| obj.0 == ptr) {
            Some((_, ptr, _)) => Some(self.relation(*ptr, parent_ptr)? + 1),
            None => None,
        }
    }

    /// Return Some if success, else None.
    pub fn let_field(&mut self, ptr: usize, name: String, value: Value) -> Option<()> {
        self.objects.iter().find(|obj| obj.0 == ptr)?;
        // get_mut(&(ptr, name.clone()))
        match self
            .fields
            .iter_mut()
            .find(|field| (field.0, field.1.clone()) == (ptr, name.clone()))
        {
            Some(field) => field.2 = value,
            None => self.fields.push((ptr, name.clone(), value)),
        };
        Some(())
    }
    pub fn set_field(&mut self, ptr: usize, name: String, value: Value) -> Option<()> {
        let field = self
            .fields
            .iter_mut()
            .find(|field| (field.0, field.1.clone()) == (ptr, name.clone()))?;
        (*field).2 = value;
        Some(())
    }
    pub fn get_field(&self, ptr: usize, name: String) -> Option<Value> {
        match self
            .fields
            .iter()
            .find(|field| (field.0, field.1.clone()) == (ptr, name.clone()))
        {
            Some(field) => Some(field.2),
            None => {
                if ptr == 0 {
                    None?
                } else {
                    let parent_ptr = self.objects.iter().find(|obj| obj.0 == ptr)?.1;
                    self.get_field(parent_ptr, name)
                }
            }
        }
    }
    pub fn get_field_ctx(&self, name: String) -> Option<Value> {
        // You can get field in a context-object,
        // only if you entered into it from another context,
        // that is a copy of the current context-object's creation context.
        // Exception: the global context.
        let here = self.here().unwrap();
        let heres_context = self.objects.iter().find(|obj| obj.0 == here)?.2;
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
        // Check if same method is already defined
        let some_method_pos = self
            .methods
            .iter()
            .position(|method| (method.0, method.1.clone()) == (ptr, pattern.clone()));
        let redefined = if let Some(index) = some_method_pos {
            self.methods.remove(index);
            true
        } else {
            false
        };
        self.methods.push((ptr, pattern, body));
        redefined
    }
    /// Use when message is a name (word (keyword)).
    pub(super) fn get_method(
        &self,
        ptr: usize,
        keyword: String,
    ) -> Option<&(usize, Pattern, Body)> {
        match self
            .methods
            .iter()
            .find(|method| (method.0, method.1.clone()) == (ptr, Pattern::Kw(keyword.clone())))
        {
            Some(method) => Some(method),
            None => {
                if ptr == 0 {
                    None?
                } else {
                    let parent_ptr = self.objects.iter().find(|obj| obj.0 == ptr)?.1;
                    self.get_method(parent_ptr, keyword)
                }
            }
        }
    }
    /// Use when message is a name (word (keyword)).
    pub(super) fn get_method_ctx(&self, keyword: String) -> Option<&(usize, Pattern, Body)> {
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
    ) -> Option<(usize, Pattern, Body)> {
        for (owner_ptr, pattern, body) in self.methods.clone().iter() {
            match pattern {
                Pattern::Eq(pattern_ptr) | Pattern::EqA(pattern_ptr, ..)
                    if *owner_ptr == ptr && {
                        // pattern_ptr == message
                        let method = self.get_method(*pattern_ptr, "==".into()).unwrap();
                        let ptr =
                            execute_method(self, *pattern_ptr, method.2.clone(), ("".into(), 0))
                                .unwrap();

                        let method = self.match_method(ptr, message).unwrap();
                        let arg_name = match &method.1 {
                            Pattern::Eq(_) | Pattern::Pt(_) => "".into(),
                            Pattern::EqA(_, name) | Pattern::PtA(_, name) => name.clone(),
                            _ => unreachable!(),
                        };
                        let result_ptr =
                            execute_method(self, ptr, method.2.clone(), (arg_name, message));

                        result_ptr.unwrap()
                            == self.get_field(1, "True".into()).unwrap().unwrap_ptr()
                    } =>
                {
                    return Some((*owner_ptr, pattern.clone(), body.clone()));
                }
                Pattern::Pt(pattern_ptr) | Pattern::PtA(pattern_ptr, ..)
                    if *owner_ptr == ptr && self.relation(message, *pattern_ptr).is_some() =>
                {
                    return Some((*owner_ptr, pattern.clone(), body.clone()));
                }
                _ => continue,
            }
        }
        if ptr == 0 {
            None?
        } else {
            let parent_ptr = self.objects.iter().find(|obj| obj.0 == ptr)?.1;
            self.match_method(parent_ptr, message)
        }
    }

    fn count_links(&self, ptr: usize) -> usize {
        let mut count = 0;
        // As parent and context owner
        for (_, parent, context) in &self.objects {
            if *parent == ptr {
                count += 1;
            }
            if *context == ptr {
                count += 1;
            }
        }
        // As field value
        for field in &self.fields {
            if let Value::Pointer(p) = field.2 {
                if p == ptr {
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
            for (_, ptr, _) in self.objects.clone() {
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
        self.methods.retain(|method| method.0 != ptr);
        self.fields.retain(|field| field.0 != ptr);
        self.objects.retain(|obj| obj.0 != ptr);
    }
}
