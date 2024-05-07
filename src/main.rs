use executor::Interrupt;
use std::env;
use std::process::exit;

pub mod executor;
mod lexer;
mod parser;
mod probastd;
pub mod rpmt;
pub mod vmstate;

#[derive(Debug)]
struct Config {
    file_path: Option<String>,
    args: Vec<String>,
    debug_state: bool,
    debug_answer: bool,
}

impl Config {
    pub fn new() -> Self {
        Self {
            file_path: Some(String::new()),
            args: Vec::new(),
            debug_state: false,
            debug_answer: false,
        }
    }
}

fn parse_args() -> Config {
    let mut config = Config::new();
    let mut args = env::args().collect::<Vec<String>>();
    args.remove(0);

    while args.len() > 0 && args[0].starts_with("-") {
        match args[0].as_str() {
            "-debug-state" | "-ds" => config.debug_state = true,
            "-debug-answer" | "-da" => config.debug_answer = true,
            "--" => {
                args.remove(0);
                break;
            }
            _ => break,
        }
        args.remove(0);
    }

    if args.len() == 0 {
        config.file_path = None;
    } else {
        config.file_path = Some(args.remove(0));
    }
    config.args = args;

    config
}

fn proba_error(message: &str) -> ! {
    println!("\nProba error: {message}");
    exit(0)
}

fn main() {
    const TEST_FILE_PATH: &str = "test.proba";
    let config = parse_args();

    let file_path = if let Some(fp) = config.file_path {
        fp
    } else {
        TEST_FILE_PATH.into()
    };
    let tokens = match parser::parse_file(file_path.clone()) {
        Ok(tokens) => tokens,
        Err(_) => {
            println!("Failed to open file `{file_path}'");
            exit(0)
        }
    };
    let tree = lexer::lex(tokens);
    let mut state = executor::State::new();
    probastd::define_standard(&mut state).unwrap();

    let result = executor::execute(&mut state, tree);
    if config.debug_state {
        dbg!(state);
    }
    let answer = match result {
        Ok(a) | Err(Interrupt::Exit(a) | Interrupt::Return(a)) => a,
        Err(Interrupt::Error(line, message)) => proba_error(&format!("on line {line}: {message}")),
        _ => unreachable!(),
    };
    if config.debug_answer {
        println!("\nProgram returned: {answer}");
    }
}
