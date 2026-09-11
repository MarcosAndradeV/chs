module main

const SCREEN_WIDTH = 800
const SCREEN_HEIGHT = 450

type Ball struct {
    pos: Vector2
    speed: Vector2
    radius: f32
    color: Color
}

var ball: Ball = .{}

fn (b: ^Ball) init() {
    b.radius = 8.0;
    b.pos = Vector2.{cast(f32)SCREEN_WIDTH / 2.0, cast(f32)SCREEN_HEIGHT / 2.0};
    b.speed = Vector2.{5.0, 5.0};
    b.color = get_color(0xFFFFFFFF);
}

fn (ball: ^Ball) update() {
    ball.pos.x += ball.speed.x;
    ball.pos.y += ball.speed.y;
    if ball.pos.y - ball.radius <= 0.0 || ball.pos.y + ball.radius >= cast(f32) SCREEN_HEIGHT {
        ball.speed.y *= -1.0;
    };
    if ball.pos.x - ball.radius <= 0.0 || ball.pos.x + ball.radius >= cast(f32) SCREEN_WIDTH {
        ball.speed.x *= -1.0;
    };
}

fn main() {
    ball.init();
    init_window(w: SCREEN_WIDTH, h: SCREEN_HEIGHT, title: "Pong".data);
    defer close_window();
    set_target_fps(60);

    for !window_should_close() {
        update();
        begin_drawing();
        clear_background(get_color(0x181818FF));
        draw();
        end_drawing();
    }
}

fn update() {
    ball.update();
}

fn draw() {
    draw_circle_v(ball.pos, ball.radius, ball.color);
}

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
fn draw_circle_v(center: Vector2, radius: f32, color: Color) #foreign Raylib #link_name "DrawCircleV"
fn get_frame_time() -> f32 #foreign Raylib #link_name "GetFrameTime"
fn set_target_fps(fps: int) #foreign Raylib #link_name "SetTargetFPS"
fn get_screen_height() -> int #foreign Raylib #link_name "GetScreenHeight"
fn get_screen_width() -> int #foreign Raylib #link_name "GetScreenWidth"
fn get_mouse_position() -> Vector2 #foreign Raylib #link_name "GetMousePosition"
fn check_collision_circles(center1: Vector2, radius1: f32, center2: Vector2, radius2: f32) -> bool #foreign Raylib #link_name "CheckCollisionCircles"

type Color struct {
    r: u8, g: u8, b: u8, a: u8
}

type Vector2 struct { x: f32, y: f32 }
