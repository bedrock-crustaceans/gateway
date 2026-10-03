<br />
<div align="center">

# Gateway

A MITM Proxy tool for Minecraft: Bedrock written in rust

[![rust][rust_badge_url]][rust_url]
[![protocol][protocol_badge_url]][protocol_url]
[![license][license_badge_url]][license_url]

</div>

<!-- BADGES -->

[protocol_badge_url]: https://img.shields.io/badge/protocol-v2193-white?style=flat-square
[protocol_url]: https://github.com/Mojang/bedrock-protocol-docs
[rust_badge_url]: https://img.shields.io/badge/rust-2024-%23D34516?style=flat-square&logo=rust&logoColor=%23D34516&labelColor=white
[rust_url]: https://rust-lang.org/
[license_badge_url]: https://img.shields.io/github/license/bedrock-crustaceans/gateway?style=flat-square
[license_url]: LICENSE

<!-- BADGES -->
## Usage

1. Run the target server with `online-mode=false` (Gateway re-signs the login with its own key, like ProxyPass).
2. `cargo run`, set the proxy bind address and the target server address, then press play.
3. Connect your client to the proxy address. Packets show up in both directions; click one to inspect it.

### Filtering

The search box takes a small query language:

| Query | Shows |
| --- | --- |
| `text move` | packets whose name contains any of the words |
| `-move` / `!move` | hides packets whose name contains the word |
| `id:9` | packets with that id |
| `dir:c2s` / `dir:s2c` | one direction |
| `from:192.168` | clients whose address contains the text |
| `size:>1000` | packets by size in bytes (`<`, `>`, `=`) |

The **Filters** button lists every packet type and client seen so far, each with Show / Only / Hide.
Right-click a packet for the same actions on its type or client.
