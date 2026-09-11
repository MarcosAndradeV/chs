use std::path::Path;

use ast::*;
use diagnostic::*;
use lex_just_parse::lexer::*;
use lex_just_parse::parser::*;
use lex_just_parse::try_parse;

pub mod ast;
#[cfg(test)]
mod tests;

pub fn parse_file(
    file_path: &Path,
    source: &str,
    reporter: &mut DiagnosticReporter,
) -> ChsResult<FileAst> {
    let mut lex = Lexer::new(file_path.to_string_lossy(), source).with_keywords(&[
        "fn", "var", "const", "if", "else", "for", "return", "in", "true", "false", "break",
        "continue", "foreach", "null", "defer", "switch", "cast", "autocast", "new", "make",
        "drop", // "import", "struct", "enum",
                // "type", "module",
    ]);

    match parse_module_declaration(&mut lex, reporter).and_then(|mut lex, module| {
        let items = try_parse!(lex, parse_decls(lex, reporter));
        Parser::Success(lex, (module, items))
    }) {
        Parser::Fail(lex, _) => {
            bail!(
                "Fail to parse {}: Last token: {}",
                file_path.display(),
                lex.peek()
            )
        }
        Parser::Success(_, (module, items)) => Ok(FileAst { module, items }),
    }
}

fn parse_decls<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<FileItem>, ()> {
    many(lex, |lex| parse_decl(lex, reporter)).and_then(|lex, items| {
        if !lex.peek().is_eof() {
            Parser::Fail(lex, ())
        } else {
            Parser::Success(lex, items)
        }
    })
}

fn parse_module_declaration<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Token, ()> {
    let token = lex.next();
    if token.kind == TokenKind::Identifier && token.source() == "module" {
        parse_identifier(lex)
    } else {
        reporter.report(
            token.loc,
            format!("expected module declaration found: {}", token.source()),
        );
        Parser::Fail(lex, ())
    }
}

fn parse_decl<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FileItem, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::Keyword if token.source() == "fn" => {
            lex.next();
            let mut self_param = None;
            if lex.peek().kind == TokenKind::OpenParen {
                lex.next();
                self_param = Some(try_parse!(lex, parse_fn_pointer_parameter(lex, reporter)));
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
            }
            let f = try_parse!(lex, parse_function(lex, reporter));
            if let Some(self_parameter) = self_param {
                let loc = f.signature.name.loc;
                Parser::Success(
                    lex,
                    FileItem::MethodImplementation(MethodImplementation {
                        loc,
                        self_parameter,
                        function: f,
                    }),
                )
            } else {
                Parser::Success(lex, FileItem::Function(f))
            }
        }
        TokenKind::Identifier if token.source() == "type" => {
            lex.next();
            let name = try_parse!(lex, parse_identifier(lex));
            match lex.peek().source() {
                "struct" => {
                    lex.next();
                    let s = try_parse!(lex, parse_struct(lex, reporter, name));
                    Parser::Success(lex, FileItem::Struct(s))
                }
                "enum" => {
                    lex.next();
                    let e = try_parse!(lex, parse_enum(lex, reporter, name));
                    Parser::Success(lex, FileItem::Enum(e))
                }
                _ => {
                    let is_distinct = if lex.peek().kind == TokenKind::Directive
                        && lex.peek().source() == "#distinct"
                    {
                        lex.next(); // Consume "#distinct"
                        true
                    } else {
                        false
                    };
                    let base_type = try_parse!(lex, parse_type(lex, reporter));
                    Parser::Success(
                        lex,
                        FileItem::TypeDecl(TypeDecl {
                            name,
                            is_distinct,
                            base_type,
                        }),
                    )
                }
            }
        }
        TokenKind::Identifier if token.source() == "import" => {
            lex.next();
            let mut mode = ImportMode::Default;
            match lex.peek().kind {
                TokenKind::Dot => {
                    lex.next();
                    mode = ImportMode::All;
                }
                TokenKind::Identifier => {
                    let alias = lex.next().source;
                    mode = ImportMode::Alias(alias);
                }
                _ => (),
            }
            let first = lex.next();
            if first.kind != TokenKind::StringLiteral {
                reporter.report(
                    first.loc,
                    format!("Expected string literal, found {}", first.source()),
                );
                return Parser::Fail(lex, ());
            }
            let path_str = first.source().trim_matches('"').trim().to_string();
            if path_str.is_empty()
                || path_str
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != '/')
            {
                reporter.report(
                    first.loc,
                    format!("Invalid module name: {}", path_str.as_str()),
                );
                return Parser::Fail(lex, ());
            }
            let path_token = Token::new(
                TokenKind::Identifier,
                first.loc,
                TokenSource::from(path_str),
            );
            Parser::Success(
                lex,
                FileItem::Import(Import {
                    mode,
                    path: path_token,
                }),
            )
        }
        TokenKind::Keyword if token.source() == "var" => {
            lex.next(); // Consume "var"
            let stmt = try_parse!(lex, parse_var_decl_inner(lex, reporter));
            Parser::Success(lex, FileItem::VarDecl(stmt))
        }
        TokenKind::Keyword if token.source() == "const" => {
            let stmt = try_parse!(lex, parse_const_declaration(lex, reporter));
            Parser::Success(lex, FileItem::Const(stmt))
        }
        TokenKind::Identifier if token.source() == "operator" => {
            lex.next(); // Consume "operator"
            let op = lex.next();
            let loc = op.loc;
            let op = match op.kind {
                TokenKind::Plus => Op::Add,
                TokenKind::Minus => Op::Sub,
                TokenKind::Asterisk => Op::Mul,
                TokenKind::Slash => Op::Div,
                TokenKind::Mod => Op::Mod,
                TokenKind::Lt => Op::Lt,
                TokenKind::LtEq => Op::LtEq,
                TokenKind::Gt => Op::Gt,
                TokenKind::GtEq => Op::GtEq,
                TokenKind::EqEq => Op::Eq,
                TokenKind::NotEq => Op::NotEq,
                _ => {
                    reporter.report(lex.peek().loc, "expected operator not supported");
                    return Parser::Fail(lex, ());
                }
            };

            let (parameters, va_args) = try_parse!(lex, parse_parameters(lex, reporter));

            if va_args {
                reporter.report(
                    lex.peek().loc,
                    "operator overload cannot have variadic arguments",
                );
                return Parser::Fail(lex, ());
            }

            if op.is_binary() && parameters.len() != 2 {
                reporter.report(
                    lex.peek().loc,
                    "operator overload of binary must have 2 arguments",
                );
                return Parser::Fail(lex, ());
            }

            if op.is_unary() && parameters.len() != 1 {
                reporter.report(
                    lex.peek().loc,
                    "operator overload of unary must have 1 argument",
                );
                return Parser::Fail(lex, ());
            }

            let mut return_type = None;
            if lex.peek().kind == TokenKind::Arrow {
                lex.next();
                let typ = try_parse!(lex, parse_type(lex, reporter));
                return_type = Some(typ);
            }

            if return_type.is_none() {
                reporter.report(lex.peek().loc, "operator overload must have a return type");
                return Parser::Fail(lex, ());
            }

            let mut body = None;
            if lex.peek().kind == TokenKind::OpenCurly {
                let (l, b) = try_parse!(parse_block_stmt(lex, reporter));
                lex = l;
                body = Some(b);
            }
            if body.is_none() {
                reporter.report(lex.peek().loc, "operator overload must have a body");
                return Parser::Fail(lex, ());
            }

            Parser::Success(
                lex,
                FileItem::Operator(OperatorOverload {
                    loc,
                    op,
                    parameters,
                    return_type: return_type.unwrap(),
                    body: body.unwrap(),
                    resolved_name: None,
                }),
            )
        }
        TokenKind::Identifier if token.source() == "library" => {
            lex.next();
            let name = try_parse!(lex, parse_identifier(lex));
            try_parse!(lex, expect(lex, TokenKind::OpenCurly, reporter));

            let mut link_name = None;
            let mut kind = None;

            while lex.peek().kind != TokenKind::CloseCurly && lex.peek().kind != TokenKind::EOF {
                let key_tok = try_parse!(lex, parse_identifier(lex));
                try_parse!(lex, expect(lex, TokenKind::Eq, reporter));
                let val_tok = try_parse!(lex, parse_string_literal(lex));
                let val_str = val_tok.source().trim_matches('"');

                match key_tok.source() {
                    "link_name" => {
                        link_name = Some(TokenSource::from(val_str));
                    }
                    "kind" => {
                        let k = match val_str {
                            "static" => LibraryKind::Static,
                            "dynlib" => LibraryKind::Dynlib,
                            s => {
                                reporter.report(
                                    val_tok.loc,
                                    format!("Unexpected library kind `{}`", s),
                                );
                                return Parser::Fail(lex, ());
                            }
                        };
                        kind = Some(k);
                    }
                    other => {
                        reporter.report(
                            key_tok.loc,
                            format!("unexpected library attribute `{}`", other),
                        );
                        return Parser::Fail(lex, ());
                    }
                }

                if lex.peek().kind == TokenKind::Comma {
                    lex.next();
                }
            }

            try_parse!(lex, expect(lex, TokenKind::CloseCurly, reporter));

            let link_name = if let Some(ln) = link_name {
                ln
            } else {
                reporter.report(name.loc, "library missing 'link_name' attribute");
                return Parser::Fail(lex, ());
            };

            let kind = kind.unwrap_or(LibraryKind::Dynlib);

            Parser::Success(
                lex,
                FileItem::Library(Library {
                    name,
                    link_name,
                    kind,
                }),
            )
        }
        TokenKind::Directive => {
            let token = lex.next();
            reporter.report(
                token.loc,
                format!("Unknown top-level directive: {}", token.source()),
            );
            Parser::Fail(lex, ())
        }
        TokenKind::EOF => Parser::Fail(lex, ()),
        _ => {
            let p = lex.peek();
            reporter.report(p.loc, format!("Unexpected token `{}`", p.source()));
            Parser::Fail(lex, ())
        }
    }
}

