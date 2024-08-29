alloc
    8
    8
    +
:= p

fn Point.x! : int ptr -> {
    !64
}

fn Point.x@ : ptr -> int {
    @64
}

fn Point.y! : int ptr -> {
    8 offset !64
}

fn Point.y@ : ptr -> int {
    8 offset @64
}

20 p Point.x!
p Point.x@ debug drop

10 p Point.y!
p Point.y@ debug drop
