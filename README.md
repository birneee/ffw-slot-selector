# ffw-slot-selector

QR-code based time-slot booking system, built for "150 Jahre Feuerwehr Eibach — Bungee Jumping".

Participants scan a QR code, enter their name and email, and pick a time slot. An admin panel allows managing bookings.

## Tech stack

- **Backend**: Rust + [Axum](https://github.com/tokio-rs/axum), SQLite via [sqlx](https://github.com/launchbadge/sqlx)
- **Frontend**: Rust compiled to WASM via [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)
- **API**: OpenAPI spec → generated server stubs ([openapi-generator](https://openapi-generator.tech/)) and client ([progenitor](https://github.com/oxidecomputer/progenitor))
- **Build**: Nix flake

## Development

Enter the dev shell:

```shell
nix develop
```

Run the server:

```shell
DATABASE_URL=sqlite:./data.db RUST_LOG=info cargo run -p server -- --port 3000
```

The server starts on `http://localhost:3000`. On first run it prints the admin panel URL to the log.

## Build

```shell
nix build
./result/bin/server
```

## NixOS

Add to your `flake.nix`:

```nix
inputs.ffw-slot-selector.url = "github:youruser/ffw-slot-selector";
```

Then in your NixOS module:

```nix
{ inputs, ... }: {
  imports = [ inputs.ffw-slot-selector.nixosModules.default ];

  services.ffw-slot-selector = {
    enable = true;
    port = 3000;     # optional, default 3000
    logLevel = "info"; # optional, default info
  };
}
```

## Logs

```shell
journalctl -u ffw-slot-selector -f
```

## License

MIT — see [LICENSE](LICENSE).

The logos (`wappen.png`, `150-jahre-logo.png`) are not covered by the MIT license and remain the property of their respective owners.
