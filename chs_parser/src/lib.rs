use std::{process::exit, rc::Rc};

use chs_lexer::{Lexer, Loc, Token, TokenKind};

struct Parser {
    pub lexer: Lexer,
    pub filepath: String,
}

pub fn parse_file(input: Vec<u8>, filepath: String) -> Vec<Operation> {
    let lexer = Lexer::new(input);
    let mut ops: Vec<Operation> = vec![];
    let mut p = Parser { lexer, filepath };

    loop {
        let token = p.next();
        if token.kind == TokenKind::EOF {
            break;
        }
        ops.push(match token.kind {
            TokenKind::KeyWord if token == *"if" => parse_if_expr(&mut p),
            TokenKind::KeyWord if token == *"else" => parse_else_expr(&mut p),
            TokenKind::KeyWord if token == *"while" => parse_while_expr(&mut p),
            TokenKind::KeyWord if token == *"fn" => parse_fn_expr(&mut p),
            TokenKind::KeyWord if token == *":" => parse_assing_expr(&mut p),
            _ => parse_expr(token),
        })
    }
    return ops;
}

fn parse_fn_expr(p: &mut Parser) -> Operation {
    let name = match p.expect(TokenKind::Word) {
        Ok(token) => token.value,
        Err(e) => {
            eprintln!("Error:\n  Expect function name in {}{}", p.filepath, e);
            exit(-1);
        }
    };
    let mut args: Vec<String> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *":" => break,
            Ok(token) if token.kind == TokenKind::Word => args.push(token.value),
            Ok(token) => {
                eprintln!(
                    "Error:\n  Expect argument name but got `{}` in {}{}",
                    token.value, p.filepath, token.loc
                );
                exit(-1);
            }
            Err(e) => {
                eprintln!("Error:\n  Expect `:` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    let mut ins: Vec<String> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *"->" => break,
            Ok(token) if token.kind == TokenKind::Word => ins.push(token.value),
            Ok(token) => {
                eprintln!(
                    "Error:\n  Expect Type but got `{}` in {}{}",
                    token.value, p.filepath, token.loc
                );
                exit(-1);
            }
            Err(e) => {
                eprintln!("Error:\n  Expect `->` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    let mut outs: Vec<String> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *"{" => break,
            Ok(token) if token.kind == TokenKind::Word => outs.push(token.value),
            Ok(token) => {
                eprintln!(
                    "Error:\n  Expect Type but got `{}` in {}{}",
                    token.value, p.filepath, token.loc
                );
                exit(-1);
            }
            Err(e) => {
                eprintln!("Error:\n  Expect `{{` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    let mut body: Vec<Operation> = vec![];
    loop {
        match p.require() {
            Ok(token) if token.kind == TokenKind::CloseCurly => break,
            Ok(token) => body.push(parse_expr(token)),
            Err(e) => {
                eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    Operation::Fn(name, args.into(), ins.into(), outs.into(), body.into())
}

fn parse_assing_expr(p: &mut Parser) -> Operation {
    let mut type_: Vec<String> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *"=" => break,
            Ok(token) if token.kind == TokenKind::Word => type_.push(token.value),
            Ok(token) => {
                eprintln!(
                    "Error:\n  Expect Type but got `{}` in {}{}",
                    token.value, p.filepath, token.loc
                );
                exit(-1);
            }
            Err(e) => {
                eprintln!("Error:\n  Expect `=` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    match p.expect(TokenKind::Word) {
        Ok(token) => Operation::Bind(token.value, type_.into()),
        Err(e) => {
            eprintln!("Error:\n  Expect a Word in {}{}", p.filepath, e);
            exit(-1);
        }
    }
}

fn parse_while_expr(p: &mut Parser) -> Operation {
    let mut cond: Vec<Operation> = vec![];
    loop {
        match p.require() {
            Ok(token) if token.kind == TokenKind::OpenCurly => break,
            Ok(token) => cond.push(parse_expr(token)),
            Err(e) => {
                eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    let mut body: Vec<Operation> = vec![];
    loop {
        match p.require() {
            Ok(token) if token.kind == TokenKind::CloseCurly => break,
            Ok(token) => body.push(parse_expr(token)),
            Err(e) => {
                eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
    Operation::While(cond.into(), body.into())
}

fn parse_if_expr(p: &mut Parser) -> Operation {
    match p.expect(TokenKind::OpenCurly) {
        Ok(_) => {
            let mut body: Vec<Operation> = vec![];
            loop {
                match p.require() {
                    Ok(token) if token.kind == TokenKind::CloseCurly => break,
                    Ok(token) => body.push(parse_expr(token)),
                    Err(e) => {
                        eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                        exit(-1);
                    }
                }
            }
            Operation::If(body.into())
        }
        Err(e) => {
            eprintln!("Error:\n  Expect `{{` after `if` in {}{}", p.filepath, e);
            exit(-1)
        }
    }
}

fn parse_else_expr(p: &mut Parser) -> Operation {
    match p.expect(TokenKind::OpenCurly) {
        Ok(_) => {
            let mut body: Vec<Operation> = vec![];
            loop {
                match p.require() {
                    Ok(token) if token.kind == TokenKind::CloseCurly => break,
                    Ok(token) => body.push(parse_expr(token)),
                    Err(e) => {
                        eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                        exit(-1);
                    }
                }
            }
            Operation::Else(body.into())
        }
        Err(e) => {
            eprintln!("Error:\n  Expect `{{` after `else` in {}{}", p.filepath, e);
            exit(-1)
        }
    }
}

fn parse_expr(token: Token) -> Operation {
    match token.kind {
        TokenKind::Interger => {
            let val = token
                .value
                .parse()
                .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                .expect("ParseInt");
            Operation::PushI(val)
        }
        TokenKind::KeyWord if token == *"drop" => Operation::Drop,
        TokenKind::KeyWord if token == *"debug" => Operation::Debug,
        TokenKind::Intrinsic => Operation::Intrinsic(token.value),
        TokenKind::Word => Operation::Word(token.value),
        _ => {
            dbg!(token);
            exit(-1)
        }
    }
}

impl Parser {
    fn expect(&mut self, kind: TokenKind) -> Result<Token, Loc> {
        let token = self.next();

        if token.kind == kind {
            return Ok(token);
        }
        Err(token.loc)
    }

    fn next(&mut self) -> Token {
        loop {
            let token = self.lexer.next_token();

            match token.kind {
                TokenKind::Whitespace | TokenKind::Comment => {}
                _ => return token,
            }
        }
    }
    fn require(&mut self) -> Result<Token, Loc> {
        let tok = self.next();
        if matches!(tok.kind, TokenKind::Invalid | TokenKind::EOF) {
            return Err(tok.loc);
        }
        Ok(tok)
    }
}

#[derive(Debug)]
pub enum Operation {
    Drop,
    Debug,
    Word(String),                                                     // Word
    Intrinsic(String),                                                // Symbol
    PushI(i32),                                                       // Literal
    If(Rc<[Self]>),                                                   // Body
    Else(Rc<[Self]>),                                                 // Body
    While(Rc<[Self]>, Rc<[Self]>),                                    // cond Body
    Bind(String, Rc<[String]>),                                       // name type
    Fn(String, Rc<[String]>, Rc<[String]>, Rc<[String]>, Rc<[Self]>), // name args ins outs body
}
