// TODO: Review #link_name's when support other OS
module libc

type Whence enum : int {
	SET = SEEK_SET,
	CUR = SEEK_CUR,
	END = SEEK_END,
}

// if CHS_OS == .Linux {
type fpos_t #distinct u16

const _IOFBF = 0
const _IOLBF = 1
const _IONBF = 2
const BUFSIZ = 1024
const EOF: int = -1
const FOPEN_MAX = 1000
const FILENAME_MAX = 4096
const L_tmpnam = 20
const SEEK_SET = 0
const SEEK_CUR = 1
const SEEK_END = 2
const TMP_MAX = 308915776

var stderr: ^FILE #foreign Libc
var stdin:  ^FILE #foreign Libc
var stdout: ^FILE #foreign Libc

// }

fn remove (filename: cstring) -> int #foreign Libc
fn rename (old: cstring, new: cstring) -> int #link_name "rename" #foreign Libc
fn tmpfile () -> ^FILE #foreign Libc
fn tmpnam (s: ^char) -> ^char #foreign Libc
fn fclose (stream: ^FILE) -> int #foreign Libc
fn fflush (stream: ^FILE) -> int #foreign Libc
fn fopen (filename: cstring, mode: cstring) -> ^FILE #foreign Libc
fn freopen (filename: cstring, mode: cstring, stream: ^FILE) -> ^FILE #foreign Libc
fn setbuf (stream: ^FILE, buf: ^char) #foreign Libc
fn setvbuf (stream: ^FILE, buf: ^char, mode: int, size: size_t) -> int #foreign Libc
fn fprintf (stream: ^FILE, format: cstring, ...) -> int #foreign Libc
fn fscanf (stream: ^FILE, format: cstring, ...) -> int #foreign Libc
fn printf (format: cstring, ...) -> int #foreign Libc
fn scanf (format: cstring, ...) -> int #foreign Libc
fn snprintf (s: ^char, n: size_t, format: cstring, ...) -> int #foreign Libc
fn sscanf (s: cstring, format: cstring, ...) -> int #foreign Libc
fn vfprintf (stream: ^FILE, format: cstring, arg: ^va_list) -> int #foreign Libc
fn vfscanf (stream: ^FILE, format: cstring, arg: ^va_list) -> int #foreign Libc
fn vprintf (format: cstring, arg: ^va_list) -> int #foreign Libc
fn vscanf (format: cstring, arg: ^va_list) -> int #foreign Libc
fn vsnprintf (s: ^char, n: size_t, format: cstring, arg: ^va_list) -> int #foreign Libc
fn vsprintf (s: ^char, format: cstring, arg: ^va_list) -> int #foreign Libc
fn vsscanf (s: cstring, format: cstring, arg: ^va_list) -> int #foreign Libc

fn fgetc (stream: ^FILE) -> int #foreign Libc
fn fgets (s: ^char, n: int, stream: ^FILE) -> ^char #foreign Libc
fn fputc (s: int, stream: ^FILE) -> int #foreign Libc
fn getc (stream: ^FILE) -> int #foreign Libc
fn getchar () -> int #foreign Libc
fn putc (c: int, stream: ^FILE) -> int #foreign Libc
fn putchar (c: int) -> int #foreign Libc
fn puts (s: cstring) -> int #foreign Libc
fn ungetc (c: int, stream: ^FILE) -> int #foreign Libc
fn fread (ptr: rawptr, size: size_t, nmemb: size_t, stream: ^FILE) -> size_t #foreign Libc
fn fwrite (ptr: rawptr, size: size_t, nmemb: size_t, stream: ^FILE) -> size_t #foreign Libc

fn fgetpos (stream: ^FILE, pos: ^fpos_t) -> int #link_name "fgetpos" #foreign Libc
fn fseek (stream: ^FILE, offset: long, whence: Whence) -> int #foreign Libc

fn fsetpos (stream: ^FILE, pos: ^fpos_t) -> int #link_name "fsetpos" #foreign Libc
fn ftell (stream: ^FILE) -> long #foreign Libc
fn rewind (stream: ^FILE) #foreign Libc

fn clearerr (stream: ^FILE) #foreign Libc
fn feof (stream: ^FILE) -> int #foreign Libc
fn ferror (stream: ^FILE) -> int #foreign Libc
fn perror (s: cstring) #foreign Libc
