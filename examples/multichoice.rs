use clytia::{Clytia, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut clytia = Clytia::default();

    println!("What animal do you like?");
    let choices = clytia.multichoice(["cats", "dogs", "birds"])?;
    if choices.len() < 3 {
        println!("What, You don't like all of them?");
    } else {
        println!("Correct choice!");
    }

    Ok(())
}
