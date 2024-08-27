use std::{process::exit, rc::Rc};

use chs_lexer::{Lexer, Loc, Token, TokenKind};

struct Parser {
    pub lexer: Lexer,
    pub filepath: String,
    pub peeked: Option<Token>,
}

pub fn parse_file(input: Vec<u8>, filepath: String) -> Vec<Operation> {
    let lexer = Lexer::new(input);
    let mut ops: Vec<Operation> = vec![];
    let mut p = Parser {
        lexer,
        filepath,
        peeked: None,
    };

    loop {
        let token = p.next();
        if token.kind == TokenKind::EOF {
            break;
        }
        ops.push(match token.kind {
            TokenKind::KeyWord if token == *"fn" => parse_fn_expr(&mut p),

            // TokenKind::OpenCurly => continue,
            _ => parse_expr(&mut p, token),
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
    let mut ins: Vec<DataType> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *"->" => break,
            Ok(token) if token.kind == TokenKind::Word => {
                let typ = match token.value.as_str() {
                    "ptr" => DataType::Ptr,
                    "int" => DataType::Int,
                    "bool" => DataType::Bool,
                    _ => {
                        eprintln!(
                            "Error:\n  Expect Type but got `{}` in {}{}",
                            token.value, p.filepath, token.loc
                        );
                        exit(-1);
                    }
                };

                ins.push(typ);
            }
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
    let mut outs: Vec<DataType> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *"{" => break,
            Ok(token) if token.kind == TokenKind::Word => {
                let typ = match token.value.as_str() {
                    "ptr" => DataType::Ptr,
                    "int" => DataType::Int,
                    "bool" => DataType::Bool,
                    _ => {
                        eprintln!(
                            "Error:\n  Expect Type but got `{}` in {}{}",
                            token.value, p.filepath, token.loc
                        );
                        exit(-1);
                    }
                };

                outs.push(typ);
            }
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
            Ok(token) => body.push(parse_expr(p, token)),
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
        Ok(token) => Operation::Assing(token.value, type_.into()),
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
            Ok(token) => cond.push(parse_expr(p, token)),
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
            Ok(token) => body.push(parse_expr(p, token)),
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
                    Ok(token) => body.push(parse_expr(p, token)),
                    Err(e) => {
                        eprintln!("Error:\n  Expect `}}` but got EOF in {}{}", p.filepath, e);
                        exit(-1);
                    }
                }
            }
            if p.peek() == "else" {
                p.next();
                match p.expect(TokenKind::OpenCurly) {
                    Ok(_) => {
                        let mut elsebody: Vec<Operation> = vec![];
                        loop {
                            match p.require() {
                                Ok(token) if token.kind == TokenKind::CloseCurly => break,
                                Ok(token) => elsebody.push(parse_expr(p, token)),
                                Err(e) => {
                                    eprintln!(
                                        "Error:\n  Expect `}}` but got EOF in {}{}",
                                        p.filepath, e
                                    );
                                    exit(-1);
                                }
                            }
                        }
                        return Operation::IfElse(body.into(), elsebody.into());
                    }
                    Err(e) => {
                        eprintln!("Error:\n  Expect `{{` after `else` in {}{}", p.filepath, e);
                        exit(-1)
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

fn parse_bind_expr(p: &mut Parser) -> Operation {
    match p.expect(TokenKind::Interger) {
        Ok(token) => {
            let val = token
                .value
                .parse()
                .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                .expect("ParseInt");
            Operation::Bind(val)
        }
        Err(e) => {
            eprintln!("Error:\n  Expect `{{` after `else` in {}{}", p.filepath, e);
            exit(-1)
        }
    }
}

fn parse_read_expr(p: &mut Parser) -> Operation {
    match p.expect(TokenKind::Interger) {
        Ok(token) => {
            let val = token
                .value
                .parse()
                .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                .expect("ParseInt");
            Operation::Read(val)
        }
        Err(e) => {
            eprintln!("Error:\n  Expect `{{` after `else` in {}{}", p.filepath, e);
            exit(-1)
        }
    }
}

fn parse_write_expr(p: &mut Parser) -> Operation {
    match p.expect(TokenKind::Interger) {
        Ok(token) => {
            let val = token
                .value
                .parse()
                .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                .expect("ParseInt");
            Operation::Write(val)
        }
        Err(e) => {
            eprintln!("Error:\n  Expect `{{` after `else` in {}{}", p.filepath, e);
            exit(-1)
        }
    }
}

fn parse_let_expr(p: &mut Parser) -> Operation {
    let mut value: Vec<Operation> = vec![];
    loop {
        match p.require() {
            Ok(token) if token == *":" => break,
            Ok(token) => value.push(parse_expr(p, token)),
            Err(e) => {
                eprintln!("Error:\n  Expect `:` but got EOF in {}{}", p.filepath, e);
                exit(-1);
            }
        }
    }
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
        Ok(token) => Operation::Let(token.value, type_.into(), value.into()),
        Err(e) => {
            eprintln!("Error:\n  Expect a Word in {}{}", p.filepath, e);
            exit(-1);
        }
    }
}

fn parse_expr(p: &mut Parser, token: Token) -> Operation {
    //dbg!(&token);
    match token.kind {
        TokenKind::KeyWord if token == *"if" => parse_if_expr(p),
        TokenKind::KeyWord if token == *"while" => parse_while_expr(p),
        TokenKind::KeyWord if token == *":" => parse_assing_expr(p),
        TokenKind::KeyWord if token == *"let" => parse_let_expr(p),
        TokenKind::KeyWord if token == *"&" => parse_bind_expr(p),
        TokenKind::Intrinsic if token == *"@" => parse_read_expr(p),
        TokenKind::Intrinsic if token == *"!" => parse_write_expr(p),
        TokenKind::Interger => {
            let val = token
                .value
                .parse()
                .map_err(|e| eprintln!("ParseIntError: {} in {:?}", e, token))
                .expect("ParseInt");
            Operation::PushI(val)
        }
        TokenKind::KeyWord if token == *"debug" => Operation::Debug,
        TokenKind::Intrinsic => Operation::Intrinsic(token.value),
        TokenKind::Word => Operation::Word(token.value),
        TokenKind::KeyWord => {
            eprintln!(
                "Error:\n  Unexpect KeyWord `{}` in {}{}",
                token.value, p.filepath, token.loc
            );
            exit(-1)
        }
        _ => {
            unimplemented!(
                "Error:\n  Unimplemented {:?} `{}` in {}{}",
                token.kind,
                token.value,
                p.filepath,
                token.loc
            );
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

    #[allow(dead_code)]
    fn peek(&mut self) -> &Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.next());
        }

        self.peeked.as_ref().unwrap()
    }

    fn next(&mut self) -> Token {
        loop {
            let token = self
                .peeked
                .take()
                .unwrap_or_else(|| self.lexer.next_token());

            match token.kind {
                TokenKind::Whitespace | TokenKind::Comment => {}
                _ => return token,
            }
        }
    }
    fn require(&mut self) -> Result<Token, Loc> {
        let tok = self.next();
        if matches!(tok.kind, TokenKind::EOF) {
            return Err(tok.loc);
        }
        Ok(tok)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Int,
    Ptr,
    Bool,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Debug,
    Read(usize),                           // Bytes
    Write(usize),                          // Bytes
    Word(String),                          // Word
    Intrinsic(String),                     // Symbol
    PushI(i32),                            // Literal
    If(Rc<[Self]>),                        // Body
    IfElse(Rc<[Self]>, Rc<[Self]>),        // Body1 Body2
    While(Rc<[Self]>, Rc<[Self]>),         // cond Body
    Bind(u32),                             // index
    Assing(String, Rc<[String]>),          // name type
    Let(String, Rc<[String]>, Rc<[Self]>), // name type value
    Fn(
        String,
        Rc<[String]>,
        Rc<[DataType]>,
        Rc<[DataType]>,
        Rc<[Self]>,
    ), // name args ins outs body
}
