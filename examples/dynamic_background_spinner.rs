use std::sync::atomic::{AtomicUsize, Ordering};

use clytia::{Clytia, Result};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Clytia::default();

    let counter = AtomicUsize::new(0);
    cli.dynamic_background_spinner::<_, _, _, _, &str>(
        || {
            let count = counter.load(Ordering::Relaxed);
            if count >= 10_000 {
                "Waited for 2 seconds (ish)".to_string()
            } else {
                format!("Waiting. {}/10000ms left.", 10_000 - count)
            }
        },
        || {
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
