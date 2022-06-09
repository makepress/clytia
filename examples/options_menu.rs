use clytia::{Clytia, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut clytia = Clytia::default();

    println!("What animal do you like?");
    let choice = clytia.options_menu(["cats", "dogs", "both"])?;
    match choice {
        "cats" => println!("What about dogs?"),
        "dogs" => println!("But what about cats?"),
        "both" => println!("Good choice!"),
        _ => unreachable!(),
    }

    Ok(())
}
