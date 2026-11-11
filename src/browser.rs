use std::process::{Command, Stdio};

pub fn open(url: &str) {
    if std::env::var_os("NO_BROWSER").is_some() {
        return;
    }
    let mut command = launcher(url);
    let spawned = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if spawned.is_err() {
        println!("  Could not open a browser automatically: open {url} manually.\n");
    }
}

#[cfg(target_os = "windows")]
fn launcher(url: &str) -> Command {
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", url]);
    command
}

#[cfg(target_os = "macos")]
fn launcher(url: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(url);
    command
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn launcher(url: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    command
}
