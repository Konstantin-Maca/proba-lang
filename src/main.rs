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
    pub file_path: Option<String>,
    pub args: Vec<String>,
    pub debug_state: bool,
    pub debug_answer: bool,
}

impl Config {
    pub const fn new() -> Self {
        Self {
            file_path: Some(String::new()),
            args: Vec::new(),
            debug_state: false,
            debug_answer: false,
        }
    }
}

fn parse_args(config: &mut Config) {
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
}

fn proba_error(message: &str) -> ! {
    println!("\nProba error: {message}");
    exit(0)
}

pub(crate) static mut PROG_CONFIG: Config = Config::new();

fn main() {
    const TEST_FILE_PATH: &str = "test.proba";
    parse_args(unsafe { &mut PROG_CONFIG });

    let file_path = if let Some(fp) = unsafe { PROG_CONFIG.file_path.clone() } {
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
    let mut state = vmstate::State::new();
    probastd::define_standard(&mut state).unwrap();

    let result = executor::execute(&mut state, tree);
    if unsafe { PROG_CONFIG.debug_state } {
        dbg!(state);
    }
    let answer = match result {
        Ok(a) | Err(Interrupt::Exit(a) | Interrupt::Return(a)) => a,
        Err(Interrupt::Error(line, message)) => proba_error(&format!("on line {line}: {message}")),
        _ => unreachable!(),
    };
    if unsafe { PROG_CONFIG.debug_answer } {
        println!("\nProgram returned: {answer}");
    }
}
