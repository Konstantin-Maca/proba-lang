use std::env;
use std::process::exit;
use executor::Interrupt;

mod parser;
mod lexer;
pub mod executor;
pub mod vmstate;
pub mod rpmt;

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
    let tokens = parser::parse_file(file_path.into());
    let tree = lexer::lex(tokens);
    let mut state = executor::State::new();
    let result = executor::execute(&mut state, tree);
    if config.debug_state {
        dbg!(state);
    }
    let answer = match result {
        Ok(a) | Err(Interrupt::Exit(a) | Interrupt::Return(a)) => a,
        Err(Interrupt::Error(line, message)) => proba_error(&format!("on line {line}: {message}")),
        Err(Interrupt::Repeat) => unreachable!(),
    };
    if config.debug_answer {
        println!("\nProgram returned: {answer}");
    }
}
