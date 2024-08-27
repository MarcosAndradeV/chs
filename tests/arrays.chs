
alloc 8 10 * := xs

fn Array.set : int int ptr -> { -- val idx ptr
    swap 8 * offset !64
}
fn Array.get : int ptr -> int { -- idx ptr
    swap 8 * offset @64
}


0
while dup 10 < {
    dup 10 + over xs Array.set
    1 +
} drop

0
while dup 10 < {
    dup xs Array.get debug drop
    1 +
} drop
