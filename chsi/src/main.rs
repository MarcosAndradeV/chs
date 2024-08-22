use std::{env, fs::File, io::Read, process::exit};

use chs_parser::parse_file;

fn main() {
    let mut args = env::args();
    let _program = args.next().expect("Program always provided.");
    if let Some(filepath) = args.next() {
        if let Ok(mut file) = File::open(filepath.clone()) {
            let mut buf = Vec::new();
            let _ = file.read_to_end(&mut buf);
            let program = parse_file(buf, filepath);
            dbg!(program);
        } else {
            exit(-1)
        }
    }
}
