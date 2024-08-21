use chs_lexer::{Lexer, TokenKind};

pub fn parse_file(input: Vec<u8>, _filepath: String) -> Vec<Operation> {
    let mut lexer = Lexer::new(input);
    let mut ops: Vec<Operation> = vec![];

    loop {
        let token = lexer.next_token();
        if token.kind == TokenKind::EOF {
            break;
        }
        if token.kind == TokenKind::Whitespace {
            continue;
        }
        match token.kind {
            TokenKind::Interger => {
                let val = token
                    .value
                    .parse()
                    .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                    .expect("ParseInt");
                ops.push(Operation::PushI(val));
            }
            TokenKind::KeyWord if token == *"drop" => ops.push(Operation::Drop),
            TokenKind::Intrinsic if token == *"+" => ops.push(Operation::Plus),
            _ => {
                dbg!(token);
            }
        }
    }
    return ops;
}

#[derive(Debug)]
pub enum Operation {
    PushI(i32), // Literal
    Drop,
    Plus,
}
