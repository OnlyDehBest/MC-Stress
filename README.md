<p align="center">
  <img src="site/favicon.svg" width="72" alt="MC-Stress logo">
</p>

<h1 align="center">MC-Stress</h1>

<p align="center">
  <b>Load and stress testing for Minecraft servers, written in Rust.</b><br>
  Raw-protocol bots on tokio · live axum dashboard · breaking-point detection
</p>

<p align="center">
  <a href="https://onlydehbest.github.io/MC-Stress/">Website</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#bot-models">Bot models</a> ·
  <a href="#supported-versions">Versions</a>
</p>

---

MC-Stress spawns hundreds of lightweight bots that join your server like real players: in batches, spreading out from spawn, walking, sprinting and loading chunks. A web dashboard streams bots online, server latency and kicks in real time, and flags the **breaking point**, the load at which your server starts to struggle.

There are no headless game clients and no Node runtime. Each bot is a small async task that speaks the Minecraft protocol directly, so the load lands on the server and not on the machine running the test.

## Features

- **Raw protocol clients**: handshake, login, configuration and play states implemented by hand on tokio, with compression, keep-alives and teleport confirms handled per bot.
- **Multi-protocol**: the server version is detected through Server List Ping, or you can pin a protocol manually. Packet IDs are mapped per version from 1.20.2 to 26.3.
- **Realistic load**: per-bot view distance, spawn dispersion, and walking or sprinting at vanilla speeds, so each bot costs the server about as much as a real player.
- **Seven bot models**: from a parked connection to a sprinting explorer that streams fresh chunks.
- **Player activity**: optional chat messages, commands and arm swings on a timer, to exercise chat plugins, command handlers and packet listeners.
- **Live dashboard**: bots online, moving bots, latency, players online, errors and kicks, plus a load-over-time chart, pushed over WebSocket every second.
- **Breaking-point detection**: flags the bot count at which latency exceeds twice the baseline plus 60 ms, or at which the server starts kicking.
- **Private link**: a random access key protects the dashboard, so you can reach it safely from your phone.
- **Single binary**: the dashboard is embedded at compile time, with nothing extra to deploy.

## Quick start

Requires a recent [Rust toolchain](https://rustup.rs).

```bash
git clone https://github.com/OnlyDehBest/MC-Stress
cd MC-Stress
cargo run --release
```

On startup MC-Stress generates a random access key and prints a **private link**. On a desktop it also opens the dashboard in your browser. On a headless server (such as a VPS) it prints a second link with the machine's IP address, which you can open from your phone or PC:

```
  MC-Stress is running. Private links (keep them secret):

  This machine:   http://localhost:3000/?key=WVewFhV9MdlMY6IxQ0f6WSSl
  Phone / PC:     http://203.0.113.10:3000/?key=WVewFhV9MdlMY6IxQ0f6WSSl
```

Open the link once: the browser keeps the key in a cookie, so you can bookmark the dashboard. Requests without the key get `401 Access denied`, including the API and the WebSocket. Set the target server and press **Start**. Stop MC-Stress with `Ctrl+C`.

| Environment variable | Effect |
|---|---|
| `PORT=8080` | Listen on another port (default `3000`) |
| `MC_STRESS_KEY=yourSecretKey` | Use a fixed key (8+ letters or digits) so the link survives restarts |
| `NO_BROWSER=1` | Never try to open a browser |

```bash
PORT=8080 MC_STRESS_KEY=myLongSecretKey123 cargo run --release
```

> The dashboard is served over plain HTTP, so anyone who can see your traffic can read the key. Use a long key, and on untrusted networks prefer an SSH tunnel (`ssh -L 3000:localhost:3000 user@server`).

### Server requirements

- The target server must run with `online-mode=false`.
- Bots move on a constant `y`, so disable movement anti-cheat for the duration of the test.
- The breaking point is a proxy based on latency and kicks. For real TPS, run `/spark tps` alongside the test.

> Only stress-test servers you own or are authorised to test.

## Configuration

Every option is set from the dashboard.

| Option | Default | Description |
|---|---|---|
| Host / Port | `localhost` / `25565` | Target server |
| Nickname | `Bot` | Prefix for bot usernames |
| Protocol | auto | Auto-detect via Server List Ping, or force a protocol number |
| Bots | `120` | Total number of bots to connect |
| Batch | `10` | Bots added per step |
| Interval ms | `2000` | Delay between batches |
| View distance | `8` | Chunks requested by each bot |
| Spread s | `6` | Time spent dispersing from spawn |
| Bot model | `sentry` | See [Bot models](#bot-models) |
| Chat message | off | Sent periodically by every bot |
| Command | off | Run periodically by every bot (no leading `/` needed) |
| Swing | off | Periodic arm swings |
| Actions every s | `5` | Interval for chat, command and swing |

## Bot models

Listed from lightest to heaviest server cost:

| Model | Behaviour |
|---|---|
| `idle` | Connects only and holds at spawn |
| `sentry` | Disperses, then holds position (default) |
| `spread` | Disperses 4× wider, then holds |
| `wanderer` | Disperses, then wanders gently |
| `movement` | Constant in-place movement at 20 Hz |
| `runner` | Roams constantly at walking speed (4.3 blocks/s), streaming chunks |
| `voyager` | Travels at sprinting speed (5.6 blocks/s) for maximum chunk load |

## Supported versions

| Version | Protocol | | Version | Protocol |
|---|---|---|---|---|
| 1.20.2 | 764 | | 1.21.5 | 770 |
| 1.20.4 | 765 | | 1.21.6 | 771 |
| 1.20.6 | 766 | | 1.21.8 | 772 |
| 1.21.1 | 767 | | 26.1 | 773 |
| 1.21.3 | 768 | | 26.2 | 774 |
| 1.21.4 | 769 | | 26.3 | 775 |

If the detected version is not recognised, MC-Stress falls back to 1.21.1. You can force a protocol or add a mapping in `src/versions.rs`.

## HTTP API

The dashboard is a thin client over a small API, which you can also script against:

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/start` | Start a run (JSON body, camelCase fields) |
| `POST` | `/api/stop` | Stop the run and disconnect all bots |
| `GET` | `/ws` | WebSocket that pushes live stats and log lines |

```bash
curl -X POST http://localhost:3000/api/start \
  -H 'Cookie: mcs_key=YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -d '{"host":"localhost","port":25565,"target":200,"step":10,"intervalMs":2000,"mode":"runner"}'
```

Every request needs the key, either as the `mcs_key` cookie or by visiting `/?key=…` first:

```bash
curl -X POST http://localhost:3000/api/stop -H 'Cookie: mcs_key=YOUR_KEY'
```

Omitted fields use these defaults: `target` 20, `step` 5, `intervalMs` 3000, `viewDistance` 8, `spreadSeconds` 6, `mode` `sentry`, `actionIntervalMs` 5000.

## Project layout

```
src/
  main.rs      axum server, routes, metrics loop
  access.rs    private-link access key
  browser.rs   startup links, browser opening
  swarm.rs     bot orchestration, ramp-up, breaking-point logic
  bot.rs       single bot: connection, states, movement, activity
  model.rs     bot models and movement profiles
  protocol.rs  VarInt, packet framing, compression
  versions.rs  per-version packet ID mappings
  slp.rs       Server List Ping (version detection, latency)
public/        dashboard (embedded into the binary)
site/          project website (GitHub Pages)
```

## Disclaimer

MC-Stress is not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB.
