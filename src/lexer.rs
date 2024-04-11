use crate::parser::{Token, TokenData};

#[derive(Debug, Clone)]
pub enum PatternKind {
    Prototype,
    Equalness,
    Keyword,
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Here,
    Name(String),
    Int(isize),
    Float(f64),
    String(String),
    Pattern(PatternKind, Node),
    As(Node, String),

    Queue(Vec<Node>),
    QuickContext(Vec<Node>),
    Message(Node, Node),
    Copy(Node),
    At(Node, Node),
    Let(String, Node),
    OnBe(Vec<Node>, Node),
    OnDo(Vec<Node>, Node),
}

#[derive(Debug, Clone)]
pub struct Node {
    pub data: Box<NodeData>,
    pub line: usize,
}

pub fn lex(tokens: Vec<Token>) -> Node {
    let mut i = 0;
    lex_queue(&tokens, &mut i, 0, true)
}

fn lex_queue(tokens: &Vec<Token>, i: &mut usize, line: usize, global: bool) -> Node {
    let mut queue = vec![];

    while *i < tokens.len() {
        match &tokens[*i].data {
            TokenData::EOQ => {
                *i += 1;
            }
            TokenData::As | TokenData::Be | TokenData::Do => {
                panic!("Unexpected method definition keyword")
            }
            TokenData::CloseParen | TokenData::CloseContext if global => {
                panic!("Unexpected closing paren or brace in global context")
            }
            TokenData::CloseParen | TokenData::CloseContext => break,
            _ => {
                let node = match lex_message_chain(tokens, i) {
                    Some(n) => n,
                    None => continue,
                };
                queue.push(node);
            }
        }
    }
    return Node {
        data: Box::new(NodeData::Queue(queue)),
        line,
    };
}

fn lex_message_chain(tokens: &Vec<Token>, i: &mut usize) -> Option<Node> {
    let recipient = lex_singleton(tokens, i)?;
    let line = recipient.line;
    let mut message = match lex_singleton(tokens, i) {
        Some(node) => Node {
            data: Box::new(NodeData::Message(recipient, node)),
            line,
        },
        None => return Some(recipient),
    };

    while *i < tokens.len() {
        match lex_singleton(tokens, i) {
            Some(node) => {
                message = Node {
                    data: Box::new(NodeData::Message(message, node)),
                    line,
                }
            }
            None => break,
        }
    }
    return Some(message);
}

