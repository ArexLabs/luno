# Luno Language Specification v1.0

## Language Philosophy

Luno is a compiled, memory-safe, type-safe systems and application language.
It combines Go's operational simplicity, Rust's safety guarantees, and
Python's productivity, without the complexity of any of them.

**Core values:**
- Simplicity over cleverness
- Explicit over magical
- Composition over inheritance
- Fast compilation as a feature
- Predictable performance
- Zero hidden runtime cost

---

## 1. Syntax

### 1.1 Comments

```
# Single-line comment
```

### 1.2 Blocks

All blocks use curly braces `{}`. No indentation sensitivity.

```
fn main() {
    print("hello")
}
```

### 1.3 Identifiers

- Variables/functions: `camelCase`
- Types/Enums: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`

### 1.4 Literals

```
42              # Int
3.14            # Float
true            # Bool
false           # Bool
"hello"         # String
'c'             # Char (single byte)
```

---

## 2. Variables

```
# Short declaration (type inferred)
name := "Luno"
count := 42

# Explicit type
age: Int = 30

# Immutable constant
const PI = 3.14159

# Reassignment (must be declared with := first)
count = count + 1
```

Variables are **mutable by default**. Use `const` for immutability.

---

## 3. Functions

Every function in Luno is **asynchronous by default** — all functions
can use `await` to suspend until a future is ready.

```
fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn fetch(url: String) -> String {
    response := await httpGet(url)
    return response
}

fn main() {
    print(greet("Luno"))
}
```

### 3.1 Parameters

Parameters are typed with `name: Type` syntax. Default values:

```
fn div(a: Int, b: Int = 1) -> Int {
    return a / b
}
```

### 3.2 Multiple Return Values

```
fn divide(a: Int, b: Int) -> (Int, Int) {
    return (a / b, a % b)
}

fn main() {
    (q, r) := divide(10, 3)
}
```

---

## 4. Types

### 4.1 Primitives

| Type       | Size    | Description              |
|------------|---------|--------------------------|
| `Int`      | 64-bit  | Signed integer           |
| `Float`    | 64-bit  | IEEE-754 float           |
| `Bool`     | 8-bit   | Boolean                  |
| `Char`     | 8-bit   | ASCII byte               |
| `String`   | dynamic | UTF-8 string             |
| `Byte`     | 8-bit   | Unsigned byte            |
| `Future[T]`| pointer | Async task handle        |
| `Chan[T]`  | pointer | Thread-safe channel      |

### 4.2 Type Aliases

```
type Age = Int
type Name = String
```

### 4.3 Struct Types

```
type Point {
    x: Float
    y: Float
}

type User {
    id: Int
    name: String
    email: String
}

# Instantiation
fn main() {
    p := Point { x: 0.0, y: 0.0 }
    u := User {
        id: 1
        name: "Luno"
        email: "luno@example.com"
    }
}
```

### 4.4 Enum Types

```
enum Option[T] {
    Some(T)
    None
}

enum Result[T, E] {
    Ok(T)
    Err(E)
}

enum Color {
    Red
    Green
    Blue
    Rgb(Int, Int, Int)
}

fn main() {
    c := Color::Red
    rgb := Color::Rgb(255, 0, 0)

    match c {
        Color::Red => print("red")
        Color::Green => print("green")
        Color::Blue => print("blue")
        Color::Rgb(r, g, b) => print(`rgb(${r}, ${g}, ${b})`)
    }
}
```

---

## 5. Methods

Methods are defined separately from types using `impl`.

```
type Point {
    x: Float
    y: Float
}

impl Point {
    fn new(x: Float, y: Float) -> Point {
        return Point { x: x, y: y }
    }

    fn distance_to(self, other: Point) -> Float {
        dx := self.x - other.x
        dy := self.y - other.y
        return (dx*dx + dy*dy).sqrt()
    }

    fn scale(self, factor: Float) -> Point {
        return Point { x: self.x * factor, y: self.y * factor }
    }
}
```

### 5.1 Self Parameter

- `self` — value receiver (consumes or copies)
- `self: &Self` — immutable reference (default for read-only)
- `self: &mut Self` — mutable reference

---

## 6. Interfaces / Traits

```
trait Stringify {
    fn to_string(self: &Self) -> String
}

