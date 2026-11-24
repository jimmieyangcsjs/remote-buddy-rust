fn main() {
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next().unwrap_or_default();
    let name = arguments.next().unwrap_or_default();
    let mut worker_thread = false;
    let mut delay = std::time::Duration::ZERO;
    while let Some(argument) = arguments.next() {
        if argument == "--thread" {
            worker_thread = true;
        } else if argument == "--delay" {
            let milliseconds = arguments
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            delay = std::time::Duration::from_millis(milliseconds);
        }
    }
    let activate = move || match remote_buddy_windows::apps::focus_or_launch_named(&path, &name) {
        Ok(()) => {
            println!("activated: {name} | {path} | thread={worker_thread}");
            true
        }
        Err(error) => {
            eprintln!("activation failed: {error} | thread={worker_thread}");
            false
        }
    };
    let activated = if worker_thread {
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            activate()
        })
        .join()
        .unwrap_or(false)
    } else {
        std::thread::sleep(delay);
        activate()
    };
    if !activated {
        std::process::exit(1);
    }
}