fn lex_singleton(tokens: &Vec<Token>, i: &mut usize) -> Option<Node> {
    let token = tokens.get(*i)?;
    let node = match &token.data {
        TokenData::EOQ
        | TokenData::CloseParen
        | TokenData::CloseContext
        | TokenData::As
        | TokenData::Be
        | TokenData::Do => None?,
        TokenData::Name(name) => {
            *i += 1;
            let data = if name == "here" {
                NodeData::Here
            } else {
                NodeData::Name(name.clone())
            };
            Node {
                data: Box::new(data),
                line: token.line,
            }
        }
        TokenData::Int(value) => {
            *i += 1;
            Node {
                data: Box::new(NodeData::Int(*value)),
                line: token.line,
            }
        }
        TokenData::Float(value) => {
            *i += 1;
            Node {
                data: Box::new(NodeData::Float(*value)),
                line: token.line,
            }
        }
        TokenData::String(string) => {
            *i += 1;
            Node {
                data: Box::new(NodeData::String(string.clone())),
                line: token.line,
            }
        }
        TokenData::OpenParen => {
            *i += 1;
            let queue = lex_queue(tokens, i, token.line, false);
            match &tokens.get(*i).expect("Paren is never closed").data {
                TokenData::CloseParen => {
                    *i += 1;
                    queue
                }
                TokenData::CloseContext => panic!("Unexpected closing brace"),
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
        TokenData::OpenContext => {
            *i += 1;
            let queue = match *lex_queue(tokens, i, token.line, false).data {
                NodeData::Queue(queue) => queue,
                _ => unreachable!(),
            };
            match &tokens.get(*i).expect("Brace is never closed").data {
                TokenData::CloseContext => {
                    *i += 1;
                    Node {
                        data: Box::new(NodeData::QuickContext(queue)),
                        line: token.line,
                    }
                }
                TokenData::CloseParen => panic!("Unexpected closing paren"),
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
        TokenData::Copy => {
            // "copy" SINGLETON <";"|")"|"}">
            *i += 1;
            let data =
                NodeData::Copy(lex_singleton(tokens, i).expect("Semicolon after `copy' keyword"));
            Node {
                data: Box::new(data),
                line: token.line,
            }
        }
        TokenData::Let => {
            // "let" NAME MESSAGE_CHAIN <";"|")"|"}">
            *i += 1;
            let name = match &tokens[*i].data {
                TokenData::Name(name) => name,
                _ => panic!("Name is expected after `let' keyword"),
            };
            *i += 1;
            let node_data = match lex_message_chain(tokens, i) {
                Some(node) => Box::new(NodeData::Let(name.clone(), node)),
                None => None?, // TODO: Make error "empty let value"
            };
            Node {
                data: node_data,
                line: token.line,
            }
        }
        TokenData::At => {
            // "at" SINGLETON MESSAGE_CHAIN EOQ
            *i += 1;
            let context = lex_singleton(tokens, i).expect("Expecting singleton message");
            match lex_message_chain(tokens, i) {
                Some(node) => Node {
                    data: Box::new(NodeData::At(context, node)),
                    line: token.line,
                },
                None => panic!("Empty context message"), // Node::At(Box::new(context.clone()), Box::new(context)),
            }
        }
        TokenData::On => {
            // "on" {["="|":"] MESSAGE_CHAIN ["as" NAME] ";"} ("be"|"do") MESSAGE_CHAIN EOQ
            *i += 1;
            // Patterns
            let mut is_as = false;
            let mut patterns = vec![];
            while *i < tokens.len() {
                let token_data = &tokens.get(*i).expect("Unfinished method definition").data;
                let pattern_kind = match token_data {
                    TokenData::Name(n) if n.as_str() == ":" => {
                        // Keyword-pattern
                        *i += 1;
                        let token = tokens.get(*i).expect("Unfinished method definition");
                        match &token.data {
                            TokenData::Name(name) => {
                                let data = NodeData::Name(name.clone());
                                let data = NodeData::Pattern(
                                    PatternKind::Keyword,
                                    Node {
                                        data: Box::new(data),
                                        line: token.line,
                                    },
                                );
                                patterns.push(Node {
                                    data: Box::new(data),
                                    line: token.line,
                                });
                                *i += 1;
                                match &tokens.get(*i).expect("Unfinished method definition").data {
                                    TokenData::EOQ => {
                                        *i += 1;
                                        continue;
                                    },
                                    TokenData::Be | TokenData::Do => break,
                                    t => panic!("Expecting `;', `be', or `do' after a keyword-pattern, got: {:?}", t),
                                }
                            }
                            _ => panic!("Expecting a name after key-operator `:'"),
                        }
                    }
                    TokenData::Name(n) if n.as_str() == "=" => {
                        *i += 1;
                        PatternKind::Equalness
                    }
                    _ => PatternKind::Prototype,
                };
                let pattern_message = lex_message_chain(tokens, i).expect("Empty pattern message");
                let node = tokens.get(*i).expect("Unfinished method definition");
                match node.data {
                    TokenData::As => {
                        is_as = true;
                        *i += 1;
                        let token = tokens.get(*i).expect("Unfinished method definition");
                        let name = if let TokenData::Name(name) = &token.data {
                            name.clone()
                        } else {
                            panic!("Expecting a name after token `as'")
                        };
                        *i += 1;
                        let node_data = NodeData::As(
                            Node {
                                data: Box::new(NodeData::Pattern(
                                    pattern_kind,
                                    pattern_message.clone(),
                                )),
                                line: pattern_message.line,
                            },
                            name,
                        );
                        patterns.push(Node {
                            data: Box::new(node_data),
                            line: token.line,
                        });
                        match tokens.get(*i).expect("Unfinished method definition").data {
                            TokenData::EOQ => {
                                *i += 1;
                                continue;
                            }
                            TokenData::Be | TokenData::Do => break,
                            _ => panic!("Expecting `;' or one of keywords `as', `be', and `do'"),
                        }
                    }
                    TokenData::EOQ => {
                        patterns.push(Node {
                            data: Box::new(NodeData::Pattern(pattern_kind, pattern_message)),
                            line: token.line,
                        });
                        *i += 1;
                        continue;
                    }
                    TokenData::Be | TokenData::Do => {
                        patterns.push(Node {
                            data: Box::new(NodeData::Pattern(pattern_kind, pattern_message)),
                            line: token.line,
                        });
                        break;
                    }
                    _ => panic!("Expecting `;' or one of keywords `as', `be', and `do'"),
                }
            }
            let token_data = &tokens[*i].data;
            *i += 1;
            if patterns.is_empty() {
                panic!("Empty pattern in method definition")
            }
            // Body
            let body_message =
                lex_message_chain(tokens, i).expect("Empty body message of method definition");
            let data = match token_data {
                TokenData::Be if !is_as => {
                    expand_method_definition(NodeData::OnBe(patterns, body_message), token.line)
                }
                TokenData::Be => panic!("Method that contains as-pattern must have do-body"),
                TokenData::Do => NodeData::OnDo(patterns, body_message),
                _ => unreachable!(),
            };
            Node {
                data: Box::new(data),
                line: token.line,
            }
        }
    };
    return Some(node);
}

fn expand_method_definition(node_data: NodeData, line: usize) -> NodeData {
    match &node_data {
        NodeData::OnBe(patterns, body) => {
            if patterns.len() == 1 {
                return node_data;
            }
            // Every pattern is a keyword
            // on : a, : b be {};
            // =>
            // on : a be at copy Object on : b be {};
            let next_definition_node = Node {
                data: Box::new(expand_method_definition(
                    NodeData::OnBe(patterns[1..].into(), body.clone()),
                    line,
                )),
                line,
            };
            let object_node = Node {
                data: Box::new(NodeData::Name("Object".into())),
                line,
            };
            let copy_object_node = Node {
                data: Box::new(NodeData::Copy(object_node)),
                line,
            };
            let at_node = Node {
                data: Box::new(NodeData::At(copy_object_node, next_definition_node)),
                line,
            };
            NodeData::OnBe(vec![patterns[0].clone()], at_node)
        }
        NodeData::OnDo(patterns, body) => {
            /*  on A as a; B as b do {};
             * =>
             *  on A as a do at copy Object (
             *      let a a;
             *      on B as b do {};
             *  );
             */
            if patterns.len() == 1 {
                return node_data;
            }
            let next_definition_node = Node {
                data: Box::new(expand_method_definition(
                    NodeData::OnDo(patterns[1..].into(), body.clone()),
                    line,
                )),
                line,
            };
            let object_node = Node {
                data: Box::new(NodeData::Name("Object".into())),
                line,
            };
            let copy_object_node = Node {
                data: Box::new(NodeData::Copy(object_node)),
                line,
            };
            let at_node = Node {
                data: Box::new(NodeData::At(copy_object_node, next_definition_node)),
                line,
            };
            NodeData::OnDo(vec![patterns[0].clone()], at_node)
        }
        _ => panic!("Wrong token type to expand method definition"),
    }
}
