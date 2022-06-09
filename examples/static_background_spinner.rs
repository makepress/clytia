use clytia::{Clytia, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Clytia::default();

    cli.static_background_spinner::<_, _, _, &str>("A delay for 10 seconds", || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        Ok(())
    })??;

    Ok(())
}
