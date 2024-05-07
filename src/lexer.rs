use std::ops::Deref;

use crate::parser::{Token, TokenKind};

#[derive(Debug, Clone)]
pub enum PatternKind {
    Prototype,
    Equalness,
    Keyword,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Here,
    Me,
    Return,
    Repeat,
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
    Import(String, Node),
    At(Node, Node),
    Let(String, Node),
    Set(String, Node),
    OnDo(Vec<Node>, Node),
    // NOTE: Maybe useless
    // OnRust(
    //     Vec<Node>,
    //     unsafe extern fn(State) -> (State, Result<usize, Interrupt>),
    // ),
    // TODO: Add keyword `import` or sth like that.
}

#[derive(Debug, Clone)]
pub struct Node {
    pub data: Box<NodeKind>,
    pub line: usize,
}

impl Node {
    pub fn new(data: NodeKind, line: usize) -> Self {
        Self {
            data: Box::new(data),
            line,
        }
    }
}

pub fn lex(tokens: Vec<Token>) -> Node {
    let mut i = 0;
    lex_queue(&tokens, &mut i, 0, true)
}

fn lex_queue(tokens: &Vec<Token>, i: &mut usize, line: usize, global: bool) -> Node {
    let mut queue = vec![];

    while *i < tokens.len() {
        match &tokens[*i].data {
            TokenKind::EOQ => {
                *i += 1;
            }
            TokenKind::As | TokenKind::Do => {
                panic!("Unexpected method definition keyword")
            }
            TokenKind::CloseParen | TokenKind::CloseContext if global => {
                panic!("Unexpected closing paren or brace in global context")
            }
            TokenKind::CloseParen | TokenKind::CloseContext => break,
            _ => {
                let node = match lex_message_chain(tokens, i) {
                    Some(n) => n,
                    None => continue,
                };
                queue.push(node);
            }
        }
    }
    Node::new(NodeKind::Queue(queue), line)
}

fn lex_message_chain(tokens: &Vec<Token>, i: &mut usize) -> Option<Node> {
    let recipient = lex_singleton(tokens, i)?;
    let line = recipient.line;
    let mut message = match lex_singleton(tokens, i) {
        Some(node) => Node::new(NodeKind::Message(recipient, node), line),
        None => return Some(recipient),
    };

    while *i < tokens.len() {
        match lex_singleton(tokens, i) {
            Some(node) => message = Node::new(NodeKind::Message(message, node), line),
            None => break,
        }
    }
    return Some(message);
}

