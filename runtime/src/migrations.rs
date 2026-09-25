//! Runtime-level migrations, see `Migrations` in `lib.rs`.

use frame_support::{
	traits::{
		fungible::{Inspect, Mutate},
		OnRuntimeUpgrade,
	},
	weights::Weight,
};

use crate::{Balances, Runtime};

/// Creates `pallet_revive`'s own account (`py/reviv`) with the existential deposit, if it doesn't
/// exist yet.
///
/// The pallet moves storage deposits into this account and places them on hold there, which only
/// works if the account already exists with at least the existential deposit free: without it,
/// uploading the first contract fails with `StorageDepositNotEnoughFunds`. `pallet_revive`'s
/// genesis build creates the account the same way, but genesis doesn't run when the pallet is
/// added to a live chain by a runtime upgrade.
///
/// Does nothing if the account exists, so it is harmless to keep. It can be removed once every
/// live chain has been upgraded past it.
pub struct CreateRevivePalletAccount;

impl OnRuntimeUpgrade for CreateRevivePalletAccount {
	fn on_runtime_upgrade() -> Weight {
		let db = <Runtime as frame_system::Config>::DbWeight::get();
		let account = pallet_revive::Pallet::<Runtime>::account_id();
		if frame_system::Pallet::<Runtime>::account_exists(&account) {
			return db.reads(1);
		}
		let minimum_balance = <Balances as Inspect<_>>::minimum_balance();
		if <Balances as Mutate<_>>::mint_into(&account, minimum_balance).is_err() {
			frame_support::defensive!("failed to create the revive pallet account");
		}
		// `account_exists`, then `mint_into` updates the account and the total issuance.
		db.reads_writes(2, 2)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		frame_support::ensure!(
			frame_system::Pallet::<Runtime>::account_exists(&pallet_revive::Pallet::<Runtime>::account_id()),
			"the revive pallet account does not exist"
		);
		Ok(())
	}
}
