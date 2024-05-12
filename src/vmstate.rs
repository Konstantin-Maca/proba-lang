use crate::executor::{Interrupt, CURRENT_FILE_PATH};
use crate::lexer::Node;

#[derive(Debug, Clone, Copy)]
pub enum Value {
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
pub enum Pattern {
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
pub enum Body {
    Do(Node),
    Rust(fn(&mut State) -> Result<usize, Interrupt>),
}

#[derive(Debug, Clone)]
pub struct State {
    pub op_count: usize,
    pub contexts: Vec<(usize, bool)>, // Context (ptr, is pushed for method?)

    pub objects: Vec<(usize, usize, usize)>, // (ptr, parent_ptr, cotnext_ptr)
    pub fields: Vec<(usize, String, Value)>, // (owner_ptr, name, ptr|int|float)
    pub methods: Vec<(usize, Pattern, Body, String)>, // (owner_ptr, pattern, body, file)
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

    pub fn clone_from(&mut self, other: &Self) {
        self.op_count = other.op_count;
        self.contexts = other.contexts.clone();
        self.objects = other.objects.clone();
        self.fields = other.fields.clone();
        self.methods = other.methods.clone();
    }

    pub fn here(&self) -> Option<usize> {
        Some(self.contexts.last()?.0)
    }
    /// Return None, when was called at not method.
    pub fn recipient(&self) -> Option<usize> {
        for (ptr, is_for_method) in self.contexts.iter().rev() {
            if *is_for_method {
                return self.parent(*ptr);
            }
        }
        None
    }
    pub fn copy(&mut self, ptr: usize) -> Option<usize> {
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
            None?;
        }
        Some(self.relation(self.parent(ptr)?, parent_ptr)? + 1)
    }
    pub fn relation_ctx(&self, ptr: usize, super_context_ptr: usize) -> Option<usize> {
        if ptr == super_context_ptr {
            return Some(0);
        }
        if ptr == 1 || ptr == 0 {
            None?;
        }
        Some(self.relation_ctx(self.context_of(ptr)?, super_context_ptr)? + 1)
    }

    pub fn parent(&self, ptr: usize) -> Option<usize> {
        Some(self.objects.iter().find(|obj| obj.0 == ptr)?.1)
    }
    pub fn context_of(&self, ptr: usize) -> Option<usize> {
        Some(self.objects.iter().find(|obj| obj.0 == ptr)?.2)
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
    pub fn get_field(&self, ptr: usize, name: String) -> Option<(usize, Value)> {
        // println!("GET FIELD OF {ptr} NAMED {name}...");
        match self
            .fields
            .iter()
            .find(|field| (field.0, field.1.clone()) == (ptr, name.clone()))
        {
            Some(field) => {
                let result = Some((field.0, field.2));
                // println!("GET FIELD OF {ptr} NAMED {name} => {result:?}");
                result
            }
            None => {
                if ptr == 0 {
                    // println!("GET FIELD OF {ptr} NAMED {name} => None");
                    None?
                } else {
                    let parent_ptr = self.objects.iter().find(|obj| obj.0 == ptr)?.1;
                    let result = self.get_field(parent_ptr, name.clone());
                    // println!("GET FIELD OF {ptr} NAMED {name} => {result:?}");
                    result
                }
            }
        }
    }
    pub fn get_field_value(&self, ptr: usize, name: String) -> Option<Value> {
        // println!("GET FIELD VALUE...");
        Some(self.get_field(ptr, name)?.1)
    }
    pub fn get_field_value_ctx(&self, name: String) -> Option<Value> {
        // You can get field in a context-object,
        // only if you entered into it from another context,
        // that is a copy of the current context-object's creation context.
        // Exception: the global context.
        // println!("GET FIELD VALUE OF CTX NAMED {name}...");
        let prev_context = if self.contexts.len() <= 1 {
            None
        } else {
            Some(self.contexts[self.contexts.len() - 2].0)
        };

        for (ptr, is_for_method) in self.contexts.iter().rev() {
            // println!("Next context: {ptr}");
            match self.get_field(*ptr, name.clone()) {
                Some((ptr, value)) if self.have_access_premission(prev_context, ptr) => {
                    // println!("GET FIELD VALUE OF CTX NAMED {} => {:?}", name, Some(value));
                    return Some(value);
                }
                _ => (),
            }
            if *is_for_method {
                break;
            }
        }
        let result = self.get_field_value(self.contexts[0].0, name.clone());
        // println!("GET FIELD VALUE OF CTX NAMED {name} => {result:?}");
        result
    }

    fn have_access_premission(&self, prev_context: Option<usize>, ptr: usize) -> bool {
        if prev_context.is_none() {
            // println!("HAVE ACCESS FROM {prev_context:?} AT {ptr} => true (GLOBAL)");
            true
        } else if self.relation(prev_context.unwrap(), ptr).is_some() {
            // println!("HAVE ACCESS FROM {prev_context:?} AT {ptr} => true (1 -> 2)");
            true
        } else {
            let context_of_target_ptr = self
                .context_of(ptr)
                .expect(&format!("Failed to get context of object #{ptr}."));
            let result = self
                .relation(prev_context.unwrap(), context_of_target_ptr)
                .is_some();
            // println!("HAVE ACCESS FROM {prev_context:?} AT {ptr} (_, _, {context_of_target_ptr}) => {result}");
            result
        }
    }

    /// Return true, if method is re-defined;
    /// return false, if new method is defined.
    pub fn define_method(&mut self, ptr: usize, pattern: Pattern, body: Body) -> bool {
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
        self.methods
            .push((ptr, pattern, body, unsafe { CURRENT_FILE_PATH.clone() }));
        redefined
    }
    /// Use when message is a name (word (keyword)).
    pub fn get_method(
        &self,
        ptr: usize,
        keyword: String,
    ) -> Option<&(usize, Pattern, Body, String)> {
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
    pub fn get_method_ctx(&self, keyword: String) -> Option<&(usize, Pattern, Body, String)> {
        for &(ptr, is_for_method) in self.contexts.iter().rev() {
            match self.get_method(ptr, keyword.clone()) {
                Some(method) => return Some(method),
                None => (),
            }
            if is_for_method {
                break;
            }
        }
        self.get_method(self.contexts.first()?.0, keyword)
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
    pub(crate) fn clear_garbage(&mut self, white_list: Vec<usize>) {
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
