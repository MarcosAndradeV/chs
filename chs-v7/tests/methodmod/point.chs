module methodmod

type Point struct {
    x: int,
    y: int,
}

fn (self: Point) sum() -> int {
    return self.x + self.y;
}

fn (self: ^Point) add_x(dx: int) {
    self.x = self.x + dx;
}

var test_point: Point #private = .{}
