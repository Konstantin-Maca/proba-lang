use executor::Interrupt;
use std::io::Write;
use std::process::exit;
use std::{env, io};

use crate::executor::CURRENT_FILE_PATH;
use crate::rpmt::exec;

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
    pub interactive_terminal_mode: bool,
    pub debug_state: bool,
    pub debug_answer: bool,
    pub debug_context: bool,
}

impl Config {
    pub const fn new() -> Self {
        Self {
            file_path: Some(String::new()),
            args: Vec::new(),
            interactive_terminal_mode: false,
            debug_state: false,
            debug_answer: false,
            debug_context: false,
        }
    }
}

pub(crate) static mut PROG_CONFIG: Config = Config::new();

fn main() {
    parse_args();
    let mut state = vmstate::State::new();
    match probastd::define_standard(&mut state) {
        Ok(_) => (),
        Err(int) => {
            println!("FATAL ERROR: Failed to load standard library!");
            proba_exit(&mut state, Err(int));
        }
    }
    let file_path = if let Some(fp) = unsafe { PROG_CONFIG.file_path.clone() } {
        fp
    } else {
        run_pit(&mut state);
    };

    let tokens = match parser::parse_file(file_path.clone()) {
        Ok(tokens) => tokens,
        Err(_) => {
            println!("Failed to open file `{file_path}'");
            exit(0)
        }
    };
    let tree = lexer::lex(tokens);

    unsafe { executor::CURRENT_FILE_PATH = PROG_CONFIG.file_path.clone().unwrap() };
    let result = executor::execute(&mut state, tree);
    proba_exit(&mut state, result);
}

fn parse_args() {
    let config = unsafe { &mut PROG_CONFIG };
    let mut args = env::args().collect::<Vec<String>>();
    args.remove(0);

    while args.len() > 0 && args[0].starts_with("-") {
        match args[0].as_str() {
            "-interactive-terminal" | "-terminal" | "-pit" => {
                config.interactive_terminal_mode = true
            }
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

fn run_pit(state: &mut vmstate::State) -> ! {
    unsafe {
        PROG_CONFIG.file_path = Some("<pit>".into());
        PROG_CONFIG.interactive_terminal_mode = false;
    }

    // TODO: Define methods for quitting and getting answer of previous executed command.

    println!("\nCall method [: exit] or press ctrl-c to exit.\n");

    let mut command_input = String::new();
    let result = 'main: loop {
        print!("pit> ");
        io::stdout().flush().unwrap();
        command_input.clear();
        io::stdin()
            .read_line(&mut command_input)
            .expect("Failed to get user input"); // TODO: Handle error
        let result = exec(state, &command_input);
        match &result {
            Ok(answer) => {
                print!("=> ");
                let method = state.get_method(*answer, "println".into()).unwrap();
                let file_path = unsafe { CURRENT_FILE_PATH.clone() };
                let res = executor::execute_method(
                    state,
                    *answer,
                    method.2.clone(),
                    ("[:println]".into(), *answer),
                    file_path,
                );
                match res {
                    Ok(_) => (),
                    Err(int) => {
                        println!("Failed to represent the answer:");
                        if let Interrupt::Error(fp, line, message) = int {
                            print_error(&fp, line, &message);
                        }
                    }
                }
            }
            Err(Interrupt::Exit(answer)) => {
                print!("=> ");
                let method = state.get_method(*answer, "println".into()).unwrap();
                let file_path = unsafe { CURRENT_FILE_PATH.clone() };
                let res = executor::execute_method(
                    state,
                    *answer,
                    method.2.clone(),
                    ("[:println]".into(), *answer),
                    file_path,
                );
                match res {
                    Ok(_) => (),
                    Err(int) => {
                        println!("Failed to represent the answer:");
                        if let Interrupt::Error(fp, line, message) = int {
                            print_error(&fp, line, &message);
                        }
                    }
                }
                break 'main result;
            }
            Err(Interrupt::Error(fp, line, message)) => print_error(&fp, *line, message),
            _ => todo!("handle other interrupts in PIT"), // TODO
        }
    };

    proba_exit(state, result);
}

fn proba_exit(state: &mut vmstate::State, result: Result<usize, Interrupt>) -> ! {
    if unsafe { PROG_CONFIG.debug_state } {
        dbg!(&state);
    }
    if unsafe { !PROG_CONFIG.debug_state && PROG_CONFIG.debug_context } {
        dbg!(&state.contexts);
    }

    let answer = match result {
        Ok(a) | Err(Interrupt::Exit(a) | Interrupt::Return(a)) => a,
        Err(Interrupt::Error(fp, line, message)) => {
            print_error(&fp, line, &message);
            exit(0);
        }
        _ => unreachable!(),
    };
    if unsafe { PROG_CONFIG.debug_answer } {
        println!("\nProgram returned: {answer}");
    }

    if unsafe { PROG_CONFIG.interactive_terminal_mode } {
        run_pit(state);
    }

    exit(0)
}

fn print_error(file_path: &str, line: usize, message: &str) {
    let line = line + 1;
    println!("\nRuntime error on line {line} in `{file_path}':\n {message}");
}
