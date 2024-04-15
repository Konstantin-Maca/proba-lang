use regex::Regex;
use std::fs;

#[derive(Debug, Clone)]
pub enum TokenData {
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
}

#[derive(Debug, Clone)]
pub struct Token {
    pub data: TokenData,
    pub line: usize,
}

pub fn parse_file(file_path: String) -> Vec<Token> {
    let contents = fs::read_to_string(file_path).unwrap();
    parse_str(&contents)
}

pub fn parse_str(string: &str) -> Vec<Token> {
    let token_re =
        Regex::new(r#"(\[[^\]]*\]|"[^"]+"|[(){};]|[\w~!@#$%^&*\-+=:|\\,.\/<>?]+|\s+|.)"#).unwrap();
    let string_re = Regex::new(r#"^"[^"]+"$"#).unwrap();
    let comment_re = Regex::new(r#"^\[[^\]]*\]$"#).unwrap();

    let mut tokens = Vec::new();
    let mut line_count = 0usize;
    for cap in token_re.captures_iter(&string) {
        let token = match &cap[0] {
            "(" => TokenData::OpenParen,
            ")" => TokenData::CloseParen,
            "{" => TokenData::OpenContext,
            "}" => TokenData::CloseContext,
            ";" => TokenData::EOQ,
            "here" => TokenData::Here,
            "me" => TokenData::Me,
            "copy" => TokenData::Copy,
            "at" => TokenData::At,
            "let" => TokenData::Let,
            "set" => TokenData::Set,
            "on" => TokenData::On,
            "do" => TokenData::Do,
            "as" => TokenData::As,
            "return" => TokenData::Return,
            "repeat" => TokenData::Repeat,
            s => {
                for c in s.chars() {
                    if c == '\n' {
                        line_count += 1;
                    }
                }

                if s.trim().is_empty() || comment_re.is_match(s) {
                    continue;
                } else if string_re.is_match(s) {
                    TokenData::String(s[1..s.len() - 1].into())
                } else if let Ok(i) = s.parse::<isize>() {
                    TokenData::Int(i)
                } else if let Ok(f) = s.parse::<f64>() {
                    TokenData::Float(f)
                } else {
                    TokenData::Name(s.into())
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
