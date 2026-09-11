// TODO: Review when support other OS
module libc

// TODO add a OS comptime check
// if OS = LINUX {
const RAND_MAX = 0x7fffffff

fn __ctype_get_mb_cur_max() -> size_t #foreign Libc #private #link_name "__ctype_get_mb_cur_max"

fn MB_CUR_MAX() -> size_t {
    return __ctype_get_mb_cur_max();
}

// }

const EXIT_SUCCESS = 0
const EXIT_FAILURE = 1

type div_t struct { quot: int, rem: int }
type ldiv_t struct { quot: int, rem: long }
type lldiv_t struct { quot: int, rem: longlong }

fn atof(nptr: cstring) -> double #foreign Libc #link_name "atof"
fn atoi(nptr: cstring) -> int #foreign Libc #link_name "atoi"
fn atol(nptr: cstring) -> long #foreign Libc #link_name "atol"
fn atoll(nptr: cstring) -> longlong #foreign Libc #link_name "atoll"
fn strtod(nptr: cstring, endptr: ^^char) -> double #foreign Libc #link_name "strtod"
fn strtof(nptr: cstring, endptr: ^^char) -> float #foreign Libc #link_name "strtof"
fn strtol(nptr: cstring, endptr: ^^char, base: int) -> long #foreign Libc #link_name "strtol"
fn strtoll(nptr: cstring, endptr: ^^char, base: int) -> longlong #foreign Libc #link_name "strtoll"
fn strtoul(nptr: cstring, endptr: ^^char, base: int) -> ulong #foreign Libc #link_name "strtoul"
fn strtoull(nptr: cstring, endptr: ^^char, base: int) -> ulonglong #foreign Libc #link_name "strtoull"

fn rand() -> int #foreign Libc #link_name "rand"
fn srand(seed: uint) #foreign Libc #link_name "srand"

fn calloc(nmemb: size_t, size: size_t) -> rawptr #foreign Libc #link_name "calloc"
fn free(ptr: rawptr) #foreign Libc #link_name "free"
fn malloc(size: size_t) -> rawptr #foreign Libc #link_name "malloc"
fn realloc(ptr: rawptr, size: size_t) -> rawptr #foreign Libc #link_name "realloc"

fn abort() -> noreturn #foreign Libc #link_name "abort"
fn atexit(func: fn()) -> int #foreign Libc #link_name "atexit"
fn at_quick_exit(func: fn()) -> int #foreign Libc #link_name "at_quick_exit"
fn exit(status: int) -> noreturn #foreign Libc #link_name "exit"
fn _Exit(status: int) -> noreturn #foreign Libc #link_name "_Exit"
fn getenv(name: cstring) -> cstring #foreign Libc #link_name "getenv"
fn quick_exit(status: int) -> noreturn #foreign Libc #link_name "quick_exit"
fn system(cmd: cstring) -> int #foreign Libc #link_name "system"

fn bsearch(key: rawptr, base: rawptr, nmemb: size_t, size: size_t, compar: fn(lhs: rawptr, rhs: rawptr) -> int) -> rawptr #foreign Libc #link_name "bsearch"
fn qsort(base: rawptr, nmemb: size_t, size: size_t, compar: fn(lhs: rawptr, rhs: rawptr) -> int) #foreign Libc #link_name "qsort"

fn abs(j: int) -> int #foreign Libc #link_name "abs"
fn labs(j: long) -> long #foreign Libc #link_name "labs"
fn llabs(j: longlong) -> longlong #foreign Libc #link_name "llabs"
fn div(numer: int, denom: int) -> div_t #foreign Libc #link_name "div"
fn ldiv(numer: long, denom: long) -> ldiv_t #foreign Libc #link_name "ldiv"
fn lldiv(numer: longlong, denom: longlong) -> lldiv_t #foreign Libc #link_name "lldiv"

fn mblen(s: cstring, n: size_t) -> int #foreign Libc #link_name "mblen"
fn mbtowc(pwc: ^wchar_t, s: cstring, n: size_t) -> int #foreign Libc #link_name "mbtowc"
fn wctomb(s: ^char, wc: wchar_t) -> int #foreign Libc #link_name "wctomb"

fn mbstowcs(pwcs: ^wchar_t, s: cstring, n: size_t) -> size_t #foreign Libc #link_name "mbstowcs"
fn wcstombs(s: ^char, pwcs: ^wchar_t, n: size_t) -> size_t #foreign Libc #link_name "wcstombs"
