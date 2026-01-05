// user/shell.rs

use core::fmt::Write;
use rustux::syscall;

fn main() {
    let mut input = String::new();
    loop {
        print!("rustux> ");
        syscall::read_line(&mut input).unwrap();
        match input.trim() {
            "exit" => break,
            _ => println!("Unknown command: {}", input),
        }
    }
}