<div align="center">

# rsbash

![GitHub issues](https://img.shields.io/github/issues/ljelliot/rash)
![Cargo Downloads](https://img.shields.io/crates/d/rsbash)
![Cargo Latest version](https://img.shields.io/crates/v/rsbash)

</div>

**rsbash**: run bash commands from rust. Simply.

There's as many ways to use **rsbash** as there are ways to use your beloved bash shell, but here are some examples anyway!

### Some ways you can use **rsbash** ...

#### A simple command:

```rust
use rsbash::{rash, RashError};

pub fn simple() -> Result<(), RashError> {
    let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
    assert_eq!(ret_val, 0);
    assert_eq!(stdout, "Hello world!");
    Ok(())
}
```

#### A _less_ simple command:

```rust
use rsbash::{rash, RashError};

pub fn less_simple() -> Result<(), RashError> {
    let (ret_val, stdout) =
        rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
    assert_eq!(ret_val, 0);
    assert_eq!(stdout, "Hello world!");
    Ok(())
}
```

#### With a format!'d string:

```rust
use rsbash::rash;
use tempfile::TempDir;

pub fn format() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().to_str().unwrap();
    let message = "Hi from within bar.txt!";
    let script = format!("cd {}; echo -n '{}' > bar.txt; cat bar.txt;", path, message);

    assert_eq!(rash!(script)?, (0, String::from(message)));

    dir.close()?;
    Ok(())
}
```

#### Execute a script:

```rust
use rsbash::rash;
use tempfile::TempDir;

pub fn script() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().to_str().unwrap();
    let message = "Hi from within foo.sh!";
    let script = format!(
        "cd {}; echo -n \"echo -n '{}'\" > foo.sh; chmod u+x foo.sh; ./foo.sh;",
        path, message
    );
    
    assert_eq!(rash!(script)?, (0, String::from(message)));

    dir.close()?;
    Ok(())
}
```

#### Using a raw string script:

```rust
use rsbash::{rash, RashError};

const SCRIPT: &'static str = r#"
    s="*"
    for i in {1..3}; do
        echo "$s"
        s="$s *"
    done;
    "#;

pub fn raw_script() -> Result<(), RashError> {  // ... it prints a lovely triangle.
    let (ret_val, stdout) = rash!(SCRIPT)?;     // *
    assert_eq!(ret_val, 0);                     // * *
    assert_eq!(stdout, "*\n* *\n* * *\n");      // * * *   
    Ok(())
}
```

## License

MIT License - Copyright (c) 2023 Luke Elliot

