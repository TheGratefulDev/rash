<div align="center">

# RsBash

![GitHub issues](https://img.shields.io/github/issues/ljelliot/rash)
![Cargo Downloads](https://img.shields.io/crates/d/rsbash)
![Cargo Latest version](https://img.shields.io/crates/v/rsbash)

</div>

**RsBash** makes running arbitrarily complex bash commands wonderfully easy!

With **RsBash** you can easily use **all** your favourite bash operators in your commands, redirects `>`, pipes `|`, subshells `$(...)`, boolean logic `&& ||` etc!

## Examples

### Hello world!

```rust
use rsbash::rash;

pub fn hello() -> Result<(), RashError> {
    let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
    assert_eq!(ret_val, 0);
    assert_eq!(stdout, "Hello world!");

    let (ret_val, stdout) =
        rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
    assert_eq!(ret_val, 0);
    assert_eq!(stdout, "Hello world!");
}
```

#### If you want to get _really_ fancy ...

```rust
use rsbash::rash;

const SCRIPT: &'static str = r#"
    s="*"
    for i in {1..3}; do
        echo "$s"
        s="$s *"
    done;
    "#;

pub fn script() -> Result<(), RashError> {  // ... it prints a lovely triangle.
    let (ret_val, stdout) = rash!(SCRIPT)?; // *
    assert_eq!(ret_val, 0);                 // * *
    assert_eq!(stdout, "*\n* *\n* * *\n");  // * * *   
}
```

## License

MIT License - Copyright (c) 2023 Luke Elliot

