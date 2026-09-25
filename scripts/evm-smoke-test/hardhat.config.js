import { defineConfig } from "hardhat/config";
import hardhatEthers from "@nomicfoundation/hardhat-ethers";

// Alith, the first of the well-known Ethereum dev accounts pre-funded by the dev chain spec.
// Public key material: development chains only.
const ALITH = "0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133";

export default defineConfig({
	plugins: [hardhatEthers],
	solidity: { version: "0.8.28" },
	networks: {
		xodeDev: {
			type: "http",
			chainType: "l1",
			url: process.env.ETH_RPC_URL ?? "http://127.0.0.1:8545",
			accounts: [process.env.PRIVATE_KEY ?? ALITH],
		},
	},
});
