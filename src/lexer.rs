use std::{ops::Deref, process::exit};

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
                syntax_error(line, "Unexpected method definition keyword.".into());
            }
            TokenKind::CloseParen | TokenKind::CloseContext if global => {
                syntax_error(
                    line,
                    "Unexpected closing paren or brace in global context.".into(),
                );
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
            let t = match tokens.get(*i) {
                Some(t) => &t.data,
                None => syntax_error(token.line, "Paren is never closed".into()),
            };
            match t {
                TokenKind::CloseParen => {
                    *i += 1;
                    queue
                }
                TokenKind::CloseContext => {
                    syntax_error(token.line, "Unexpected closing brace".into())
                }
                t => unreachable!("Unexpected token: {:?}", t),
            }
        }
        TokenKind::OpenContext => {
            *i += 1;
            let queue = match *lex_queue(tokens, i, token.line, false).data {
                NodeKind::Queue(queue) => queue,
                _ => unreachable!("UNREACHABLE"),
            };
            let token_kind = match tokens.get(*i) {
                Some(val) => &val.data,
                None => syntax_error(token.line, "Brace is never closed.".into()),
            };
            match token_kind {
                TokenKind::CloseContext => {
                    *i += 1;
                    Node::new(NodeKind::QuickContext(queue), token.line)
                }
                TokenKind::CloseParen => {
                    syntax_error(token.line, "Unexpected closing paren.".into())
                }
                t => unreachable!("UNREACHABLE: Unexpected token: {:?}.", t),
            }
        }
        TokenKind::Copy => {
            // "copy" SINGLETON EOQ
            *i += 1;
            let data = NodeKind::Copy(match lex_singleton(tokens, i) {
                Some(val) => val,
                None => syntax_error(token.line, "Unexpected end of copy-statement".into()),
            });
            Node::new(data, token.line)
        }
        TokenKind::Import => {
            // "import" NAME SINGLETON
            *i += 1;
            let node = match lex_singleton(tokens, i) {
                Some(val) => val,
                None => syntax_error(token.line, "Unexpected end of import-statement".into()),
            };
            let name = match *node.data {
                NodeKind::Name(name) => name,
                _ => None?,
            };
            let node = match lex_singleton(tokens, i) {
                Some(val) => val,
                None => syntax_error(token.line, "Unexpected end of import-statement.".into()),
            };
            Node::new(NodeKind::Import(name, node), token.line)
        }
        TokenKind::Let => {
            // "let" NAME MESSAGE_CHAIN EOQ
            *i += 1;
            let name = match &tokens[*i].data {
                TokenKind::Name(name) => name,
                _ => syntax_error(token.line, "Unexpected end of let-statement.".into()),
            };
            *i += 1;
            let node_data = match lex_message_chain(tokens, i) {
                Some(node) => NodeKind::Let(name.clone(), node),
                None => syntax_error(token.line, "Unexpected end of let-statement.".into()),
            };
            Node::new(node_data, token.line)
        }
        TokenKind::Set => {
            // "set" NAME MESSAGE_CHAIN EOQ
            *i += 1;
            let name = match &tokens[*i].data {
                TokenKind::Name(name) => name,
                _ => syntax_error(token.line, "Name is expected after `set' keyword.".into()),
            };
            *i += 1;
            let node_data = match lex_message_chain(tokens, i) {
                Some(node) => NodeKind::Set(name.clone(), node),
                None => syntax_error(token.line, "Unexpected end of set-statement.".into()),
            };
            Node::new(node_data, token.line)
        }
        TokenKind::At => {
            // "at" SINGLETON MESSAGE_CHAIN EOQ
            *i += 1;
            let context = match lex_singleton(tokens, i) {
                Some(val) => val,
                None => syntax_error(token.line, "Expecting singleton message.".into()),
            };
            match lex_message_chain(tokens, i) {
                Some(node) => Node::new(NodeKind::At(context, node), token.line),
                None => syntax_error(token.line, "Empty body of at-statement.".into()),
            }
        }
        TokenKind::On => {
            // "on" {["="|":"] MESSAGE_CHAIN ["as" NAME] ";"} ("be"|"do") MESSAGE_CHAIN EOQ
            *i += 1;
            // Patterns
            let mut patterns = vec![];
            while *i < tokens.len() {
                let token_data = match tokens.get(*i) {
                    Some(val) => &val.data,
                    None => syntax_error(token.line, "Unfinished method definition.".into()),
                };
                let pattern_kind = match token_data {
                    TokenKind::Name(n) if n.as_str() == ":" => {
                        // Keyword-pattern
                        *i += 1;
                        let token = match tokens.get(*i) {
                            Some(val) => val,
                            None => {
                                syntax_error(token.line, "Unfinished method definition.".into())
                            }
                        };
                        match &token.data {
                            TokenKind::Name(name) => {
                                let data = NodeKind::Name(name.clone());
                                let data = NodeKind::Pattern(
                                    PatternKind::Keyword,
                                    Node::new(data, token.line),
                                );
                                patterns.push(Node::new(data, token.line));
                                *i += 1;
                                let token_kind = match tokens.get(*i) {
                                    Some(val) => &val.data,
                                    None => syntax_error(
                                        token.line,
                                        "Unfinished method definition.".into(),
                                    ),
                                };
                                match token_kind {
                                    TokenKind::EOQ => {
                                        *i += 1;
                                        continue;
                                    }
                                    TokenKind::Do => break,
                                    _ => syntax_error(token.line, "Expecting `;', or `do' after a keyword-pattern, got: {t:?}".into()),
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
                let pattern_message = match lex_message_chain(tokens, i) {
                    Some(val) => val,
                    None => syntax_error(token.line, "Empty pattern message.".into()),
                };
                let node = {
                    match tokens.get(*i) {
                        Some(val) => val,
                        None => syntax_error(token.line, "Unfinished method definition.".into()),
                    }
                };
                match node.data {
                    TokenKind::As => {
                        *i += 1;
                        let token = match tokens.get(*i) {
                            Some(val) => val,
                            None => {
                                syntax_error(token.line, "Unfinished method definition.".into())
                            }
                        };
                        let name = if let TokenKind::Name(name) = &token.data {
                            name.clone()
                        } else {
                            syntax_error(token.line, "Expecting a name after token `as'.".into())
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
                        let token_kind = match tokens.get(*i) {
                                Some(val) => &val.data,
                                None => {
                                    syntax_error(token.line, "Unfinished method definition.".into())
                                }
                            };
                        match token_kind {
                            TokenKind::EOQ => {
                                *i += 1;
                                continue;
                            }
                            TokenKind::Do => break,
                            _ => syntax_error(
                                token.line,
                                "Expecting `;' or one of keywords `as' and `do'.".into(),
                            ),
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
                    _ => syntax_error(
                        token.line,
                        "Expecting `;' or one of keywords `as' and `do'".into(),
                    ),
                }
            }
            let token_data = &tokens[*i].data;
            *i += 1;
            if patterns.is_empty() {
                syntax_error(token.line, "Empty pattern in method definition.".into())
            }
            // Body
            let body_message = match lex_message_chain(tokens, i) {
                Some(val) => val,
                None => syntax_error(
                    token.line,
                    "Empty body message of method definition.".into(),
                ),
            };
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

fn syntax_error(line: usize, message: String) -> ! {
    println!("Syntax error on line {line}: {message}");
    exit(0)
}
