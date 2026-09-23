use sc_service::ChainType;
use xode_solochain_runtime::WASM_BINARY;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

/// Chain properties matching the live Xode network (tokenSymbol, tokenDecimals, ss58Format).
fn xode_properties() -> sc_service::Properties {
	serde_json::json!({
		"tokenSymbol": "XON",
		"tokenDecimals": 12,
		"ss58Format": 280,
	})
	.as_object()
	.expect("object literal is a valid map")
	.clone()
}

pub fn development_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Xode Development")
	.with_id("xode_dev")
	.with_chain_type(ChainType::Development)
	.with_properties(xode_properties())
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.build())
}

pub fn local_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Xode Local Testnet")
	.with_id("xode_local_testnet")
	.with_chain_type(ChainType::Local)
	.with_properties(xode_properties())
	.with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
	.build())
}
