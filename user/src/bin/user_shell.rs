//! shell app

#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

extern crate alloc;

use user_lib::console::getchar;
use alloc::string::String;
use user_lib::{exec, fork, waitpid};

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;

pub fn main() -> i32 {
    println!("Welcome to rCore's shell!");
    println!("Type 'help' to see the help message.");
    loop {
        print!(">> ");
        let c = getchar();
        let mut line = String::new();
        match c {
            LF | CR => {
                println!("");
            }
            BS | DL => {
                print!("{}", BS as char);
                print!(" ");
                print!("{}", BS as char);
                line.pop();
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
        if !line.is_empty() {
            line.push('\n');
            let pid = fork();
            if pid == 0 {
                // child process
                if exec(line.as_str()) == -1 {
                    println!("Error when executing!");
                    return -4;
                }
                unreachable!();
            } else {
                let mut exit_code: i32 = 0;
                let exit_pid = waitpid(pid as usize, &mut exit_code);
                assert_eq!(pid, exit_pid);
                println!(
                    "Shell: Process {} exited with code {}",
                    pid, exit_code
                );
            }
            line.clear();
        }
    }
}