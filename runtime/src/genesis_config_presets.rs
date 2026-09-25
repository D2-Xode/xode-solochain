// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	AccountId, BalancesConfig, RuntimeGenesisConfig, SessionKeys, SudoConfig,
	TechnicalCommitteeMembershipConfig, TreasuryCouncilMembershipConfig,
};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
	root: AccountId,
	technical_committee: Vec<AccountId>,
	treasury_council: Vec<AccountId>,
) -> Value {
	build_struct_json_patch!(RuntimeGenesisConfig {
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1u128 << 60))
				.collect::<Vec<_>>(),
		},
		// Aura's and GRANDPA's authority sets are *not* set directly here — `pallet_session`'s
		// own genesis build seeds them from `session.keys` below via its `SessionHandler =
		// (Aura, Grandpa)`. Setting them here too makes Aura/GRANDPA panic at genesis with
		// "Authorities are already initialized!" since both would try to initialize them.
		session: pallet_session::GenesisConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						SessionKeys { aura: x.1.clone(), grandpa: x.2.clone() },
					)
				})
				.collect::<Vec<_>>(),
		},
		xode_staking: pallet_xode_staking::GenesisConfig {
			desired_candidates: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		sudo: SudoConfig { key: Some(root) },
		// Only the membership pallets are seeded; each one initializes its collective's members
		// through `MembershipInitialized`.
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
			members: technical_committee
				.try_into()
				.expect("genesis technical committee must fit within MaxMembers; qed"),
		},
		treasury_council_membership: TreasuryCouncilMembershipConfig {
			members: treasury_council
				.try_into()
				.expect("genesis treasury council must fit within MaxMembers; qed"),
		},
	})
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
	testnet_genesis(
		vec![(
			Sr25519Keyring::Alice.to_account_id(),
			sp_keyring::Sr25519Keyring::Alice.public().into(),
			sp_keyring::Ed25519Keyring::Alice.public().into(),
		)],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::AliceStash.to_account_id(),
			Sr25519Keyring::BobStash.to_account_id(),
		],
		sp_keyring::Sr25519Keyring::Alice.to_account_id(),
		vec![Sr25519Keyring::Alice.to_account_id()],
		vec![Sr25519Keyring::Alice.to_account_id()],
	)
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
	testnet_genesis(
		vec![
			(
				Sr25519Keyring::Alice.to_account_id(),
				sp_keyring::Sr25519Keyring::Alice.public().into(),
				sp_keyring::Ed25519Keyring::Alice.public().into(),
			),
			(
				Sr25519Keyring::Bob.to_account_id(),
				sp_keyring::Sr25519Keyring::Bob.public().into(),
				sp_keyring::Ed25519Keyring::Bob.public().into(),
			),
		],
		Sr25519Keyring::iter()
			.filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
			.map(|v| v.to_account_id())
			.collect::<Vec<_>>(),
		Sr25519Keyring::Alice.to_account_id(),
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::Charlie.to_account_id(),
		],
		vec![
			Sr25519Keyring::Dave.to_account_id(),
			Sr25519Keyring::Eve.to_account_id(),
			Sr25519Keyring::Ferdie.to_account_id(),
		],
	)
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
	]
}