impl Stringify for Point {
    fn to_string(self: &Self) -> String {
        return `Point(${self.x}, ${self.y})`
    }
}

fn print_static(s: &Stringify) {
    print(s.to_string())
}

fn main() {
    p := Point { x: 3.0, y: 4.0 }
    print_static(&p)
}
```

Interfaces are **structural** (implicit satisfaction) like Go, not
nominal like Java. If a type has the required methods, it satisfies
the trait automatically.

---

## 7. Control Flow

### 7.1 If / Elif / Else

```
if x > 0 {
    print("positive")
} elif x < 0 {
    print("negative")
} else {
    print("zero")
}
```

### 7.2 For Loops

```
# Range
for i in 0..10 {
    print(i)
}

# Iterate list
for name in names {
    print(name)
}

# While
while condition {
    process()
}
```

### 7.3 Break / Continue

```
for i in 0..100 {
    if i % 2 == 0 {
        continue
    }
    if i > 50 {
        break
    }
    print(i)
}
```

---

## 8. Pattern Matching

```
match value {
    0 => print("zero")
    1 => print("one")
    _ => print("other")
}

match result {
    Ok(val) => print(val)
    Err(e) => log(e)
}

match point {
    Point { x: 0, y: 0 } => print("origin")
    Point { x, y } => print(`(${x}, ${y})`)
}
```

Match must be **exhaustive** — every possible value must be covered.
Use `_` as wildcard catch-all.

---

## 9. Error Handling

Luno uses `Result[T, E]` for all fallible operations.
No exceptions. No try/catch.

```
fn readFile(path: String) -> Result[String, IOError]

fn main() -> Result[(), String] {
    # ? propagates errors to caller
    content := readFile("hello.txt")?
    print(content)
    return Ok(())
}
```

### 9.1 Error Propagation

The `?` operator:
- If value is `Ok(x)`: unwraps to `x`
- If value is `Err(e)`: returns early with `Err(e.into())`

### 9.2 Error Handling Pattern

```
fn process(path: String) -> Result[(), String] {
    result := readFile(path)

    match result {
        Ok(content) => {
            print(content)
            return Ok(())
        }
        Err(e) => {
            log("failed: " + e)
            return Err(e)
        }
    }
}
```

---

## 10. Generics

```
fn identity[T](x: T) -> T {
    return x
}

type Pair[A, B] {
    first: A
    second: B
}

fn first[A, B](p: Pair[A, B]) -> A {
    return p.first
}
```

---

## 11. Ownership & Memory Model

Luno uses **affine types** with **automatic lifetime inference**.
Every value has exactly one owner at any time.

### 11.1 Ownership Rules

1. Each value has exactly one owner
2. When the owner goes out of scope, the value is dropped
3. Ownership can be moved (transferred)
4. References borrow without taking ownership

### 11.2 Move Semantics

```
type LargeData {
    buffer: String
}

fn take(data: LargeData) {
    # 'data' owns the value now
    print(data.buffer)
}

fn main() {
    d := LargeData { buffer: "hello" }
    take(d)          # ownership moves to take()
    # print(d)      # ERROR: d no longer owns the value
}
```

### 11.3 Borrowing

```
fn read(data: &String) {
    print(data)      # borrows immutably
}

fn write(data: &mut String) {
    data.push_str("!")
}

fn main() {
    s := "hello"
    read(&s)         # immutable borrow
    write(&mut s)    # mutable borrow (exclusive)
    print(s)         # OK: borrows are released
}
```

### 11.4 Borrowing Rules (enforced at compile time)

At any point, you can have either:
- Any number of immutable borrows (`&T`), OR
- Exactly one mutable borrow (`&mut T`)

Lifetimes are **automatically inferred**. No explicit lifetime annotations.

### 11.5 Copy Types

Primitive types (`Int`, `Float`, `Bool`, `Char`, `Byte`) are `Copy` —
they are automatically duplicated on assignment instead of moved.

### 11.6 Memory Safety Guarantees

- No use-after-free
- No double-free
- No dangling pointers
- No buffer overflows
- No null pointer dereference (no null in the language)

---

## 12. Concurrency

Concurrency and asynchronous execution are **default** in Luno.
Every function is async-capable: `await` is available anywhere,
and `spawn` starts concurrent work.

### 12.1 Async by Default

All functions can use `await`. No `async fn` keyword is needed.

```
fn fetch(url: String) -> String {
    response := await httpGet(url)
    return response
}

