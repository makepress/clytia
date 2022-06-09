use clytia::{Clytia, Error, Result};

fn main() -> Result<()> {
    let mut clytia = Clytia::default();

    let result: Result<usize, _> =
        clytia.validated_input("Please enter a number", "1-10", |v| *v >= 1 && *v <= 10);
    match result {
        Ok(number) => {
            println!("Double your number is {}", number * 2);
            Ok(())
        }
        Err(Error::ParseError(_)) => {
            println!("You didn't enter a number!");
            Ok(())
        }
        Err(e) => Err(e),
    }?;

    Ok(())
}
