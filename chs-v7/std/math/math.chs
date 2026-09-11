module math

library LibM {
    link_name = "m",
    kind = "dynlib",
}

// --- Mathematical Constants ---
const PI = 3.141592653589793
const TAU = 6.283185307179586
const E = 2.718281828459045
const SQRT2 = 1.4142135623730951
const LOG2E = 1.4426950408889634
const LOG10E = 0.4342944819032518
const LN2 = 0.6931471805599453
const LN10 = 2.302585092994046

type double f64

// --- Foreign Bindings to C libm ---
fn sqrt(x: double) -> double #foreign LibM #link_name "sqrt"
fn cbrt(x: double) -> double #foreign LibM #link_name "cbrt"
fn pow(base: double, exp: double) -> double #foreign LibM #link_name "pow"
fn sin(x: double) -> double #foreign LibM #link_name "sin"
fn cos(x: double) -> double #foreign LibM #link_name "cos"
fn tan(x: double) -> double #foreign LibM #link_name "tan"
fn asin(x: double) -> double #foreign LibM #link_name "asin"
fn acos(x: double) -> double #foreign LibM #link_name "acos"
fn atan(x: double) -> double #foreign LibM #link_name "atan"
fn atan2(y: double, x: double) -> double #foreign LibM #link_name "atan2"
fn sinh(x: double) -> double #foreign LibM #link_name "sinh"
fn cosh(x: double) -> double #foreign LibM #link_name "cosh"
fn tanh(x: double) -> double #foreign LibM #link_name "tanh"
fn floor(x: double) -> double #foreign LibM #link_name "floor"
fn ceil(x: double) -> double #foreign LibM #link_name "ceil"
fn round(x: double) -> double #foreign LibM #link_name "round"
fn trunc(x: double) -> double #foreign LibM #link_name "trunc"
fn fabs(x: double) -> double #foreign LibM #link_name "fabs"
fn fmod(x: double, y: double) -> double #foreign LibM #link_name "fmod"
fn log(x: double) -> double #foreign LibM #link_name "log"
fn log2(x: double) -> double #foreign LibM #link_name "log2"
fn log10(x: double) -> double #foreign LibM #link_name "log10"
fn exp(x: double) -> double #foreign LibM #link_name "exp"
fn exp2(x: double) -> double #foreign LibM #link_name "exp2"
fn hypot(x: double, y: double) -> float #foreign LibM #link_name "hypot"

// --- Pure Utilities ---

// Integer math utilities
fn abs(x: int) -> int {
    if x < 0 {
        return -x;
    };
    return x;
}

fn min(a: int, b: int) -> int {
    if a < b {
        return a;
    };
    return b;
}

fn max(a: int, b: int) -> int {
    if a > b {
        return a;
    };
    return b;
}

fn clamp(val: int, min_val: int, max_val: int) -> int {
    if val < min_val {
        return min_val;
    };
    if val > max_val {
        return max_val;
    };
    return val;
}

fn sign(x: int) -> int {
    if x < 0 {
        return -1;
    } else if x > 0 {
        return 1;
    };
    return 0;
}

// Float math utilities
fn fmin(a: float, b: float) -> float {
    if a < b {
        return a;
    };
    return b;
}

fn fmax(a: float, b: float) -> float {
    if a > b {
        return a;
    };
    return b;
}

fn fclamp(val: float, min_val: float, max_val: float) -> float {
    if val < min_val {
        return min_val;
    };
    if val > max_val {
        return max_val;
    };
    return val;
}

fn fsign(x: float) -> float {
    if x < 0.0 {
        return -1.0;
    } else if x > 0.0 {
        return 1.0;
    };
    return 0.0;
}

fn lerp(a: float, b: float, t: float) -> float {
    return a + (b - a) * t;
}

fn rad_to_deg(rad: float) -> float {
    return rad * (180.0 / PI);
}

fn deg_to_rad(deg: float) -> float {
    return deg * (PI / 180.0);
}