fn parse_enum<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
    name: Token,
) -> Parser<'lex, Enum, ()> {
    let inner_type = if lex.peek().kind == TokenKind::Colon {
        lex.next();
        Some(try_parse!(lex, parse_type(lex, reporter)))
    } else {
        None
    };

    let mut directives = Vec::new();
    while lex.peek().kind == TokenKind::Directive {
        let value = lex.next();
        match value.source() {
            "#test" => directives.push(EnumDirective::Test(value.loc)),
            _ => {
                reporter.report(
                    value.loc,
                    format!("unknown enum directive: {}", value.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    let variants = try_parse!(lex, parse_enum_variants(lex, reporter));

    Parser::Success(
        lex,
        Enum {
            name,
            directives,
            inner_type,
            variants,
        },
    )
}

fn parse_enum_variants<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<EnumVariant>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let variants = try_parse!(
        lex,
        sep_by(lex, |l| parse_enum_variant(l, reporter), parse_maybe_comma)
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, variants)
}

fn parse_struct<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
    name: Token,
) -> Parser<'lex, Struct, ()> {
    let mut directives = Vec::new();
    while lex.peek().kind == TokenKind::Directive {
        let value = lex.next();
        match value.source() {
            "#test" => directives.push(StructDirective::Test(value.loc)),
            _ => {
                reporter.report(
                    value.loc,
                    format!("unknown struct directive: {}", value.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    let fields = try_parse!(lex, parse_fields(lex, reporter));

    Parser::Success(
        lex,
        Struct {
            name,
            directives,
            fields,
        },
    )
}

fn parse_fields<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Field>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let fields = try_parse!(
        lex,
        sep_by(lex, |l| parse_field(l, reporter), parse_maybe_comma)
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, fields)
}

fn parse_function<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Function, ()> {
    let name = try_parse!(lex, parse_identifier_or_keyword(lex));

    let (parameters, va_args) = try_parse!(lex, parse_parameters(lex, reporter));

    let mut return_type = None;
    if lex.peek().kind == TokenKind::Arrow {
        lex.next();
        let typ = try_parse!(lex, parse_type(lex, reporter));
        return_type = Some(typ);
    }

    let mut directives = Vec::new();
    let mut is_foreign = false;
    while lex.peek().kind == TokenKind::Directive {
        let directive = lex.next();
        match directive.source() {
            "#foreign" => {
                let name = try_parse!(lex, parse_identifier(lex));
                directives.push(FunctionDirective::Foreign(directive.loc, name));
                is_foreign = true;
            }
            "#link_name" => {
                let name = try_parse!(lex, parse_string_literal(lex));
                directives.push(FunctionDirective::LinkName(name));
            }
            "#private" => {
                directives.push(FunctionDirective::Private);
            }
            "#owned_return" => {
                directives.push(FunctionDirective::OwnedReturn);
            }
            "#export" => {
                directives.push(FunctionDirective::Export);
            }
            _ => {
                reporter.report(
                    directive.loc,
                    format!("unknown function directive: {}", directive.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    if va_args && !is_foreign {
        reporter.report(
            name.loc,
            "non foreign functions cannot have variadic arguments",
        );
        return Parser::Fail(lex, ());
    }

    for (i, p) in parameters.iter().enumerate() {
        if p.is_variadic && i < parameters.len() - 1 {
            reporter.report(p.name.loc, "variadic parameter must be the last parameter");
            return Parser::Fail(lex, ());
        }
    }

    let signature = FunctionSignature {
        name,
        parameters,
        return_type,
        va_args,
    };

    let mut body = None;
    if lex.peek().kind == TokenKind::OpenCurly {
        let (l, b) = try_parse!(parse_block_stmt(lex, reporter));
        lex = l;
        body = Some(b);
    }

    Parser::Success(
        lex,
        Function {
            signature,
            directives,
            body,
            resolved_name: None,
        },
    )
}

fn parse_block_stmt<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, BlockStmt, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let mut block = BlockStmt::new(token.loc);

    block.stmts = try_parse!(
        lex,
        sep_by(
            lex,
            |lex| {
                if lex.peek().kind == TokenKind::CloseCurly {
                    return Parser::Fail(lex, ());
                }
                parse_stmt(lex, reporter)
            },
            parse_maybe_semicolon
        )
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, block)
}

fn parse_stmt<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Stmt, ()> {
    let ptoken = lex.peek().clone();
    match ptoken.kind {
        TokenKind::Identifier => {
            let ident = try_parse!(lex, parse_identifier(lex));
            let peek = lex.peek();
            match peek.kind {
                TokenKind::OpenParen => {
                    let loc = peek.loc;
                    let arguments = try_parse!(lex, parse_arguments(lex, reporter));
                    Parser::Success(
                        lex,
                        Stmt::Call(CallExpr {
                            loc,
                            callee: Box::new(Expr::new(ExprKind::Identifier(ident))),
                            arguments,
                            resolved_name: None,
                            module_name: None,
                        }),
                    )
                }
                _ => {
                    // e.g. `arr = 5` or `arr.foo`
                    let mut left = Expr::new(ExprKind::Identifier(ident.clone()));
                    loop {
                        let p = token_to_precedence(lex.peek());
                        if Precedence::Lowest >= p {
                            break;
                        }
                        let next_left = try_parse!(lex, parse_infix(lex, left, reporter));
                        left = next_left;
                    }

                    if lex.peek().kind == TokenKind::Comma {
                        let mut exprs = vec![left];
                        let mut last_loc = ident.loc;
                        while lex.peek().kind == TokenKind::Comma {
                            lex.next(); // consume comma
                            let expr = try_parse!(
                                lex,
                                parse_expr_with_precedence(lex, Precedence::Assignment, reporter)
                            );
                            last_loc = expr.loc();
                            exprs.push(expr);
                        }
                        let left_tuple = Expr::new(ExprKind::Tuple(exprs, last_loc));

                        let op_token = lex.peek().clone();
                        if let Some(assign_kind) = token_to_assign_kind(op_token.kind) {
                            lex.next(); // consume assign operator
                            let (lex, first_right) = try_parse!(parse_expr_with_precedence(
                                lex,
                                Precedence::Lowest,
                                reporter
                            ));
                            let mut current_lex = lex;

                            let right = if current_lex.peek().kind == TokenKind::Comma {
                                let mut rexprs = vec![first_right];
                                let mut rlast_loc = op_token.loc;
                                while current_lex.peek().kind == TokenKind::Comma {
                                    current_lex.next();
                                    let (lex, expr) = try_parse!(parse_expr_with_precedence(
                                        current_lex,
                                        Precedence::Lowest,
                                        reporter
                                    ));
                                    current_lex = lex;
                                    rlast_loc = expr.loc();
                                    rexprs.push(expr);
                                }
                                Expr::new(ExprKind::Tuple(rexprs, rlast_loc))
                            } else {
                                first_right
                            };

                            let assign_expr = Expr::new(ExprKind::Assign(AssignExpr {
                                left: Box::new(left_tuple),
                                assign_kind,
                                right: Box::new(right),
                            }));
                            Parser::Success(current_lex, Stmt::ExprStmt(assign_expr))
                        } else {
                            reporter
                                .report(op_token.loc, "expected assignment operator after tuple");
                            Parser::Fail(lex, ())
                        }
                    } else {
                        Parser::Success(lex, Stmt::ExprStmt(left))
                    }
                }
            }
        }
        TokenKind::Keyword if ptoken.source() == "var" => {
            lex.next();
            let (lex, stmt) = try_parse!(parse_var_decl_inner(lex, reporter));
            Parser::Success(lex, Stmt::VarDecl(stmt))
        }
        TokenKind::Keyword if ptoken.source() == "const" => {
            let stmt = try_parse!(lex, parse_const_declaration(lex, reporter));
            Parser::Success(lex, Stmt::Const(stmt))
        }
        TokenKind::OpenCurly => {
            let body = try_parse!(lex, parse_block_stmt(lex, reporter));
            Parser::Success(lex, Stmt::Block(body))
        }
        TokenKind::Keyword if ptoken.source() == "if" => {
            lex.next();
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            let true_body = try_parse!(lex, parse_block_stmt(lex, reporter));
            let peek = lex.peek();
            let stmt = if peek.kind == TokenKind::Keyword && peek.source() == "else" {
                lex.next();
                let peek = lex.peek();
                // `else if` hack
                let false_body = if peek.kind == TokenKind::Keyword && peek.source() == "if" {
                    let next_if = try_parse!(lex, parse_stmt(lex, reporter));
                    BlockStmt {
                        loc: next_if.loc(),
                        stmts: vec![next_if],
                    }
                } else {
                    try_parse!(lex, parse_block_stmt(lex, reporter))
                };

                Stmt::IfStmt(IfStmt::IfElse {
                    cond,
                    true_body,
                    false_body,
                })
            } else {
                Stmt::IfStmt(IfStmt::If { cond, true_body })
            };
            Parser::Success(lex, stmt)
        }
        TokenKind::Keyword if ptoken.source() == "foreach" => {
            lex.next();
            if lex.peek().kind == TokenKind::Identifier {
                let var_name = lex.next();
                let peek = lex.peek();
                if peek.kind == TokenKind::Keyword && peek.source() == "in" {
                    lex.next();
                    let iter_expr = try_parse!(lex, parse_expr(lex, reporter));
                    let body = try_parse!(lex, parse_block_stmt(lex, reporter));
                    return Parser::Success(
                        lex,
                        Stmt::ForEach(ForEachStmt {
                            var_name,
                            iter_expr,
                            body,
                        }),
                    );
                }
            }
            reporter.report(
                ptoken.loc,
                "Expect Identifier: foreach statements must use this format `foreach x in xs`",
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Keyword if ptoken.source() == "for" => {
            lex.next();
            if lex.peek().kind == TokenKind::OpenCurly {
                let body = try_parse!(lex, parse_block_stmt(lex, reporter));
                return Parser::Success(lex, Stmt::ForStmt(ForStmt::ForLoop(body)));
            }
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            let body = try_parse!(lex, parse_block_stmt(lex, reporter));
            Parser::Success(lex, Stmt::ForStmt(ForStmt::ForCond { cond, body }))
        }
        TokenKind::Keyword if ptoken.source() == "break" => {
            let token = lex.next();
            Parser::Success(lex, Stmt::Break(token.loc))
        }
        TokenKind::Keyword if ptoken.source() == "continue" => {
            let token = lex.next();
            Parser::Success(lex, Stmt::Continue(token.loc))
        }
        TokenKind::Keyword if ptoken.source() == "return" => {
            let loc = lex.next().loc;
            if lex.peek().kind == TokenKind::SemiColon {
                return Parser::Success(lex, Stmt::Return(loc, None));
            }
            let first_expr = try_parse!(lex, parse_expr(lex, reporter));
            if lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_expr];
                while lex.peek().kind == TokenKind::Comma {
                    lex.next();
                    let expr = try_parse!(lex, parse_expr(lex, reporter));
                    exprs.push(expr);
                }
                let tuple_expr = Expr::new(ExprKind::Tuple(exprs, loc));
                Parser::Success(lex, Stmt::Return(loc, Some(tuple_expr)))
            } else {
                Parser::Success(lex, Stmt::Return(loc, Some(first_expr)))
            }
        }
        TokenKind::Keyword if ptoken.source() == "defer" => {
            let loc = lex.next().loc;
            let inner_stmt = try_parse!(lex, parse_stmt(lex, reporter));
            Parser::Success(lex, Stmt::Defer(loc, Box::new(inner_stmt)))
        }
        TokenKind::Keyword if ptoken.source() == "switch" => {
            let loc = lex.next().loc;
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::OpenCurly, reporter));

            let mut branches = Vec::new();
            let mut default = None;

            while lex.peek().kind != TokenKind::CloseCurly && lex.peek().kind != TokenKind::EOF {
                let pattern = try_parse!(lex, parse_expr(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::Arrow, reporter));
                let body = try_parse!(lex, parse_stmt(lex, reporter));

                if lex.peek().kind == TokenKind::Comma || lex.peek().kind == TokenKind::SemiColon {
                    lex.next();
                }

                let is_wildcard = if let ExprKind::Identifier(ref ident) = pattern.kind {
                    ident.source() == "_"
                } else {
                    false
                };

                if is_wildcard {
                    if default.is_some() {
                        reporter.report(pattern.loc(), "Multiple default cases in switch");
                    }
                    default = Some(Box::new(body));
                } else {
                    branches.push(SwitchBranch { pattern, body });
                }
            }

            try_parse!(lex, expect(lex, TokenKind::CloseCurly, reporter));

            Parser::Success(
                lex,
                Stmt::Switch(SwitchStmt {
                    loc,
                    cond,
                    branches,
                    default,
                }),
            )
        }
        _ => {
            let first_expr = try_parse!(lex, parse_expr(lex, reporter));
            if lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_expr];
                let mut last_loc = ptoken.loc;
                while lex.peek().kind == TokenKind::Comma {
                    lex.next(); // consume comma
                    let expr = try_parse!(
                        lex,
                        parse_expr_with_precedence(lex, Precedence::Assignment, reporter)
                    );
                    last_loc = expr.loc();
                    exprs.push(expr);
                }
                let left_tuple = Expr::new(ExprKind::Tuple(exprs, last_loc));

                let op_token = lex.peek().clone();
                if let Some(assign_kind) = token_to_assign_kind(op_token.kind) {
                    lex.next(); // consume assign operator
                    let (lex, first_right) = try_parse!(parse_expr_with_precedence(
                        lex,
                        Precedence::Lowest,
                        reporter
                    ));
                    let mut current_lex = lex;

                    let right = if current_lex.peek().kind == TokenKind::Comma {
                        let mut rexprs = vec![first_right];
                        let mut rlast_loc = op_token.loc;
                        while current_lex.peek().kind == TokenKind::Comma {
                            current_lex.next();
                            let (lex, expr) = try_parse!(parse_expr_with_precedence(
                                current_lex,
                                Precedence::Lowest,
                                reporter
                            ));
                            current_lex = lex;
                            rlast_loc = expr.loc();
                            rexprs.push(expr);
                        }
                        Expr::new(ExprKind::Tuple(rexprs, rlast_loc))
                    } else {
                        first_right
                    };

                    let assign_expr = Expr::new(ExprKind::Assign(AssignExpr {
                        left: Box::new(left_tuple),
                        assign_kind,
                        right: Box::new(right),
                    }));
                    Parser::Success(current_lex, Stmt::ExprStmt(assign_expr))
                } else {
                    reporter.report(op_token.loc, "expected assignment operator after tuple");
                    Parser::Fail(lex, ())
                }
            } else {
                Parser::Success(lex, Stmt::ExprStmt(first_expr))
            }
        }
    }
}

fn parse_identifier_or_keyword<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, Token, ()> {
    let token = lex.peek();
    if !matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword) {
        return Parser::Fail(lex, ());
    }
    let mut token = lex.next();
    token.kind = TokenKind::Identifier;
    Parser::Success(lex, token)
}

fn parse_identifier<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, Token, ()> {
    let token = lex.peek();
    if token.kind != TokenKind::Identifier {
        return Parser::Fail(lex, ());
    }
    let token = lex.next();
    Parser::Success(lex, token)
}

fn parse_ellipsis<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    let token = lex.peek();
    if token.kind != TokenKind::Ellipsis {
        return Parser::Fail(lex, ());
    }
    lex.next();
    Parser::Success(lex, ())
}

fn parse_string_literal<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, Token, ()> {
    let token = lex.peek();
    if token.kind != TokenKind::StringLiteral {
        return Parser::Fail(lex, ());
    }
    let token = lex.next();
    Parser::Success(lex, token)
}

fn expect<'lex>(
    lex: RefLexer<'lex>,
    kind: TokenKind,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, (), ()> {
    let token = lex.next();
    if token.kind != kind {
        reporter.report(
            token.loc,
            format!("expected {:?}, found {:?}", kind, token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, ())
}

fn parse_init_list_contents<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<InitElement>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    if lex.peek().kind == TokenKind::CloseCurly {
        lex.next();
        return Parser::Success(lex, Vec::new());
    }

    let mut elements = Vec::new();
    loop {
        if lex.peek().kind == TokenKind::CloseCurly {
            break;
        }

        let expr = try_parse!(lex, parse_expr(lex, reporter));
        if let ExprKind::Identifier(ident_tok) = &expr.kind
            && lex.peek().kind == TokenKind::Colon
        {
            lex.next(); // Consume ':'
            let val_expr = try_parse!(lex, parse_expr(lex, reporter));
            elements.push(InitElement::Named {
                name: ident_tok.clone(),
                value: val_expr,
            });
        } else {
            elements.push(InitElement::Positional(expr));
        }

        let next = lex.peek();
        if next.kind == TokenKind::Comma {
            lex.next();
        } else if next.kind == TokenKind::CloseCurly {
            break;
        } else {
            reporter.report(
                next.loc,
                format!("expected Comma or CloseCurly, found {:?}", next.kind),
            );
            return Parser::Fail(lex, ());
        }
    }

    let close_tok = lex.next();
    if close_tok.kind != TokenKind::CloseCurly {
        reporter.report(
            close_tok.loc,
            format!("expected CloseCurly, found {:?}", close_tok.kind),
        );
        return Parser::Fail(lex, ());
    }

    Parser::Success(lex, elements)
}

fn parse_arguments<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Argument>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenParen {
        reporter.report(
            token.loc,
            format!("expected OpenParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let mut arguments = Vec::new();
    let mut first_named = false;

    loop {
        let savepoint = lex.save();
        let peek = lex.peek();
        if peek.kind == TokenKind::CloseParen {
            break;
        }

        if first_named {
            lex.restore(savepoint);
            let _ = lex.next();
            let arg = try_parse!(
                lex,
                parse_named_argument(lex, reporter).or_else(|lex| {
                    reporter.report(lex.peek().loc, "Expected named argument");
                    Parser::Fail(lex, ())
                })
            );
            arguments.push(arg);
        } else {
            lex.restore(savepoint);
            let _ = lex.next();
            let savepoint_before_arg = lex.save();
            let parsed_named = parse_named_argument(lex, reporter);
            match parsed_named {
                Parser::Success(next_lex, arg) => {
                    lex = next_lex;
                    first_named = true;
                    arguments.push(arg);
                }
                Parser::Fail(mut next_lex, _) => {
                    next_lex.restore(savepoint_before_arg);
                    let _ = next_lex.next();
                    let expr = try_parse!(next_lex, parse_expr(next_lex, reporter));
                    lex = next_lex;
                    arguments.push(Argument::Positional(expr));
                }
            }
        }

        let next = lex.peek();
        if next.kind == TokenKind::Comma {
            lex.next();
        } else if next.kind == TokenKind::CloseParen {
            break;
        } else {
            reporter.report(
                next.loc,
                format!("expected Comma or CloseParen, found {:?}", next.kind),
            );
            return Parser::Fail(lex, ());
        }
    }

    let token = lex.next();
    if token.kind != TokenKind::CloseParen {
        reporter.report(
            token.loc,
            format!("expected CloseParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, arguments)
}

fn parse_named_argument<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Argument, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    try_parse!(lex, parse_colon(lex));
    let expr = try_parse!(lex, parse_expr(lex, reporter));
    Parser::Success(lex, Argument::Named { name, value: expr })
}

fn parse_array<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Expr>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenBracket {
        reporter.report(
            token.loc,
            format!("expected OpenBracket, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let array = try_parse!(lex, sep_by(lex, |l| parse_expr(l, reporter), parse_comma));
    let token = lex.next();
    if token.kind != TokenKind::CloseBracket {
        reporter.report(
            token.loc,
            format!("expected CloseBracket, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, array)
}

fn parse_fn_pointer_parameter<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FunctionPointerParameter, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    try_parse!(lex, parse_colon(lex));
    let typ = try_parse!(lex, parse_type(lex, reporter));
    Parser::Success(lex, FunctionPointerParameter { name, typ })
}

fn parse_field<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Field, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    try_parse!(lex, parse_colon(lex));
    let typ = try_parse!(lex, parse_type(lex, reporter));
    Parser::Success(lex, Field { name, typ })
}

fn parse_function_parameter<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FunctionParameter, ()> {
    let name = try_parse!(lex, parse_identifier_or_keyword(lex));
    try_parse!(lex, parse_colon(lex));
    let (typ, is_variadic) = if lex.peek().kind == TokenKind::Ellipsis {
        lex.next(); // Consume '...'
        let inner = try_parse!(lex, parse_type(lex, reporter));
        (Type::Slice(Box::new(inner)), true)
    } else {
        let t = try_parse!(lex, parse_type(lex, reporter));
        (t, false)
    };
    // let value = if lex.peek().kind == TokenKind::Eq {
    //     lex.next();
    //     Some(try_parse!(lex, parse_expr(lex, reporter)))
    // } else {
    //     None
    // };
    Parser::Success(
        lex,
        FunctionParameter {
            name,
            typ,
            // value,
            is_variadic,
        },
    )
}

fn parse_enum_variant<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, EnumVariant, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    let peek = lex.peek();
    if peek.kind == TokenKind::Eq {
        lex.next();
        let value = try_parse!(lex, parse_expr(lex, reporter));
        Parser::Success(lex, EnumVariant::DefaultValue(name, value))
    } else {
        Parser::Success(lex, EnumVariant::Name(name))
    }
}

fn parse_fn_pointer_parameters<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, (Vec<FunctionPointerParameter>, bool), ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenParen {
        reporter.report(
            token.loc,
            format!("expected OpenParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let (parameters, va_args) = try_parse!(
        lex,
        sep_by(
            lex,
            |l| parse_fn_pointer_parameter(l, reporter),
            parse_comma
        )
        .and_then(|lex, parameters| {
            match parse_ellipsis(lex) {
                Parser::Success(lex, _) => Parser::Success(lex, (parameters, true)),
                Parser::Fail(lex, _) => Parser::Success(lex, (parameters, false)),
            }
        })
    );

    let token = lex.next();
    if token.kind != TokenKind::CloseParen {
        reporter.report(
            token.loc,
            format!("expected CloseParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, (parameters, va_args))
}

fn parse_parameters<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, (Vec<FunctionParameter>, bool), ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenParen {
        reporter.report(
            token.loc,
            format!("expected OpenParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let (parameters, va_args) = try_parse!(
        lex,
        sep_by(lex, |l| parse_function_parameter(l, reporter), parse_comma).and_then(
            |lex, parameters| {
                match parse_ellipsis(lex) {
                    Parser::Success(lex, _) => Parser::Success(lex, (parameters, true)),
                    Parser::Fail(lex, _) => Parser::Success(lex, (parameters, false)),
                }
            }
        )
    );

    let token = lex.next();
    if token.kind != TokenKind::CloseParen {
        reporter.report(
            token.loc,
            format!("expected CloseParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, (parameters, va_args))
}

fn parse_maybe_comma<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::Comma {
        parse_comma(lex)
    } else {
        Parser::Success(lex, ())
    }
}

fn parse_comma<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::Comma {
        lex.next();
        return Parser::Success(lex, ());
    }
    Parser::Fail(lex, ())
}

fn parse_maybe_semicolon<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::SemiColon {
        parse_semicolon(lex)
    } else {
        Parser::Success(lex, ())
    }
}

fn parse_semicolon<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::SemiColon {
        lex.next();
        return Parser::Success(lex, ());
    }
    Parser::Fail(lex, ())
}

fn parse_colon<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::Colon {
        lex.next();
        return Parser::Success(lex, ());
    }
    Parser::Fail(lex, ())
}

fn parse_type<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Type, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::OpenParen => {
            let start_tok = lex.next();
            let mut types = Vec::new();
            if lex.peek().kind == TokenKind::CloseParen {
                lex.next();
                Parser::Success(lex, Type::Tuple(types, start_tok.loc))
            } else {
                loop {
                    let ty = try_parse!(lex, parse_type(lex, reporter));
                    types.push(ty);
                    let next = lex.peek();
                    if next.kind == TokenKind::Comma {
                        lex.next();
                    } else if next.kind == TokenKind::CloseParen {
                        lex.next();
                        break;
                    } else {
                        reporter.report(
                            next.loc,
                            format!(
                                "expected Comma or CloseParen in tuple type, found {:?}",
                                next.kind
                            ),
                        );
                        return Parser::Fail(lex, ());
                    }
                }
                if types.len() == 1 {
                    Parser::Success(lex, types.remove(0))
                } else {
                    Parser::Success(lex, Type::Tuple(types, start_tok.loc))
                }
            }
        }
        TokenKind::Caret => {
            lex.next();
            let mut pointer_count = 1;
            while lex.peek().kind == TokenKind::Caret {
                lex.next();
                pointer_count += 1;
            }
            let inner = try_parse!(lex, parse_type(lex, reporter));
            Parser::Success(lex, Type::Pointer(pointer_count, Box::new(inner)))
        }
        TokenKind::OpenBracket => {
            lex.next();
            if lex.peek().kind == TokenKind::CloseBracket {
                lex.next();
                let inner = try_parse!(lex, parse_type(lex, reporter));
                Parser::Success(lex, Type::Slice(Box::new(inner)))
            } else if lex.peek().kind == TokenKind::Identifier && lex.peek().source() == "dyn" {
                lex.next(); // Consume "dyn"
                try_parse!(lex, expect(lex, TokenKind::CloseBracket, reporter));
                let inner = try_parse!(lex, parse_type(lex, reporter));
                Parser::Success(lex, Type::DynArray(Box::new(inner)))
            } else {
                let array_size = if lex.peek().kind == TokenKind::Identifier {
                    ArraySize::Ident(lex.next())
                } else {
                    let size_token = try_parse!(lex, parse_number(lex, reporter));
                    ArraySize::Literal(size_token.value as usize)
                };
                try_parse!(lex, expect(lex, TokenKind::CloseBracket, reporter));
                let inner = try_parse!(lex, parse_type(lex, reporter));
                Parser::Success(lex, Type::Array(Box::new(inner), array_size))
            }
        }
        TokenKind::Identifier => {
            let mut ident = lex.next();
            let savepoint = lex.save();
            if lex.peek().kind == TokenKind::Dot {
                lex.next(); // Consume Dot
                if lex.peek().kind == TokenKind::Identifier {
                    let member_tok = lex.next();
                    let combined_source = TokenSource::from(format!(
                        "{}.{}",
                        ident.source.as_str(),
                        member_tok.source.as_str()
                    ));
                    ident.source = combined_source;
                } else {
                    _ = lex.next();
                    lex.restore(savepoint);
                }
            }
            Parser::Success(lex, Type::Scalar(ident))
        }
        TokenKind::Keyword if token.source() == "fn" => {
            let fn_tok = lex.next();
            let (parameters, _va_args) =
                try_parse!(lex, parse_fn_pointer_parameters(lex, reporter));
            let mut return_type = None;
            if lex.peek().kind == TokenKind::Arrow {
                lex.next();
                let typ = try_parse!(lex, parse_type(lex, reporter));
                return_type = Some(Box::new(typ));
            }
            Parser::Success(
                lex,
                Type::FnPointer {
                    parameters,
                    return_type,
                    loc: fn_tok.loc,
                },
            )
        }
        _ => {
            reporter.report(
                token.loc,
                format!("expected type name, '*' or '[', found {:?}", token.kind),
            );
            Parser::Fail(lex, ())
        }
    }
}

fn parse_expr<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    parse_expr_with_precedence(lex, Precedence::Lowest, reporter)
}

fn parse_expr_with_precedence<'lex>(
    mut lex: RefLexer<'lex>,
    precedence: Precedence,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let (l, mut left) = try_parse!(parse_prefix(lex, reporter));
    lex = l;

    loop {
        let ptoken = lex.peek();
        let p = token_to_precedence(ptoken);
        if precedence >= p {
            break;
        }

        let (l, next_left) = try_parse!(parse_infix(lex, left, reporter));
        lex = l;
        left = next_left;
    }

    Parser::Success(lex, left)
}

fn parse_prefix<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let ptoken = lex.peek();
    match ptoken.kind {
        TokenKind::Number(_) => {
            let num_lit = try_parse!(lex, parse_number(lex, reporter));
            let expr = Expr::new(ExprKind::Integer(num_lit));
            Parser::Success(lex, expr)
        }
        TokenKind::RealNumber => {
            let float_lit = try_parse!(lex, parse_float(lex, reporter));
            let expr = Expr::new(ExprKind::Float(float_lit));
            Parser::Success(lex, expr)
        }
        TokenKind::Keyword if ptoken.source() == "null" => {
            let loc = lex.next().loc;
            Parser::Success(lex, Expr::new(ExprKind::Null(loc)))
        }
        TokenKind::Keyword if ptoken.source() == "autocast" => {
            let cast_tok = lex.next();
            let (lex, expr) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::AutoCast(Box::new(expr), cast_tok.loc)),
            )
        }
        TokenKind::Keyword if ptoken.source() == "cast" => {
            let cast_tok = lex.next();
            try_parse!(lex, expect(lex, TokenKind::OpenParen, reporter));
            let target_type = try_parse!(lex, parse_type(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
            let (lex, expr) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Cast(target_type, Box::new(expr), cast_tok.loc)),
            )
        }
        TokenKind::Keyword if ptoken.source() == "new" => {
            let token = lex.next();
            try_parse!(lex, expect(lex, TokenKind::OpenParen, reporter));
            let target_type = try_parse!(lex, parse_type(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Make(target_type, vec![], token.loc)),
            )
        }
        TokenKind::Keyword if ptoken.source() == "make" => {
            let token = lex.next();
            try_parse!(lex, expect(lex, TokenKind::OpenParen, reporter));
            let target_type = try_parse!(lex, parse_type(lex, reporter));
            if lex.peek().kind == TokenKind::CloseParen {
                lex.next();
                Parser::Success(
                    lex,
                    Expr::new(ExprKind::Make(target_type, vec![], token.loc)),
                )
            } else if lex.peek().kind == TokenKind::Comma {
                lex.next();
                let mut args = Vec::new();
                if lex.peek().kind != TokenKind::CloseParen {
                    loop {
                        let expr = try_parse!(lex, parse_expr(lex, reporter));
                        args.push(expr);
                        if lex.peek().kind == TokenKind::Comma {
                            lex.next();
                            if lex.peek().kind == TokenKind::CloseParen {
                                lex.next();
                                break;
                            }
                        } else if lex.peek().kind == TokenKind::CloseParen {
                            lex.next();
                            break;
                        } else {
                            let next = lex.peek();
                            reporter.report(
                                next.loc,
                                format!(
                                    "expected Comma or CloseParen in make arguments, found {:?}",
                                    next.kind
                                ),
                            );
                            return Parser::Fail(lex, ());
                        }
                    }
                } else {
                    lex.next();
                }
                Parser::Success(lex, Expr::new(ExprKind::Make(target_type, args, token.loc)))
            } else {
                let next = lex.peek();
                reporter.report(
                    next.loc,
                    format!(
                        "expected Comma or CloseParen after target type in make, found {:?}",
                        next.kind
                    ),
                );
                Parser::Fail(lex, ())
            }
        }
        TokenKind::Keyword if ptoken.source() == "drop" => {
            let token = lex.next();
            try_parse!(lex, expect(lex, TokenKind::OpenParen, reporter));
            let expr = try_parse!(lex, parse_expr(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
            Parser::Success(lex, Expr::new(ExprKind::Drop(Box::new(expr), token.loc)))
        }
        TokenKind::Dot => {
            let dot_tok = lex.next();
            if lex.peek().kind == TokenKind::OpenCurly {
                let (l, elements) = try_parse!(parse_init_list_contents(lex, reporter));
                lex = l;
                let expr = Expr::new(ExprKind::InitList(InitListExpr {
                    loc: dot_tok.loc,
                    type_hint: None,
                    elements,
                }));
                Parser::Success(lex, expr)
            } else if lex.peek().kind == TokenKind::Identifier {
                let prop_tok = lex.next();
                let dummy_obj = Expr::new(ExprKind::Identifier(Token::new(
                    TokenKind::Identifier,
                    dot_tok.loc,
                    TokenSource::from("."),
                )));
                let expr = Expr::new(ExprKind::Member(MemberExpr {
                    object: Box::new(dummy_obj),
                    property: prop_tok,
                }));
                Parser::Success(lex, expr)
            } else {
                reporter.report(
                    dot_tok.loc,
                    "expected implicit initializer list or enum variant name after '.'",
                );
                Parser::Fail(lex, ())
            }
        }
        TokenKind::Directive => {
            let token = lex.next();
            if token.source() == "#type_info" && lex.peek().kind == TokenKind::OpenParen {
                lex.next(); // Consume '('
                let ty = try_parse!(lex, parse_type(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
                return Parser::Success(
                    lex,
                    Expr::new(ExprKind::TypeInfo(Box::new(ty), token.loc)),
                );
            } else if token.source() == "#sizeof" && lex.peek().kind == TokenKind::OpenParen {
                lex.next(); // Consume '('
                let ty = try_parse!(lex, parse_type(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
                return Parser::Success(lex, Expr::new(ExprKind::SizeOf(ty, token.loc)));
            } else if token.source() == "#alignof" && lex.peek().kind == TokenKind::OpenParen {
                lex.next(); // Consume '('
                let ty = try_parse!(lex, parse_type(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
                return Parser::Success(lex, Expr::new(ExprKind::AlignOf(ty, token.loc)));
            } else if token.source() == "#anycast" {
                if lex.peek().kind == TokenKind::OpenBracket {
                    let array = try_parse!(lex, parse_array(lex, reporter));
                    return Parser::Success(
                        lex,
                        Expr::new(ExprKind::AnyCast(AnyCastExpr::Array(array), token.loc)),
                    );
                }
                let expr = try_parse!(lex, parse_expr(lex, reporter));
                return Parser::Success(
                    lex,
                    Expr::new(ExprKind::AnyCast(
                        AnyCastExpr::Scalar(expr.into()),
                        token.loc,
                    )),
                );
            } else if token.source() == "#default" {
                return Parser::Success(lex, Expr::new(ExprKind::Default(token.loc)));
            } else if token.source() == "#feature" && lex.peek().kind == TokenKind::OpenParen {
                lex.next(); // Consume '('
                let value = try_parse!(
                    lex,
                    parse_identifier(lex).or_else(|lex| {
                        reporter.report(
                            token.loc,
                            format!("expect identifier as feature got '{}'", lex.peek().source()),
                        );
                        Parser::Fail(lex, ())
                    })
                );
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
                return Parser::Success(lex, Expr::new(ExprKind::Feature(token.loc, value)));
            }
            reporter.report(
                token.loc,
                format!("unsupported directive '{}'", token.source()),
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Identifier => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::Identifier(token));
            Parser::Success(lex, expr)
        }
        TokenKind::StringLiteral => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::StringLiteral(token));
            Parser::Success(lex, expr)
        }
        TokenKind::CharacterLiteral => {
            let token = lex.next();
            let unescaped = token.unescape();
            let mut chars = unescaped.chars();
            if let Some(ch) = chars.next() {
                if chars.next().is_none() {
                    let expr = Expr::new(ExprKind::Char(CharLiteral {
                        loc: token.loc,
                        value: ch as u32,
                    }));
                    Parser::Success(lex, expr)
                } else {
                    reporter.report(
                        token.loc,
                        "character literal must contain exactly one character",
                    );
                    Parser::Fail(lex, ())
                }
            } else {
                reporter.report(token.loc, "empty character literal");
                Parser::Fail(lex, ())
            }
        }
        TokenKind::EmptyCharacterLiteral => {
            let token = lex.next();
            reporter.report(token.loc, "empty character literal");
            Parser::Fail(lex, ())
        }
        TokenKind::UnterminatedCharacterLiteral => {
            let token = lex.next();
            reporter.report(token.loc, "unterminated character literal");
            Parser::Fail(lex, ())
        }
        TokenKind::Keyword if ptoken.source() == "true" || ptoken.source() == "false" => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::Bool(BoolLiteral {
                loc: token.loc,
                value: token.source() == "true",
            }));
            Parser::Success(lex, expr)
        }
        TokenKind::OpenBracket => {
            let loc = ptoken.loc;
            let typ = try_parse!(lex, parse_type(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::Dot, reporter));
            let (l, elements) = try_parse!(parse_init_list_contents(lex, reporter));
            lex = l;
            let expr = Expr::new(ExprKind::InitList(InitListExpr {
                loc,
                type_hint: Some(typ),
                elements,
            }));
            Parser::Success(lex, expr)
        }
        TokenKind::OpenParen => {
            let open_tok = lex.next();
            if lex.peek().kind == TokenKind::CloseParen {
                lex.next();
                Parser::Success(lex, Expr::new(ExprKind::Tuple(Vec::new(), open_tok.loc)))
            } else {
                let mut exprs = Vec::new();
                let mut has_comma = false;
                loop {
                    let expr = try_parse!(lex, parse_expr(lex, reporter));
                    exprs.push(expr);
                    let next = lex.peek();
                    if next.kind == TokenKind::Comma {
                        lex.next();
                        has_comma = true;
                    } else if next.kind == TokenKind::CloseParen {
                        lex.next();
                        break;
                    } else {
                        reporter.report(
                            next.loc,
                            format!(
                                "expected Comma or CloseParen in tuple expression, found {:?}",
                                next.kind
                            ),
                        );
                        return Parser::Fail(lex, ());
                    }
                }
                if exprs.len() == 1 && !has_comma {
                    Parser::Success(lex, exprs.remove(0))
                } else {
                    Parser::Success(lex, Expr::new(ExprKind::Tuple(exprs, open_tok.loc)))
                }
            }
        }
        TokenKind::Minus
        | TokenKind::Bang
        | TokenKind::Ampersand
        | TokenKind::DoubleAmpersand
        | TokenKind::Asterisk => {
            let token = lex.next();
            let (op1, op2) = match token.kind {
                TokenKind::Minus => (Some(Op::Neg), None),
                TokenKind::Bang => (Some(Op::Not), None),
                TokenKind::Ampersand => (Some(Op::Refer), None),
                TokenKind::DoubleAmpersand => (Some(Op::Refer), Some(Op::Refer)),
                TokenKind::Asterisk => (Some(Op::Deref), None),
                _ => unreachable!(),
            };
            let (lex, right) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            let mut expr = right;
            if let Some(op) = op2 {
                expr = Expr::new(ExprKind::Unary(UnaryExpr {
                    op,
                    right: Box::new(expr),
                }));
            }
            if let Some(op) = op1 {
                expr = Expr::new(ExprKind::Unary(UnaryExpr {
                    op,
                    right: Box::new(expr),
                }));
            }
            Parser::Success(lex, expr)
        }
        TokenKind::EOF => {
            let token = lex.next();
            reporter.report(token.loc, "expected expression but got EOF");
            Parser::Fail(lex, ())
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn parse_number<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, IntegerLiteral, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::Number(base) => {
            let token = lex.next();
            let value = match u64::from_str_radix(token.source(), base.radix()) {
                Ok(ok) => ok,
                Err(err) => {
                    reporter.report(token.loc, format!("invalid integer: {}", err));
                    return Parser::Fail(lex, ());
                }
            };
            Parser::Success(
                lex,
                IntegerLiteral {
                    loc: token.loc,
                    value,
                },
            )
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn parse_float<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FloatLiteral, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::RealNumber => {
            let token = lex.next();
            let value = match token.source().parse::<f64>() {
                Ok(ok) => ok,
                Err(err) => {
                    reporter.report(token.loc, format!("invalid float literal: {}", err));
                    return Parser::Fail(lex, ());
                }
            };
            Parser::Success(
                lex,
                FloatLiteral {
                    loc: token.loc,
                    value,
                },
            )
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn token_to_assign_kind(kind: TokenKind) -> Option<AssignKind> {
    match kind {
        TokenKind::Assign | TokenKind::Eq => Some(AssignKind::Default),
        TokenKind::PlusEq => Some(AssignKind::Add),
        TokenKind::MinusEq => Some(AssignKind::Sub),
        TokenKind::AsteriskEq => Some(AssignKind::Mul),
        TokenKind::SlashEq => Some(AssignKind::Div),
        TokenKind::ModEq => Some(AssignKind::Mod),
        _ => None,
    }
}

fn parse_infix<'lex>(
    mut lex: RefLexer<'lex>,
    left: Expr,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let token = lex.peek().clone();
    match token.kind {
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Asterisk
        | TokenKind::Slash
        | TokenKind::Mod
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq
        | TokenKind::EqEq
        | TokenKind::NotEq => {
            lex.next();
            let op = match token.kind {
                TokenKind::Plus => Op::Add,
                TokenKind::Minus => Op::Sub,
                TokenKind::Asterisk => Op::Mul,
                TokenKind::Slash => Op::Div,
                TokenKind::Mod => Op::Mod,
                TokenKind::Lt => Op::Lt,
                TokenKind::LtEq => Op::LtEq,
                TokenKind::Gt => Op::Gt,
                TokenKind::GtEq => Op::GtEq,
                TokenKind::EqEq => Op::Eq,
                TokenKind::NotEq => Op::NotEq,
                _ => unreachable!(),
            };
            let precedence = token_to_precedence(&token);
            let (lex, right) = try_parse!(parse_expr_with_precedence(lex, precedence, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Binary(BinaryExpr {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                    use_operator_overload: None,
                })),
            )
        }
        TokenKind::DoubleAmpersand
        | TokenKind::DoublePipe
        | TokenKind::Ampersand
        | TokenKind::Pipe
        | TokenKind::Caret => {
            lex.next();
            let op = match token.kind {
                TokenKind::DoubleAmpersand => Op::And,
                TokenKind::DoublePipe => Op::Or,
                TokenKind::Ampersand => Op::BitAnd,
                TokenKind::Pipe => Op::BitOr,
                TokenKind::Caret => Op::BitXor,
                _ => unreachable!(),
            };
            let precedence = token_to_precedence(&token);
            let (lex, right) = try_parse!(parse_expr_with_precedence(lex, precedence, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Binary(BinaryExpr {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                    use_operator_overload: None,
                })),
            )
        }
        t if t.is_assign_kind() => {
            lex.next();
            let assign_kind = match token_to_assign_kind(token.kind) {
                Some(k) => k,
                None => {
                    reporter.report(token.loc, format!("invalid assignment: {}", token.source()));
                    return Parser::Fail(lex, ());
                }
            };
            let (mut current_lex, first_right) = try_parse!(parse_expr_with_precedence(
                lex,
                Precedence::Lowest,
                reporter
            ));
            let right = if current_lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_right];
                let mut last_loc = token.loc;
                while current_lex.peek().kind == TokenKind::Comma {
                    current_lex.next();
                    let (lex, expr) = try_parse!(parse_expr_with_precedence(
                        current_lex,
                        Precedence::Lowest,
                        reporter
                    ));
                    current_lex = lex;
                    last_loc = expr.loc();
                    exprs.push(expr);
                }
                Expr::new(ExprKind::Tuple(exprs, last_loc))
            } else {
                first_right
            };
            Parser::Success(
                current_lex,
                Expr::new(ExprKind::Assign(AssignExpr {
                    left: Box::new(left),
                    assign_kind,
                    right: Box::new(right),
                })),
            )
        }
        TokenKind::DoubleColon => {
            reporter.report(
                token.loc,
                "associated paths or namespace lookup using '::' is not supported yet",
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Dot => {
            lex.next(); // Consume '.'

            if let ExprKind::Identifier(name) = &left.kind
                && lex.peek().kind == TokenKind::OpenCurly
            {
                let (l, elements) = try_parse!(parse_init_list_contents(lex, reporter));
                lex = l;
                let expr = Expr::new(ExprKind::InitList(InitListExpr {
                    loc: name.loc,
                    type_hint: Some(Type::Scalar(name.clone())),
                    elements,
                }));
                return Parser::Success(lex, expr);
            }

            let peek = lex.peek();
            let property = if peek.kind == TokenKind::Identifier {
                lex.next()
            } else if let TokenKind::Number(_) = peek.kind {
                lex.next()
            } else {
                reporter.report(
                    peek.loc,
                    format!("expected identifier, number or `{{`, found {:?}", peek.kind),
                );
                return Parser::Fail(lex, ());
            };

            // Support namespaced struct literal: io.SomeStruct.{ ... }
            if let ExprKind::Identifier(obj) = &left.kind
                && property.kind == TokenKind::Identifier
            {
                let savepoint = lex.save();
                if lex.peek().kind == TokenKind::Dot {
                    lex.next(); // Consume '.'
                    if lex.peek().kind == TokenKind::OpenCurly {
                        let (l, elements) = try_parse!(parse_init_list_contents(lex, reporter));
                        lex = l;
                        let combined = format!("{}.{}", obj.source(), property.source());
                        let combined_token = Token::new(
                            TokenKind::Identifier,
                            obj.loc,
                            TokenSource::from(combined.as_str()),
                        );
                        let expr = Expr::new(ExprKind::InitList(InitListExpr {
                            loc: obj.loc,
                            type_hint: Some(Type::Scalar(combined_token)),
                            elements,
                        }));
                        return Parser::Success(lex, expr);
                    } else {
                        let _ = lex.next(); // Clear the peek cache!
                        lex.restore(savepoint);
                    }
                }
            }

            Parser::Success(
                lex,
                Expr::new(ExprKind::Member(MemberExpr {
                    object: Box::new(left),
                    property,
                })),
            )
        }
        TokenKind::OpenParen => {
            let (lex, arguments) = try_parse!(parse_arguments(lex, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Call(CallExpr {
                    loc: left.loc(),
                    callee: Box::new(left),
                    arguments,
                    resolved_name: None,
                    module_name: None,
                })),
            )
        }
        TokenKind::OpenBracket => {
            lex.next(); // Consume '['
            let loc = token.loc;
            let (mut final_lex, index) = try_parse!(parse_expr(lex, reporter));
            try_parse!(
                final_lex,
                expect(final_lex, TokenKind::CloseBracket, reporter)
            );
            Parser::Success(
                final_lex,
                Expr::new(ExprKind::Index(IndexExpr {
                    loc,
                    array: Box::new(left),
                    index: Box::new(index),
                })),
            )
        }
        _ => {
            reporter.report(
                token.loc,
                format!("unexpected token in expression: {}", token.source()),
            );
            Parser::Fail(lex, ())
        }
    }
}

fn parse_var_decl_inner<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, VarDeclStmt, ()> {
    let mut is_private = false;
    let mut is_foreign = false;
    let mut is_export = false;
    let mut is_thread_local = false;
    let mut foreign_lib = None;
    let mut link_name = None;
    // while lex.peek().kind == TokenKind::Directive {
    //     let value = lex.next();
    //     match value.source() {
    //         "#private" => is_private = true,
    //         "#foreign" => {
    //             let lib_name = try_parse!(lex, parse_identifier(lex));
    //             is_foreign = true;
    //             foreign_lib = Some((value.loc, lib_name));
    //         }
    //         "#link_name" => {
    //             let name = try_parse!(lex, parse_string_literal(lex));
    //             link_name = Some(name);
    //         }
    //         _ => {
    //             reporter.report(
    //                 value.loc,
    //                 format!("unknown var directive: {}", value.source()),
    //             );
    //             return Parser::Fail(lex, ());
    //         }
    //     }
    // }

    let mut names = Vec::new();
    let first_name = try_parse!(lex, parse_identifier(lex));
    names.push(first_name);

    while lex.peek().kind == TokenKind::Comma {
        lex.next();
        let name = try_parse!(lex, parse_identifier(lex));
        names.push(name);
    }

    let mut var_type = None;
    if lex.peek().kind == TokenKind::Colon {
        lex.next();
        let typ = try_parse!(lex, parse_type(lex, reporter));
        var_type = Some(typ);
    }

    // Trailing directives (e.g. `var x: int #foreign Libc`)
    while lex.peek().kind == TokenKind::Directive {
        let value = lex.next();
        match value.source() {
            "#export" => is_export = true,
            "#thread_local" => is_thread_local = true,
            "#private" => is_private = true,
            "#foreign" => {
                let lib_name = try_parse!(lex, parse_identifier(lex));
                is_foreign = true;
                foreign_lib = Some((value.loc, lib_name));
            }
            "#link_name" => {
                let name = try_parse!(lex, parse_string_literal(lex));
                link_name = Some(name);
            }
            _ => {
                reporter.report(
                    value.loc,
                    format!("unknown var directive: {}", value.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    let expr = if is_foreign && lex.peek().kind != TokenKind::Eq {
        Expr::new(ExprKind::Null(lex.peek().loc))
    } else {
        try_parse!(lex, expect(lex, TokenKind::Eq, reporter));
        try_parse!(lex, parse_expr(lex, reporter))
    };

    Parser::Success(
        lex,
        VarDeclStmt {
            names,
            var_type,
            expr,
            is_thread_local,
            is_export,
            is_private,
            is_foreign,
            foreign_lib,
            link_name,
        },
    )
}

fn parse_const_declaration<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, ConstDecl, ()> {
    let const_token = lex.next();
    if const_token.kind != TokenKind::Keyword || const_token.source() != "const" {
        reporter.report(
            const_token.loc,
            format!("expected 'const', found {:?}", const_token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let name = try_parse!(lex, parse_identifier(lex));
    let mut typ = if lex.peek().kind == TokenKind::Colon {
        try_parse!(lex, expect(lex, TokenKind::Colon, reporter));
        Some(try_parse!(lex, parse_type(lex, reporter)))
    } else {
        None
    };
    try_parse!(lex, expect(lex, TokenKind::Eq, reporter));
    let expr = try_parse!(lex, parse_expr(lex, reporter));

    if typ.is_none() {
        match expr.kind {
            ExprKind::Integer(_) => {
                typ = Some(Type::Scalar(Token {
                    kind: TokenKind::Identifier,
                    loc: expr.loc(),
                    source: TokenSource("untyped_int"),
                }));
            }
            ExprKind::Float(_) => {
                typ = Some(Type::Scalar(Token {
                    kind: TokenKind::Identifier,
                    loc: expr.loc(),
                    source: TokenSource("untyped_float"),
                }));
            }
            _ => (),
        }
    }

    if typ.is_none() {
        reporter.report(
            name.loc,
            format!("expected type for constant {}", name.source()),
        );
        return Parser::Fail(lex, ());
    }

    Parser::Success(
        lex,
        ConstDecl {
            loc: const_token.loc,
            name,
            typ: typ.unwrap(),
            expr,
        },
    )
}
