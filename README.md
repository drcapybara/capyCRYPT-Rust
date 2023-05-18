# capyCRYPT - A Complete Rust Cryptosystem

[![Build Status](https://github.com/drcapybara/capyCRYPT-Rust/actions/workflows/rust.yml/badge.svg)](https://github.com/drcapybara/capyCRYPT-Rust/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/cryptolib?style=flat-square)](https://crates.io/crates/cryptolib)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/auditless/cairo-template/blob/main/LICENSE) 

A Rust library implementing FIPS 202 paired with a non-standard Edwards-521 curve. Engineered for performance; supports arbitrary message size.

## Features
- **SHA-3:** Secure Hash Algorithm 3 (SHA-3) implementation for generating cryptographic hash values.
- **Edwards Elliptic Curve:** Edwards 521 curve implementation for elliptic curve cryptography (ECC) operations.

## Supported Operations
- **Message Digest:** Computes 512 bit hash of a given message.
- **MACs:** Computes 512 bit message authentication code of a given message.
- **Passkey:** Symmetric message encryption and decryption.
- **Public Key Cryptography:** Asymmetric message encryption under public key, decryption with secret key.
- **Zero-Knowledge:** Prove knowledge of secret information with ECDH Schnorr signatures.


## Installation
Add the following line to your `Cargo.toml` file:

```toml
capycrypt = "0.1.0"
