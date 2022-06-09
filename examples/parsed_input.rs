use clytia::{Clytia, Error, Result};

fn main() -> Result<()> {
    let mut clytia = Clytia::default();

    let result: Result<usize, _> = clytia.parsed_input("Please enter a number", None);
    match result {
        Ok(number) => {
            println!("Double your number is {}", number * 2);
            Ok(())
        }
        Err(Error::NonOptionalInput) => {
            println!("You didn't enter a number!");
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
