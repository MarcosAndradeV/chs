use core::fmt;

pub struct Lexer {
    data: Vec<u8>,
    pos: usize,
    max: usize,
    loc: Loc,
}

const KEYWORDS: &[&'static str] = &[
    "debug", "if", "else", "while", "fn", "let", ":", "=", "->", "&",
];
const INTRISIC: &[u8] = &[b'+', b'-', b'*', b'=', b':', b'>', b'<', b'!', b'@'];

impl Lexer {
    pub fn new(data: Vec<u8>) -> Self {
        let max = data.len();
        Self {
            data,
            pos: 0,
            max,
            loc: Loc::new(1, 1),
        }
    }
    fn curr_char(&self) -> u8 {
        if self.pos < self.max {
            self.data[self.pos]
        } else {
            0
        }
    }
    fn advance_pos(&mut self) {
        if self.pos < self.max {
            self.pos += 1;
            self.loc = self.loc.next(self.curr_char());
        }
    }
    fn peek_char(&self, offset: usize) -> u8 {
        if self.pos + offset < self.max {
            self.data[self.pos + offset]
        } else {
            0
        }
    }
    fn make_token(&self, start: usize, kind: TokenKind, start_loc: Loc) -> Token {
        let value = String::from_utf8_lossy(&self.data[start..self.pos]).to_string();
        Token::new(value, kind, start_loc)
    }
    fn make_token_advance(&mut self, start: usize, kind: TokenKind) -> Token {
        let start_loc = self.loc;
        self.advance_pos();
        self.make_token(start, kind, start_loc)
    }
    pub fn next_token(&mut self) -> Token {
        let start = self.pos;
        let start_loc = self.loc;
        match self.curr_char() {
            c if c.is_ascii_whitespace() => self.whitespace(start),
            b'a'..=b'z' | b'A'..=b'Z' => self.identfier(start),
            b'0'..=b'9' => self.number(start),
            b'{' => self.make_token_advance(start, TokenKind::OpenCurly),
            b'}' => self.make_token_advance(start, TokenKind::CloseCurly),
            b':' => self.make_token_advance(start, TokenKind::KeyWord),
            b'&' => self.make_token_advance(start, TokenKind::KeyWord),
            b'-' => {
                if self.peek_char(1) == b'-' {
                    self.pos += 2;
                    return self.comment(start);
                }
                if self.peek_char(1) == b'>' {
                    self.pos += 2;
                    return self.make_token(start, TokenKind::KeyWord, start_loc);
                }
                self.make_token_advance(start, TokenKind::Intrinsic)
            }
            b'!' => {
                if self.peek_char(1) == b'=' {
                    self.pos += 2;
                    return self.make_token(start, TokenKind::Intrinsic, start_loc);
                }
                self.make_token_advance(start, TokenKind::Intrinsic)
            }
            b'=' => {
                if self.peek_char(1) == b'=' {
                    self.pos += 2;
                    return self.make_token(start, TokenKind::Intrinsic, start_loc);
                }
                self.make_token_advance(start, TokenKind::KeyWord)
            }
            c if INTRISIC.contains(&c) => self.make_token_advance(start, TokenKind::Intrinsic),
            _ => {
                if self.pos < self.max {
                    self.make_token_advance(start, TokenKind::Invalid)
                } else {
                    Token::new(String::from("\0"), TokenKind::EOF, start_loc)
                }
            }
        }
    }
    fn identfier(&mut self, start: usize) -> Token {
        let start_loc = self.loc;
        loop {
            self.advance_pos();
            let curr_char = &self.curr_char();
            if curr_char.is_ascii_whitespace() {
                break;
            }
        }
        let value = String::from_utf8_lossy(&self.data[start..self.pos]).to_string();
        match value.as_str() {
            c if KEYWORDS.contains(&c) => Token::new(value, TokenKind::KeyWord, start_loc),
            "dup" | "drop" | "swap" | "over" | "rot" | "mod" => {
                Token::new(value, TokenKind::Intrinsic, start_loc)
            }
            _ => Token::new(value, TokenKind::Word, start_loc),
        }
    }
    fn number(&mut self, start: usize) -> Token {
        let start_loc = self.loc;
        loop {
            self.advance_pos();
            if !matches!(self.curr_char(), b'0'..=b'9') {
                break;
            }
        }
        self.make_token(start, TokenKind::Interger, start_loc)
    }
    fn whitespace(&mut self, start: usize) -> Token {
        let start_loc = self.loc;
        loop {
            self.advance_pos();
            if !self.curr_char().is_ascii_whitespace() {
                break;
            }
        }

        self.make_token(start, TokenKind::Whitespace, start_loc)
    }
    fn comment(&mut self, start: usize) -> Token {
        let start_loc = self.loc;
        loop {
            self.advance_pos();
            if matches!(self.curr_char(), b'\n' | b'\0') {
                break;
            }
        }
        self.make_token(start, TokenKind::Comment, start_loc)
    }
}

#[derive(Debug)]
pub struct Token {
    pub loc: Loc,
    pub value: String,
    pub kind: TokenKind,
}

impl PartialEq<str> for Token {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
    fn ne(&self, other: &str) -> bool {
        self.value != other
    }
}

impl Token {
    pub fn new(value: String, kind: TokenKind, loc: Loc) -> Self {
        Self { loc, value, kind }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TokenKind {
    EOF,
    Invalid,
    Whitespace,
    Comment,
    Interger,
    KeyWord,
    Word,
    Intrinsic,
    OpenCurly,
    CloseCurly,
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Default)]
pub struct Loc {
    // file: &'a str,
    line: usize,
    col: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ":{}:{}", self.line, self.col)
    }
}

impl Loc {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
    pub fn next_column(&self) -> Self {
        Self {
            line: self.line,
            col: self.col + 1,
        }
    }
    pub fn next_line(&self) -> Self {
        Self {
            line: self.line + 1,
            col: 1,
        }
    }
    pub fn next(&self, c: u8) -> Self {
        match c {
            b'\n' => self.next_line(),
            b'\t' => {
                let ts = 8;
                Self {
                    line: self.line,
                    col: (self.col / ts) * ts + ts,
                }
            }
            c if (c as char).is_control() => *self,
            _ => self.next_column(),
        }
    }
}
