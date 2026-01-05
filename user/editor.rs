// user/editor.rs

use core::fmt::Write;
use rustux::syscall;

fn main() {
    let mut buffer = String::new();
    loop {
        print!("> ");
        syscall::read_line(&mut buffer).unwrap();
        println!("You typed: {}", buffer);
    }
}