fn lex_singleton(tokens: &Vec<Token>, i: &mut usize) -> Option<Node> {
    let token = tokens.get(*i)?;
    let node = match &token.data {
        TokenKind::EOQ
        | TokenKind::CloseParen
        | TokenKind::CloseContext
        | TokenKind::As
        | TokenKind::Do => None?,
        TokenKind::Here => {
            *i += 1;
            Node::new(NodeKind::Here, token.line)
        }
        TokenKind::Me => {
            *i += 1;
            Node::new(NodeKind::Me, token.line)
        }
        TokenKind::Return => {
            *i += 1;
            Node::new(NodeKind::Return, token.line)
        }
        TokenKind::Repeat => {
            *i += 1;
            Node::new(NodeKind::Repeat, token.line)
        }
        TokenKind::Name(name) => {
            *i += 1;
            let data = if name == "here" {
                NodeKind::Here
            } else {
                NodeKind::Name(name.clone())
            };
            Node::new(data, token.line)
        }
        TokenKind::Int(value) => {
            *i += 1;
            Node::new(NodeKind::Int(*value), token.line)
        }
        TokenKind::Float(value) => {
            *i += 1;
            Node::new(NodeKind::Float(*value), token.line)
        }
        TokenKind::String(string) => {
            *i += 1;
            Node::new(NodeKind::String(string.clone()), token.line)
        }
        TokenKind::OpenParen => {
            *i += 1;
            let queue = lex_queue(tokens, i, token.line, false);
            match &tokens.get(*i).expect("Paren is never closed").data {
                TokenKind::CloseParen => {
                    *i += 1;
                    queue
                }
                TokenKind::CloseContext => panic!("Unexpected closing brace"),
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
        TokenKind::OpenContext => {
            *i += 1;
            let queue = match *lex_queue(tokens, i, token.line, false).data {
                NodeKind::Queue(queue) => queue,
                _ => unreachable!(),
            };
            match &tokens.get(*i).expect("Brace is never closed").data {
                TokenKind::CloseContext => {
                    *i += 1;
                    Node::new(NodeKind::QuickContext(queue), token.line)
                }
                TokenKind::CloseParen => panic!("Unexpected closing paren"),
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
        TokenKind::Copy => {
            // "copy" SINGLETON EOQ
            *i += 1;
            let data = NodeKind::Copy(
                lex_singleton(tokens, i).expect("End of message after keyword `copy'"),
            );
            Node::new(data, token.line)
        }
        TokenKind::Import => {
            // "import" NAME SINGLETON
            *i += 1;
            let node = lex_singleton(tokens, i).expect("End of message after keyword `import'");
            let name = match *node.data {
                NodeKind::Name(name) => name,
                _ => None?,
            };
            let node = lex_singleton(tokens, i).expect("End of message after keyword `import'"); // *
            Node::new(NodeKind::Import(name, node), token.line)
        }
        TokenKind::Let => {
            // "let" NAME MESSAGE_CHAIN EOQ
            *i += 1;
            let name = match &tokens[*i].data {
                TokenKind::Name(name) => name,
                _ => panic!("Name is expected after `let' keyword"),
            };
            *i += 1;
            let node_data = match lex_message_chain(tokens, i) {
                Some(node) => NodeKind::Let(name.clone(), node),
                None => panic!("Proba error: Syntax: Empty let-statement value expression."),
            };
            Node::new(node_data, token.line)
        }
        TokenKind::Set => {
            // "set" NAME MESSAGE_CHAIN EOQ
            *i += 1;
            let name = match &tokens[*i].data {
                TokenKind::Name(name) => name,
                _ => panic!("Name is expected after `set' keyword"),
            };
            *i += 1;
            let node_data = match lex_message_chain(tokens, i) {
                Some(node) => NodeKind::Set(name.clone(), node),
                None => panic!("Proba error: Syntax: Empty let-statement value expression."),
            };
            Node::new(node_data, token.line)
        }
        TokenKind::At => {
            // "at" SINGLETON MESSAGE_CHAIN EOQ
            *i += 1;
            let context = lex_singleton(tokens, i).expect("Expecting singleton message");
            match lex_message_chain(tokens, i) {
                Some(node) => Node::new(NodeKind::At(context, node), token.line),
                None => panic!("Empty context message"), // Node::At(Box::new(context.clone()), Box::new(context)),
            }
        }
        TokenKind::On => {
            // "on" {["="|":"] MESSAGE_CHAIN ["as" NAME] ";"} ("be"|"do") MESSAGE_CHAIN EOQ
            *i += 1;
            // Patterns
            let mut patterns = vec![];
            while *i < tokens.len() {
                let token_data = &tokens.get(*i).expect("Unfinished method definition").data;
                let pattern_kind = match token_data {
                    TokenKind::Name(n) if n.as_str() == ":" => {
                        // Keyword-pattern
                        *i += 1;
                        let token = tokens.get(*i).expect("Unfinished method definition");
                        match &token.data {
                            TokenKind::Name(name) => {
                                let data = NodeKind::Name(name.clone());
                                let data = NodeKind::Pattern(
                                    PatternKind::Keyword,
                                    Node::new(data, token.line),
                                );
                                patterns.push(Node::new(data, token.line));
                                *i += 1;
                                match &tokens.get(*i).expect("Unfinished method definition").data {
                                    TokenKind::EOQ => {
                                        *i += 1;
                                        continue;
                                    }
                                    TokenKind::Do => break,
                                    t => panic!("Expecting `;', or `do' after a keyword-pattern, got: {t:?}"),
                                }
                            }
                            _ => panic!("Expecting a name after key-operator `:'"),
                        }
                    }
                    TokenKind::Name(n) if n.as_str() == "=" => {
                        *i += 1;
                        PatternKind::Equalness
                    }
                    _ => PatternKind::Prototype,
                };
                let pattern_message = lex_message_chain(tokens, i).expect("Empty pattern message");
                let node = tokens.get(*i).expect("Unfinished method definition");
                match node.data {
                    TokenKind::As => {
                        *i += 1;
                        let token = tokens.get(*i).expect("Unfinished method definition");
                        let name = if let TokenKind::Name(name) = &token.data {
                            name.clone()
                        } else {
                            panic!("Expecting a name after token `as'")
                        };
                        *i += 1;
                        let node_data = NodeKind::As(
                            Node::new(
                                NodeKind::Pattern(pattern_kind, pattern_message.clone()),
                                pattern_message.line,
                            ),
                            name,
                        );
                        patterns.push(Node::new(node_data, token.line));
                        match tokens.get(*i).expect("Unfinished method definition").data {
                            TokenKind::EOQ => {
                                *i += 1;
                                continue;
                            }
                            TokenKind::Do => break,
                            _ => panic!("Expecting `;' or one of keywords `as' and `do'"),
                        }
                    }
                    TokenKind::EOQ => {
                        patterns.push(Node::new(
                            NodeKind::Pattern(pattern_kind, pattern_message),
                            token.line,
                        ));
                        *i += 1;
                        continue;
                    }
                    TokenKind::Do => {
                        patterns.push(Node::new(
                            NodeKind::Pattern(pattern_kind, pattern_message),
                            token.line,
                        ));
                        break;
                    }
                    _ => panic!("Expecting `;' or one of keywords `as' and `do'"),
                }
            }
            let token_data = &tokens[*i].data;
            *i += 1;
            if patterns.is_empty() {
                panic!("Empty pattern in method definition")
            }
            // Body
            let body_message = lex_message_chain(tokens, i).expect(
                format!(
                    "Syntax error: on line {}: Empty body message of method definition",
                    token.line
                )
                .as_str(),
            );
            let data = match token_data {
                TokenKind::Do => {
                    expand_method_definition(NodeKind::OnDo(patterns, body_message), token.line)
                }
                _ => unreachable!(),
            };
            Node::new(data, token.line)
        }
    };
    return Some(node);
}

pub(crate) fn expand_method_definition(node_data: NodeKind, line: usize) -> NodeKind {
    match &node_data {
        NodeKind::OnDo(patterns, body) => {
            /*  on A as a; B as b do [[something]];
             * =>
             *  on A as a do {
             *      let a a;
             *      on B as b do [[something]];
             *      here
             *  };
             * [==================================]
             *  on A; B as b do [[something]];
             * =>
             *  on A do {
             *      on B as b do [[something]];
             *      here
             *  };
             */
            if patterns.len() == 1 {
                return node_data;
            }
            let mut queue_vec = Vec::new();
            match patterns[0].data.deref() {
                NodeKind::As(_, name) => {
                    let name_node = Node::new(NodeKind::Name(name.into()), line);
                    queue_vec.push(Node::new(NodeKind::Let(name.into(), name_node), line));
                }
                NodeKind::Pattern(_, _) => (),
                _ => unreachable!(),
            };
            let next_definition_node = Node::new(
                expand_method_definition(NodeKind::OnDo(patterns[1..].into(), body.clone()), line),
                line,
            );
            let here_node = Node::new(NodeKind::Here, line);
            queue_vec.append(&mut vec![next_definition_node, here_node]);
            let subcontext_node = Node::new(NodeKind::QuickContext(queue_vec), line);
            NodeKind::OnDo(vec![patterns[0].clone()], subcontext_node)
        }
        n => panic!("Wrong token type to expand method definition {n:?}"),
    }
}
