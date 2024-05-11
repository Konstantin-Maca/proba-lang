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
    pub debug_context: bool,
}

impl Config {
    pub const fn new() -> Self {
        Self {
            file_path: Some(String::new()),
            args: Vec::new(),
            debug_state: false,
            debug_answer: false,
            debug_context: false,
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
            "-debug-context" | "-dc" => config.debug_context = true,
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

fn proba_error(line: usize, message: &str) -> ! {
    let line = line + 1;
    println!("\nRuntime error on line {line}: {message}");
    exit(0)
}

fn proba_exit(state: vmstate::State, result: Result<usize, Interrupt>) -> ! {
    if unsafe { PROG_CONFIG.debug_state } {
        dbg!(&state);
    }
    if unsafe { !PROG_CONFIG.debug_state && PROG_CONFIG.debug_context } {
        dbg!(&state.contexts);
    }
    let answer = match result {
        Ok(a) | Err(Interrupt::Exit(a) | Interrupt::Return(a)) => a,
        Err(Interrupt::Error(line, message)) => proba_error(line, &message),
        _ => unreachable!(),
    };
    if unsafe { PROG_CONFIG.debug_answer } {
        println!("\nProgram returned: {answer}");
    }

    exit(0)
}

pub(crate) static mut PROG_CONFIG: Config = Config::new();

fn main() {
    const TEST_FILE_PATH: &str = "scripts/test.proba";
    parse_args(unsafe { &mut PROG_CONFIG });

    let file_path = if let Some(fp) = unsafe { PROG_CONFIG.file_path.clone() } {
        fp
    } else {
        unsafe { PROG_CONFIG.file_path = Some(TEST_FILE_PATH.into()) };
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
    match probastd::define_standard(&mut state) {
        Ok(_) => (),
        Err(int) => {
            println!("FATAL ERROR: Failed to load standard library!");
            proba_exit(state, Err(int));
        }
    }

    let result = executor::execute(&mut state, tree);
    proba_exit(state, result);
}
