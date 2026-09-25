# Xode Solochain

A [Substrate](https://substrate.io/)-based solochain node for the Xode network, built from
Parity's [Solochain
Template](https://github.com/paritytech/polkadot-sdk/tree/master/templates/solochain).

All bugs, suggestions, and feature requests specific to this chain's own pallets and
configuration should be filed against this repository; upstream Substrate/FRAME issues should be
reported in the [Substrate](https://github.com/paritytech/polkadot-sdk/tree/master/substrate)
repository.

## Getting Started

Depending on your operating system and Rust version, there might be additional
packages required to compile this project. Check the
[Install](https://docs.substrate.io/install/) instructions for your platform for
the most common dependencies. Alternatively, you can use one of the [alternative
installation](#alternatives-installations) options.

Fetch the Xode Solochain code:

```sh
git clone https://github.com/D2-Xode/xode-solochain.git

cd xode-solochain
```

### Build

🔨 Use the following command to build the node without launching it:

```sh
cargo build --release
```

### Embedded Docs

After you build the project, you can use the following command to explore its
parameters and subcommands:

```sh
./target/release/xode-solochain-node -h
```

You can generate and view the [Rust
Docs](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) for this project
with this command:

```sh
cargo +nightly doc --open
```

### Single-Node Development Chain

The following command starts a single-node development chain that doesn't
persist state:

```sh
./target/release/xode-solochain-node --dev
```

To purge the development chain's state, run the following command:

```sh
./target/release/xode-solochain-node purge-chain --dev
```

To start the development chain with detailed logging, run the following command:

```sh
RUST_BACKTRACE=1 ./target/release/xode-solochain-node -ldebug --dev
```

Development chains:

- Maintain state in a `tmp` folder while the node is running.
- Use the **Alice** and **Bob** accounts as default validator authorities.
- Use the **Alice** account as the default `sudo` account.
- Are preconfigured with a genesis state (`/node/src/chain_spec.rs`) that
  includes several pre-funded development accounts.


To persist chain state between runs, specify a base path by running a command
similar to the following:

```sh
// Create a folder to use as the db base path
$ mkdir my-chain-state

// Use of that folder to store the chain state
$ ./target/release/xode-solochain-node --dev --base-path ./my-chain-state/

// Check the folder structure created inside the base path after running the chain
$ ls ./my-chain-state
chains
$ ls ./my-chain-state/chains/
dev
$ ls ./my-chain-state/chains/dev
db keystore network
```

### Connect with Polkadot-JS Apps Front-End

After you start the node locally, you can interact with it using the
hosted version of the [Polkadot/Substrate
Portal](https://polkadot.js.org/apps/#/explorer?rpc=ws://localhost:9944)
front-end by connecting to the local node endpoint. A hosted version is also
available on [IPFS](https://dotapps.io/). You can
also find the source code and instructions for hosting your own instance in the
[`polkadot-js/apps`](https://github.com/polkadot-js/apps) repository.

### EVM Compatibility and Ethereum JSON-RPC

The runtime includes `pallet-revive`, so the chain accepts Ethereum transactions and runs
Solidity contracts compiled to EVM bytecode with standard `solc`. Its EVM chain id is **34170**.

The node itself does not serve the Ethereum JSON-RPC API (`eth_*`). That is done by Parity's
separate `eth-rpc` binary (crate `pallet-revive-eth-rpc`), which connects to the node's WebSocket
RPC. It must come from the same polkadot-sdk release as the runtime, because it decodes the
runtime's `ReviveApi` results. This repository is on polkadot-sdk **stable2512**
(`pallet-revive` 0.12.2), which matches the **`polkadot-stable2512-2`** release.

Install `eth-rpc` in one of these two ways:

- Download the prebuilt binary (Linux x86_64; macOS arm64 is `eth-rpc-aarch64-apple-darwin`)
  from the [`polkadot-stable2512-2`
  release](https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2512-2) and
  check it against the published checksum:

  ```sh
  curl -LO https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-stable2512-2/eth-rpc
  curl -LO https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-stable2512-2/eth-rpc.sha256
  sha256sum -c eth-rpc.sha256 && chmod +x eth-rpc
  ```

- Or build it from source at the same release tag:

  ```sh
  cargo install --locked --git https://github.com/paritytech/polkadot-sdk \
    --tag polkadot-stable2512-2 pallet-revive-eth-rpc
  ```

Don't use `cargo install pallet-revive-eth-rpc --version 0.12.0 --locked` from crates.io: that
package's lockfile pins `pallet-revive` 0.12.0, whose runtime API result types differ from the
0.12.2 used here, so gas estimation and `eth_call` would break.

To run it against a local development chain:

```sh
# Terminal 1: the node, with its RPC on ws://127.0.0.1:9944
./target/release/xode-solochain-node --dev

# Terminal 2: eth-rpc, serving Ethereum JSON-RPC on http://127.0.0.1:8545
./eth-rpc --dev --node-rpc-url ws://127.0.0.1:9944
```

| Port  | Process | Purpose |
|-------|---------|---------|
| 9944  | node    | Substrate JSON-RPC (HTTP and WebSocket). `eth-rpc` connects here. |
| 30333 | node    | p2p |
| 9615  | node    | Prometheus metrics |
| 8545  | eth-rpc | Ethereum JSON-RPC (HTTP and WebSocket). Point wallets and tools here. |
| 9616  | eth-rpc | Prometheus metrics |

Use `--rpc-port` to move `eth-rpc` off 8545. Its `--dev` flag allows CORS from any origin; drop
it outside development.

The `dev` and `local` chain specs pre-fund the well-known Ethereum development accounts, the
same ones Parity's tooling and `eth-rpc --dev` use. Their keys are public, so never use them
outside a development chain:

| Name      | Address                                      | Private key |
|-----------|----------------------------------------------|-------------|
| Alith     | `0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac` | `0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133` |
| Baltathar | `0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0` | `0x8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b` |
| Charleth  | `0x798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc` | `0x0b6e18cafb6ed99687ec547bd28139cafdd2bffe70e6b688025de6b445aa5c5b` |
| Dorothy   | `0x773539d4Ac0e786233D90A233654ccEE26a613D9` | `0x39539ab1876910bbf3a223d84a29e28f1cb4e2e456503e7e91ed39b2e7223d68` |
| Ethan     | `0xFf64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB` | `0x7dce9bc8babb68fec1409be38c8e1a52650206a7ed90ff956ae8a6d15eeaaef4` |

Assets of `pallet-assets` are available to contracts as ERC20 tokens at
`0x[asset id as 8 hex digits]000000000000000000000000` + `01200000`, for example asset 1 at
`0x0000000100000000000000000000000001200000`, the same addresses as on the Xode parachain.

`eth_getBalance` reports what an account can spend: its free balance minus whatever must stay
(the existential deposit, or frozen funds), times 10^6 (one plank of XON is 10^6 wei).

[`scripts/evm-smoke-test`](./scripts/evm-smoke-test) checks all of this end to end against a
running `--dev` node and `eth-rpc`.

### Multi-Node Local Testnet

If you want to see the multi-node consensus algorithm in action, see [Simulate a
network](https://docs.substrate.io/tutorials/build-a-blockchain/simulate-network/).

## Project Structure

A Substrate project such as this consists of a number of components that are
spread across a few directories.

### Node

A blockchain node is an application that allows users to participate in a
blockchain network. Substrate-based blockchain nodes expose a number of
capabilities:

- Networking: Substrate nodes use the [`libp2p`](https://libp2p.io/) networking
  stack to allow the nodes in the network to communicate with one another.
- Consensus: Blockchains must have a way to come to
  [consensus](https://docs.substrate.io/fundamentals/consensus/) on the state of
  the network. Substrate makes it possible to supply custom consensus engines
  and also ships with several consensus mechanisms that have been built on top
  of [Web3 Foundation
  research](https://research.web3.foundation/Polkadot/protocols/NPoS).
- RPC Server: A remote procedure call (RPC) server is used to interact with
  Substrate nodes.

There are several files in the `node` directory. Take special note of the
following:

- [`chain_spec.rs`](./node/src/chain_spec.rs): A [chain
  specification](https://docs.substrate.io/build/chain-spec/) is a source code
  file that defines a Substrate chain's initial (genesis) state. Chain
  specifications are useful for development and testing, and critical when
  architecting the launch of a production chain. Take note of the
  `development_config` and `testnet_genesis` functions. These functions are
  used to define the genesis state for the local development chain
  configuration. These functions identify some [well-known
  accounts](https://docs.substrate.io/reference/command-line-tools/subkey/) and
  use them to configure the blockchain's initial state.
- [`service.rs`](./node/src/service.rs): This file defines the node
  implementation. Take note of the libraries that this file imports and the
  names of the functions it invokes. In particular, there are references to
  consensus-related topics, such as the [block finalization and
  forks](https://docs.substrate.io/fundamentals/consensus/#finalization-and-forks)
  and other [consensus
  mechanisms](https://docs.substrate.io/fundamentals/consensus/#default-consensus-models)
  such as Aura for block authoring and GRANDPA for finality.


### Runtime

In Substrate, the terms "runtime" and "state transition function" are analogous.
Both terms refer to the core logic of the blockchain that is responsible for
validating blocks and executing the state changes they define. The Substrate
project in this repository uses
[FRAME](https://docs.substrate.io/learn/runtime-development/#frame) to construct
a blockchain runtime. FRAME allows runtime developers to declare domain-specific
logic in modules called "pallets". At the heart of FRAME is a helpful [macro
language](https://docs.substrate.io/reference/frame-macros/) that makes it easy
to create pallets and flexibly compose them to create blockchains that can
address [a variety of needs](https://substrate.io/ecosystem/projects/).

Review the [FRAME runtime implementation](./runtime/src/lib.rs) included in this
project and note the following:

- This file configures several pallets to include in the runtime. Each pallet
  configuration is defined by a code block that begins with `impl
  $PALLET_NAME::Config for Runtime`.
- The pallets are composed into a single runtime by way of the
  [#[runtime]](https://paritytech.github.io/polkadot-sdk/master/frame_support/attr.runtime.html)
  macro, which is part of the [core FRAME pallet
  library](https://docs.substrate.io/reference/frame-pallets/#system-pallets).

### Pallets

The runtime in this project is constructed using many FRAME pallets that ship
with [the Substrate
repository](https://github.com/paritytech/polkadot-sdk/tree/master/substrate/frame) and a
custom `pallet-xode` that is [defined in the
`pallets`](./pallets/xode/src/lib.rs) directory.

A FRAME pallet is comprised of a number of blockchain primitives, including:

- Storage: FRAME defines a rich set of powerful [storage
  abstractions](https://docs.substrate.io/build/runtime-storage/) that makes it
  easy to use Substrate's efficient key-value database to manage the evolving
  state of a blockchain.
- Dispatchables: FRAME pallets define special types of functions that can be
  invoked (dispatched) from outside of the runtime in order to update its state.
- Events: Substrate uses
  [events](https://docs.substrate.io/build/events-and-errors/) to notify users
  of significant state changes.
- Errors: When a dispatchable fails, it returns an error.

Each pallet has its own `Config` trait which serves as a configuration interface
to generically define the types and parameters it depends on.

## Alternatives Installations

Instead of installing dependencies and building this source directly, consider
the following alternatives.

### Nix

Install [nix](https://nixos.org/) and
[nix-direnv](https://github.com/nix-community/nix-direnv) for a fully
plug-and-play experience for setting up the development environment. To get all
the correct dependencies, activate direnv `direnv allow`.

### Docker

Please follow the [Substrate Docker instructions
here](https://github.com/paritytech/polkadot-sdk/blob/master/substrate/docker/README.md) to
build the Docker container with the Xode Solochain node binary.
