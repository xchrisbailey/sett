# Debugging the sync deadlock

The deadlock happens when two writers hold the index lock and wait on each other. Here is the minimal repro:

```rust
use std::sync::{Arc, Mutex};

fn main() {
    let a = Arc::new(Mutex::new(0));
    let b = Arc::new(Mutex::new(0));
    let (a2, b2) = (a.clone(), b.clone());
    std::thread::spawn(move || {
        let _x = a2.lock().unwrap();
        let _y = b2.lock().unwrap();
    });
    let _y = b.lock().unwrap();
    let _x = a.lock().unwrap();
}
```

Run it with:

```sh
RUST_BACKTRACE=1 cargo run --release
```

A code block without a language:

```
plain text block
    with indentation kept
```

Tilde fence too:

~~~python
def hello():
    print("hi")
~~~

And an indented code block:

    let legacy = true;
    println!("{legacy}");

The fix is to always take `a` before `b`. Tracked in [[Sync engine]].
