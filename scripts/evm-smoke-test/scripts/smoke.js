// EVM smoke test for the Xode solochain (pallet-revive + eth-rpc).
//
// Prerequisites: a `--dev` node on ws://127.0.0.1:9944 and `eth-rpc` on http://127.0.0.1:8545.
// Run with: npx hardhat run scripts/smoke.js --network xodeDev
import assert from "node:assert/strict";
import { network } from "hardhat";
import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";

const SUBSTRATE_WS = process.env.SUBSTRATE_WS ?? "ws://127.0.0.1:9944";
// `EVM_CHAIN_ID` in runtime/src/configs/mod.rs.
const EXPECTED_CHAIN_ID = BigInt(process.env.EXPECTED_CHAIN_ID ?? 34170);
// 10^18 (wei per ETH) / 10^12 (plancks per XON), the runtime's `NativeToEthRatio`.
const NATIVE_TO_ETH_RATIO = 10n ** 6n;
// `ERC20<_, InlineIdConfig<0x120>, _>` precompile prefix for the `Assets` pallet.
const ERC20_PREFIX = "0120";
const ASSET_ID = Number(process.env.ASSET_ID ?? 4242);
const ASSET_AMOUNT = 1_000_000n * 10n ** 12n;

const ok = (msg) => console.log(`  ok  ${msg}`);

/** The `AccountId32` pallet-revive maps an Ethereum address to: the address followed by 12 0xEE bytes. */
const fallbackAccountId = (address) => address.toLowerCase() + "ee".repeat(12);

/** Address of the ERC20 precompile for `assetId`: `[id: 4 bytes BE] ++ 12 zero bytes ++ prefix ++ 0x0000`. */
const erc20PrecompileAddress = (ethers, assetId) =>
	ethers.getAddress(
		"0x" + assetId.toString(16).padStart(8, "0") + "0".repeat(24) + ERC20_PREFIX + "0000",
	);

/** Submit `tx` signed by `signer` and resolve once it is in a block, failing on dispatch errors. */
const submit = (api, tx, signer) =>
	new Promise((resolve, reject) => {
		tx.signAndSend(signer, ({ status, dispatchError }) => {
			if (dispatchError) {
				const detail = dispatchError.isModule
					? JSON.stringify(api.registry.findMetaError(dispatchError.asModule))
					: dispatchError.toString();
				reject(new Error(`extrinsic failed: ${detail}`));
			} else if (status.isInBlock) {
				resolve(status.asInBlock.toHex());
			}
		}).catch(reject);
	});

const { ethers } = await network.create();
const [signer] = await ethers.getSigners();
const api = await ApiPromise.create({ provider: new WsProvider(SUBSTRATE_WS), noInitWarn: true });

try {
	console.log(`signer ${signer.address} (Substrate account ${fallbackAccountId(signer.address)})`);

	// 1. eth_chainId and eth_getBalance.
	const chainId = BigInt(await ethers.provider.send("eth_chainId", []));
	ok(`eth_chainId = ${chainId}`);
	assert.equal(chainId, EXPECTED_CHAIN_ID, "unexpected chain id");

	const ethBalance = await ethers.provider.getBalance(signer.address);
	const { data } = await api.query.system.account(fallbackAccountId(signer.address));
	const free = data.free.toBigInt();
	const reserved = data.reserved.toBigInt();
	const frozen = data.frozen.toBigInt();
	const ed = api.consts.balances.existentialDeposit.toBigInt();
	// pallet-revive reports `reducible_balance(Preserve, Polite)`: the free balance minus whatever
	// must stay put, i.e. max(frozen - reserved, existential deposit).
	const locked = frozen > reserved ? frozen - reserved : 0n;
	const untouchable = locked > ed ? locked : ed;
	console.log(`     Substrate free = ${free}, free x 10^6 = ${free * NATIVE_TO_ETH_RATIO}`);
	console.log(`     eth_getBalance = ${ethBalance}`);
	assert.equal(ethBalance, (free - untouchable) * NATIVE_TO_ETH_RATIO, "balance mismatch");
	ok(`eth_getBalance = (free - ${untouchable} untouchable) x 10^6`);

	// 2. Deploy a solc-compiled (EVM bytecode) contract.
	const counter = await ethers.deployContract("Counter");
	await counter.waitForDeployment();
	const code = await ethers.provider.getCode(await counter.getAddress());
	assert.ok(code.length > 2, "no code at the deployed address");
	ok(`Counter deployed at ${await counter.getAddress()}`);

	// 3. State-changing call, then read it back.
	const before = await counter.number();
	const receipt = await (await counter.increment()).wait();
	assert.equal(receipt.status, 1, "increment() reverted");
	const after = await counter.number();
	assert.equal(after, before + 1n, "number() did not increase");
	ok(`increment() in block ${receipt.blockNumber}: number ${before} -> ${after}`);

	// 4. ERC20 precompile over an asset created in pallet-assets.
	const alice = new Keyring({ type: "sr25519" }).addFromUri("//Alice");
	const existing = await api.query.assets.asset(ASSET_ID);
	const calls = existing.isSome ? [] : [api.tx.assets.create(ASSET_ID, alice.address, 1)];
	calls.push(api.tx.assets.mint(ASSET_ID, fallbackAccountId(signer.address), ASSET_AMOUNT));
	await submit(api, api.tx.utility.batchAll(calls), alice);
	const account = await api.query.assets.account(ASSET_ID, fallbackAccountId(signer.address));
	const assetBalance = account.unwrap().balance.toBigInt();

	const erc20 = new ethers.Contract(
		erc20PrecompileAddress(ethers, ASSET_ID),
		["function balanceOf(address) view returns (uint256)", "function totalSupply() view returns (uint256)"],
		ethers.provider,
	);
	const viaPrecompile = await erc20.balanceOf(signer.address);
	assert.equal(viaPrecompile, assetBalance, "ERC20 balanceOf does not match pallet-assets");
	ok(`ERC20 ${await erc20.getAddress()} balanceOf = ${viaPrecompile} (pallet-assets: ${assetBalance})`);

	console.log("EVM smoke test passed");
} finally {
	await api.disconnect();
}