fn compute() -> Int {
    # No await needed — synchronous in effect, async by default
    return 42
}
```

### 12.2 Await

`await` suspends the current function until a `Future[T]` produces its
value. It unwraps `Future[T]` → `T`.

```
fn main() {
    f := spawn fetchData()
    # do other work concurrently
    data := await f      # blocks until data is ready
    print(data)
}
```

### 12.3 Spawn (Structured Concurrency)

`spawn` creates a concurrent task and returns a `Future[T]`.

```
fn main() {
    handle := spawn downloadFile("large.dat")
    processOtherStuff()
    result := await handle
    print(result)
}
```

### 12.4 Futures

`Future[T]` represents a value that will be available later. It is a
built-in generic type.

```
fn main() {
    f1 := spawn readFile("a.txt")
    f2 := spawn readFile("b.txt")

    a := await f1
    b := await f2

    print(a + b)
}
```

`Future` is **move-only** — it can only be awaited once.

### 12.5 Channels

```
fn main() {
    ch := make(Chan[String], 16)     # buffered channel

    spawn fn() {
        ch.send("hello from task")
    }

    msg := ch.recv()
    print(msg)
}
```

Built-in channel operations:
- `make(Chan[T], size)` — create buffered channel
- `ch.send(value)` — send value (blocks if full)
- `ch.recv()` — receive value (blocks if empty)
- `ch.try_send(value)` — non-blocking send (returns bool)
- `ch.try_recv()` — non-blocking receive (returns Option[T])

### 12.6 Safety Guarantees

- No data races (enforced by borrow checker across threads)
- Automatic cancellation on dropped futures
- Channels are thread-safe

---

## 13. Package System

### 13.1 Project Structure

```
myproject/
├── luno.json          # Package manifest
├── src/
│   ├── main.luno      # Entry point
│   └── lib.luno       # Library code
└── tests/
    └── test_lib.luno
```

### 13.2 Package Manifest (luno.json)

```json
{
    "name": "myproject",
    "version": "0.1.0",
    "edition": "2026",
    "dependencies": {}
}
```

### 13.3 Imports

```
# Import module
import http
import json

# Import specific items
import { readFile, writeFile } from fs

# Import with alias
import { readFile as load } from fs
```

### 13.4 Visibility

- `fn` / `type` — public by default at module level
- `fn _helper()` — underscore prefix makes it private

---

## 14. Standard Library (Built-in)

| Module     | Contents                         |
|------------|----------------------------------|
| `builtin`  | `print`, `len`, `range`, `type`  |
| `fs`       | `readFile`, `writeFile`, `exists`|
| `http`     | `get`, `post`, `Client`          |
| `json`     | `parse`, `stringify`             |
| `math`     | `sqrt`, `sin`, `cos`, `abs`, `pi`|
| `time`     | `now`, `sleep`, `Duration`       |
| `os`       | `args`, `env`, `exit`            |
| `sync`     | `Chan`, `Mutex`, `WaitGroup`     |
| `renderer` | `createWindow`, `drawTriangle`   |

---

## 15. Compilation Pipeline

```
Source (.luno)
    │
    ▼
Lexer → Tokens
    │
    ▼
Parser → AST (Abstract Syntax Tree)
    │
    ▼
Type Checker → Typed AST (validated)
    │
    ▼
Code Generator → C source or LLVM IR
    │
    ▼
Backend (gcc/clang/llc) → Native Binary
    │
    ▼
Executable
```

---

## 16. Toolchain

| Command         | Description                    |
|-----------------|--------------------------------|
| `luno build`    | Compile project                |
| `luno run`      | Compile and run                |
| `luno check`    | Type-check only                |
| `luno fmt`      | Format source code             |
| `luno test`     | Run tests                      |
| `luno doc`      | Generate documentation         |
| `luno new`      | Create new project             |
| `luno repl`     | Start interactive REPL         |

---

## 17. Binary Characteristics

- **Startup time**: ~1ms (no runtime initialization)
- **Binary size**: 10KB–200KB for typical programs
- **Memory overhead**: minimal runtime (~4KB)
- **Dependencies**: statically linked, single binary
- **No VM, no interpreter, no GC pause**
