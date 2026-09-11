use std::path::PathBuf;

use diagnostic::DiagnosticReporter;

use crate::ast::FileItem;
use crate::parse_file;

#[test]
fn feature_type_defs() {
    let source = "module main

type Point struct {
        x: int,
        y: int
    }

    type NUMBERS enum {
        ONE,
        TWO,
    }

    type Seconds float
    type Dollar #distinct float";

    let mut reporter = DiagnosticReporter::new();

    let file_path = PathBuf::from("<feature_type_defs>");
    match parse_file(file_path.as_path(), source, &mut reporter) {
        Ok(file_ast) => {
            assert!(matches!(&file_ast.items[0], FileItem::Struct(_)));
            assert!(matches!(&file_ast.items[1], FileItem::Enum(_)));
            assert!(matches!(&file_ast.items[2], FileItem::TypeDecl(_)));
            assert!(matches!(&file_ast.items[3], FileItem::TypeDecl(_)));
        }
        Err(_) => {
            assert!(false, "{}", reporter.into_string());
        }
    }
}

#[test]
fn test_parse_make_and_new() {
    use crate::ast::{ExprKind, Stmt};

    let source = "module main

fn test() {
    var x = make(int);
    var y = make(int, 10);
    var z = make(Point, 1, 2);
    var w = new(int);
}";

    let mut reporter = DiagnosticReporter::new();
    let file_path = PathBuf::from("<test_make>");
    let file_ast = parse_file(file_path.as_path(), source, &mut reporter).expect("parse failed");

    let FileItem::Function(func) = &file_ast.items[0] else {
        panic!("expected function");
    };
    let stmts = &func.body.as_ref().unwrap().stmts;

    // var x = make(int)
    let Stmt::VarDecl(ref v1) = stmts[0] else {
        panic!("expected var decl");
    };
    let ExprKind::Make(_, ref args1, _) = v1.expr.kind else {
        panic!("expected make");
    };
    assert_eq!(args1.len(), 0);

    // var y = make(int, 10)
    let Stmt::VarDecl(ref v2) = stmts[1] else {
        panic!("expected var decl");
    };
    let ExprKind::Make(_, ref args2, _) = v2.expr.kind else {
        panic!("expected make");
    };
    assert_eq!(args2.len(), 1);

    // var z = make(Point, 1, 2)
    let Stmt::VarDecl(ref v3) = stmts[2] else {
        panic!("expected var decl");
    };
    let ExprKind::Make(_, ref args3, _) = v3.expr.kind else {
        panic!("expected make");
    };
    assert_eq!(args3.len(), 2);

    // var w = new(int)
    let Stmt::VarDecl(ref v4) = stmts[3] else {
        panic!("expected var decl");
    };
    let ExprKind::Make(_, ref args4, _) = v4.expr.kind else {
        panic!("expected make");
    };
    assert_eq!(args4.len(), 0);
}
