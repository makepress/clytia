use std::sync::atomic::{AtomicUsize, Ordering};

use clytia::{Clytia, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Clytia::default();

    let counter = AtomicUsize::new(0);
    cli.progress_bar(
        "Waiting for 10 seconds (ish)",
        || counter.load(Ordering::Relaxed) / 100,
        || -> Result<(), &str> {
            let mut count = counter.load(Ordering::Relaxed);
            while count < 10_000 {
                std::thread::sleep(std::time::Duration::from_millis(1));
                count = counter.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        },
    )??;

    Ok(())
}
