# EVM smoke test

An end-to-end check of the chain's EVM support, written with **Hardhat 3** (ethers.js) and
`@polkadot/api`. It needs a local `--dev` node and `eth-rpc` (see the main README):

```sh
./target/release/xode-solochain-node --dev          # ws://127.0.0.1:9944
./eth-rpc --dev --node-rpc-url ws://127.0.0.1:9944  # http://127.0.0.1:8545

cd scripts/evm-smoke-test
npm ci
npm run smoke
```

It signs with Alith, one of the pre-funded Ethereum dev accounts, and checks that:

1. `eth_chainId` is the runtime's `EVM_CHAIN_ID` (34170), and `eth_getBalance` equals the
   account's Substrate balance times 10^6. That's the balance it can spend: free minus the
   existential deposit, or minus frozen funds if more.
2. `Counter.sol`, compiled by standard `solc` 0.8.28 to EVM bytecode, deploys.
3. `increment()` changes state and `number()` reads the new value back.
4. The ERC20 precompile reports the same `balanceOf` as `pallet-assets` for an asset that Alice
   (Substrate) creates and mints to Alith. The asset id is 4242, or `ASSET_ID`.

Environment overrides: `ETH_RPC_URL`, `SUBSTRATE_WS`, `PRIVATE_KEY`, `EXPECTED_CHAIN_ID`,
`ASSET_ID`.
