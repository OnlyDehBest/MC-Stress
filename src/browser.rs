use std::io::IsTerminal;
use std::net::{IpAddr, UdpSocket};
use std::process::{Command, Stdio};

pub fn announce(port: u16, key: &str) {
    let local = format!("http://localhost:{port}/?key={key}");
    println!("\n  MC-Stress is running. Private links (keep them secret):\n");
    println!("  This machine:   {}", link(&local));
    if let Some(ip) = outbound_ip() {
        println!("  Phone / PC:     {}", link(&format!("http://{ip}:{port}/?key={key}")));
    }
    println!();

    if std::env::var_os("NO_BROWSER").is_some() {
        return;
    }
    if !has_display() {
        println!("  No display detected: open one of the links above in your browser.\n");
        return;
    }
    let spawned = launcher(&local)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if spawned.is_err() {
        println!("  Could not open a browser automatically: open one of the links above.\n");
    }
}

fn link(url: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b]8;;{url}\x1b\\\x1b[4m{url}\x1b[24m\x1b]8;;\x1b\\")
    } else {
        url.to_string()
    }
}

fn outbound_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(ip)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn has_display() -> bool {
    true
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn has_display() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
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
