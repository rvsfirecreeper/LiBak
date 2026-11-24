# LiBak

LiBak is a simple supporting encrypted backup tool.

Simple goals:
* Fast, avoid re-encrypting EVERYTHING on every single change
* Secure, using well-known primitives and zeroize + isolating cryptographically sensitive code
* Simple, with a UI anyone can understand
* Deduplicated

How it works:
* Claude is used FOR THE UI AND BUGFIXING(as little as possible only when I was super stumped) I WRITE THE CODE
* AES-GCM encryption, Argon2 KDF for security
* FastCDC, killing two birds with one stone as chunks create easy deduplication and prevent whole files being re-encrypted
* zstd compression
* Slint(the Claude UI in question)

## Installation

Linux: use Nix.
simply [install Nix](nixos.org/download) and run nix run github:rvsfirecreeper/libak
MacOS and windows: download the binary from releases page
that's it.
