use std::fs;

use regex::Regex;

#[derive(Debug, Clone)]
pub enum TokenKind {
    Name(String),
    Int(isize),
    Float(f64),
    String(String),
    OpenParen,
    CloseParen,
    OpenContext,
    CloseContext,
    EOQ,
    Here,
    Me,
    Copy,
    At,
    Let,
    Set,
    On,
    Do,
    As,
    Return, // NOTE: I've just realized, that it may be useless
    Repeat,
    Import,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub data: TokenKind,
    pub line: usize,
}

pub fn parse_file(file_path: String) -> Result<Vec<Token>, std::io::Error> {
    let contents = fs::read_to_string(file_path)?;
    Ok(parse_str(&contents))
}

pub fn parse_str(string: &str) -> Vec<Token> {
    let token_re =
        Regex::new(r#"(\[\[[^\]]*\]\]|"[^"]+"|[(){};]|[\w~!@#$%^&*\-+=:|\\,.\/<>?]+|\[|\]|\s+|.)"#)
            .unwrap();
    let string_re = Regex::new(r#"^"[^"]+"$"#).unwrap();
    let comment_re = Regex::new(r#"^\[\[[^\]]*\]\]$"#).unwrap();

    let mut tokens = Vec::new();
    let mut line_count = 0usize;
    for cap in token_re.captures_iter(&string) {
        let token = match &cap[0] {
            "(" => TokenKind::OpenParen,
            ")" => TokenKind::CloseParen,
            "{" => TokenKind::OpenContext,
            "}" => TokenKind::CloseContext,
            ";" => TokenKind::EOQ,
            "here" => TokenKind::Here,
            "me" => TokenKind::Me,
            "copy" => TokenKind::Copy,
            "at" => TokenKind::At,
            "let" => TokenKind::Let,
            "set" => TokenKind::Set,
            "on" | "[" => TokenKind::On,
            "do" | "]" => TokenKind::Do,
            "as" => TokenKind::As,
            "return" => TokenKind::Return,
            "repeat" => TokenKind::Repeat,
            "import" => TokenKind::Import,
            s => {
                for c in s.chars() {
                    if c == '\n' {
                        line_count += 1;
                    }
                }

                if s.trim().is_empty() || comment_re.is_match(s) {
                    continue;
                } else if string_re.is_match(s) {
                    TokenKind::String(s[1..s.len() - 1].into())
                } else if let Ok(i) = s.parse::<isize>() {
                    TokenKind::Int(i)
                } else if let Ok(f) = s.parse::<f64>() {
                    TokenKind::Float(f)
                } else {
                    TokenKind::Name(s.into())
                }
            }
        };
        tokens.push(Token {
            data: token,
            line: line_count,
        });
    }

    return tokens;
}
