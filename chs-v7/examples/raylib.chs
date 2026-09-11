module main

library Raylib {
    link_name = "raylib",
    kind = "dynlib"
}

fn init_window(w: int, h: int, title: ^u8) #foreign Raylib #link_name "InitWindow"
fn close_window() #foreign Raylib #link_name "CloseWindow"
fn window_should_close() -> bool #foreign Raylib #link_name "WindowShouldClose"
fn begin_drawing() #foreign Raylib #link_name "BeginDrawing"
fn end_drawing() #foreign Raylib #link_name "EndDrawing"
fn clear_background(c: Color) #foreign Raylib #link_name "ClearBackground"
fn get_color(hex: int) -> Color #foreign Raylib #link_name "GetColor"

type Color struct {
    r: u8, g: u8, b: u8, a: u8
}

fn main() {
    init_window(800, 600, "test".data);
    defer close_window();
    for !window_should_close() {
        begin_drawing();
        clear_background(get_color(0x181818FF));
        end_drawing();
    }
}
