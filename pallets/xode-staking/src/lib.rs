//! # Xode Staking Pallet
//!
//! Lets accounts bond XON and register as validator candidates, and lets other accounts
//! delegate (stake) XON behind a candidate to boost its ranking. Each session, the
//! highest-ranked candidates (by `bond + total_stake`) become the chain's Aura/GRANDPA
//! validators via [`pallet_session::SessionManager`].
//!
//! This is a from-scratch reimplementation of the design used by Xode's `pallet-xode-staking`
//! on its live parachain, adapted to this solochain: it drops the parachain-only
//! `pallet_collator_selection` dependency (its `Invulnerables` list is now owned directly by
//! this pallet) and the polkadot-sdk version those parachain pallets pin, which is newer than
//! what this workspace runs. The core concepts — proposed candidates, bonding, delegated
//! stake, stake-ordered waiting list, session-boundary rotation, staleness slashing — are the
//! same.
//!
//! Known gaps versus the parachain version: no reward/fee distribution to authors or
//! delegators yet (see [`pallet::Pallet`]'s `EventHandler::note_author`), and no dedicated
//! `mock.rs`/`tests.rs`/benchmarking suite in this initial port.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod weights;
pub use weights::*;

extern crate alloc;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use alloc::vec::Vec;
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ReservableCurrency},
	};
	use frame_system::pallet_prelude::*;
	use pallet_session::SessionManager;
	use sp_runtime::{traits::Zero, Saturating};
	use sp_staking::SessionIndex;

	pub type BalanceOf<T> =
		<<T as Config>::StakingCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Runtime configuration.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_authorship::Config + pallet_session::Config {
		/// The overarching runtime event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;
		/// The currency that candidates bond and delegators stake.
		type StakingCurrency: ReservableCurrency<Self::AccountId>;
		/// The maximum number of candidates (and thus validators) this pallet will track.
		#[pallet::constant]
		type MaxCandidates: Get<u32>;
		/// The maximum number of delegations a single candidate can have.
		#[pallet::constant]
		type MaxDelegationsPerCandidate: Get<u32>;
		/// The minimum non-zero bond a candidate must hold to stay registered.
		#[pallet::constant]
		type MinCandidateBond: Get<BalanceOf<Self>>;
		/// How many blocks a validator may go without authoring before it's marked offline.
		#[pallet::constant]
		type MaxStalingPeriod: Get<BlockNumberFor<Self>>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// A candidate's place in the validator-selection pipeline.
	#[derive(
		PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen, PartialOrd, Default,
	)]
	pub enum Status {
		#[default]
		Offline,
		Online,
		Waiting,
		Queuing,
		Authoring,
	}

	/// A registered validator candidate.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct CandidateInfo<AccountId, Balance, BlockNumber> {
		pub who: AccountId,
		pub bond: Balance,
		pub total_stake: Balance,
		pub last_updated: BlockNumber,
		pub last_authored: BlockNumber,
		pub leaving: bool,
		pub offline: bool,
		pub commission: u8,
		pub status: Status,
	}

	/// A delegator's stake behind one candidate.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Delegation<AccountId, Balance> {
		pub delegator: AccountId,
		pub stake: Balance,
	}

	/// The current validator set, returned to `pallet_session` at each session boundary.
	#[pallet::storage]
	pub type Invulnerables<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Accounts that are always kept in the waiting list and exempt from staleness slashing,
	/// regardless of stake (set once at genesis).
	#[pallet::storage]
	pub type DesiredCandidates<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// All registered candidates, kept sorted by `bond + total_stake` descending.
	#[pallet::storage]
	pub type ProposedCandidates<T: Config> = StorageValue<
		_,
		BoundedVec<CandidateInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>, T::MaxCandidates>,
		ValueQuery,
	>;

	/// Candidates lined up to become validators (`Invulnerables`) at the next session.
	#[pallet::storage]
	pub type WaitingCandidates<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Delegated stakes per candidate.
	#[pallet::storage]
	pub type Delegations<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<Delegation<T::AccountId, BalanceOf<T>>, T::MaxDelegationsPerCandidate>,
		OptionQuery,
	>;

	/// Validators that have authored at least one block in the current session; reset every
	/// session boundary.
	#[pallet::storage]
	pub type ActualAuthors<T: Config> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		/// Accounts that are validators from genesis onward (see [`DesiredCandidates`]).
		pub desired_candidates: Vec<T::AccountId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let bounded: BoundedVec<T::AccountId, T::MaxCandidates> = self
				.desired_candidates
				.clone()
				.try_into()
				.expect("genesis desired_candidates must fit within MaxCandidates; qed");
			DesiredCandidates::<T>::put(bounded.clone());
			Invulnerables::<T>::put(bounded.clone());
			WaitingCandidates::<T>::put(bounded);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CandidateRegistered { who: T::AccountId },
		CandidateBonded { who: T::AccountId, bond: BalanceOf<T> },
		CandidateBondCorrected { who: T::AccountId },
		CandidateCommissionSet { who: T::AccountId, commission: u8 },
		CandidateWentOffline { who: T::AccountId },
		CandidateWentOnline { who: T::AccountId },
		CandidateLeft { who: T::AccountId },
		DelegationAdded { delegator: T::AccountId, candidate: T::AccountId, stake: BalanceOf<T> },
		DelegationRevoked { delegator: T::AccountId, candidate: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		CandidateAlreadyExists,
		CandidateMaxExceeded,
		CandidateNotFound,
		InvalidCommission,
		InsufficientBalance,
		InsufficientBond,
		StillOnline,
		StillWaiting,
		StillQueuing,
		StillAuthoring,
		DelegationToSelfNotAllowed,
		DelegationInsufficientBalance,
		CandidateDoesNotExist,
		DelegationNotFound,
		NoDelegationsForCandidate,
		DelegationsMaxExceeded,
		WaitingCandidatesEmpty,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			if let Some(author) = pallet_authorship::Pallet::<T>::author() {
				Self::record_authored_block(author.clone());
				Self::add_actual_author(author);
			}
			T::DbWeight::get().reads_writes(2, 2)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register the caller as a validator candidate (starts unbonded and un-staked).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register_candidate())]
		pub fn register_candidate(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				ensure!(!candidates.iter().any(|c| c.who == who), Error::<T>::CandidateAlreadyExists);
				let now = frame_system::Pallet::<T>::block_number();
				let info = CandidateInfo {
					who: who.clone(),
					bond: Zero::zero(),
					total_stake: Zero::zero(),
					last_updated: now,
					last_authored: now,
					leaving: false,
					offline: false,
					commission: 0,
					status: Status::Online,
				};
				candidates.try_push(info).map_err(|_| Error::<T>::CandidateMaxExceeded)?;
				Ok(())
			})?;
			Self::deposit_event(Event::CandidateRegistered { who });
			Ok(())
		}

		/// Set the caller's bond. Setting it to zero signals the candidate is leaving; a
		/// non-zero bond must be at least [`Config::MinCandidateBond`].
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::bond_candidate())]
		pub fn bond_candidate(origin: OriginFor<T>, new_bond: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if new_bond.is_zero() {
				ensure!(!WaitingCandidates::<T>::get().contains(&who), Error::<T>::StillWaiting);
				ensure!(!Invulnerables::<T>::get().contains(&who), Error::<T>::StillQueuing);
				ensure!(!Self::is_current_validator(&who), Error::<T>::StillAuthoring);
			} else {
				ensure!(new_bond >= T::MinCandidateBond::get(), Error::<T>::InsufficientBond);
			}

			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let candidate =
					candidates.iter_mut().find(|c| c.who == who).ok_or(Error::<T>::CandidateNotFound)?;
				if new_bond > candidate.bond {
					let diff = new_bond.saturating_sub(candidate.bond);
					ensure!(T::StakingCurrency::free_balance(&who) >= diff, Error::<T>::InsufficientBalance);
					T::StakingCurrency::reserve(&who, diff)?;
				} else if new_bond < candidate.bond {
					T::StakingCurrency::unreserve(&who, candidate.bond.saturating_sub(new_bond));
				}
				candidate.bond = new_bond;
				candidate.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;

			Self::sort_proposed_candidates();
			Self::deposit_event(Event::CandidateBonded { who, bond: new_bond });
			Ok(())
		}

		/// Set the commission (1-100) the candidate intends to charge its delegators.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_commission_of_candidate())]
		pub fn set_commission_of_candidate(origin: OriginFor<T>, commission: u8) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!((1..=100).contains(&commission), Error::<T>::InvalidCommission);
			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let candidate =
					candidates.iter_mut().find(|c| c.who == who).ok_or(Error::<T>::CandidateNotFound)?;
				candidate.commission = commission;
				candidate.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;
			Self::deposit_event(Event::CandidateCommissionSet { who, commission });
			Ok(())
		}

		/// Delegate (stake) `amount` behind `candidate`, boosting its ranking.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::stake_candidate())]
		pub fn stake_candidate(
			origin: OriginFor<T>,
			candidate: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who != candidate, Error::<T>::DelegationToSelfNotAllowed);
			ensure!(
				ProposedCandidates::<T>::get().iter().any(|c| c.who == candidate),
				Error::<T>::CandidateDoesNotExist
			);
			ensure!(
				T::StakingCurrency::free_balance(&who) >= amount,
				Error::<T>::DelegationInsufficientBalance
			);

			T::StakingCurrency::reserve(&who, amount)?;

			Delegations::<T>::try_mutate(&candidate, |maybe_delegations| -> DispatchResult {
				let delegations = maybe_delegations.get_or_insert_with(Default::default);
				if let Some(delegation) = delegations.iter_mut().find(|d| d.delegator == who) {
					delegation.stake = delegation.stake.saturating_add(amount);
				} else {
					delegations
						.try_push(Delegation { delegator: who.clone(), stake: amount })
						.map_err(|_| Error::<T>::DelegationsMaxExceeded)?;
				}
				Ok(())
			})?;

			Self::recompute_total_stake(&candidate)?;
			Self::deposit_event(Event::DelegationAdded { delegator: who, candidate, stake: amount });
			Ok(())
		}

		/// Withdraw the caller's delegation behind `candidate`.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::unstake_candidate())]
		pub fn unstake_candidate(origin: OriginFor<T>, candidate: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let stake_amount = Delegations::<T>::try_mutate_exists(
				&candidate,
				|maybe_delegations| -> Result<BalanceOf<T>, DispatchError> {
					let delegations =
						maybe_delegations.as_mut().ok_or(Error::<T>::NoDelegationsForCandidate)?;
					let position = delegations
						.iter()
						.position(|d| d.delegator == who)
						.ok_or(Error::<T>::DelegationNotFound)?;
					let stake = delegations[position].stake;
					delegations.remove(position);
					if delegations.is_empty() {
						*maybe_delegations = None;
					}
					Ok(stake)
				},
			)?;

			T::StakingCurrency::unreserve(&who, stake_amount);

			Self::recompute_total_stake(&candidate)?;
			Self::deposit_event(Event::DelegationRevoked { delegator: who, candidate });
			Ok(())
		}

		/// Temporarily step down without un-bonding or un-staking.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::offline_candidate())]
		pub fn offline_candidate(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::set_offline(&who, true)?;
			Self::sort_proposed_candidates();
			Self::deposit_event(Event::CandidateWentOffline { who });
			Ok(())
		}

		/// Come back online after [`Self::offline_candidate`].
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::online_candidate())]
		pub fn online_candidate(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::set_offline(&who, false)?;
			Self::sort_proposed_candidates();
			Self::deposit_event(Event::CandidateWentOnline { who });
			Ok(())
		}

		/// Leave candidacy for good. The candidate must already be offline, not waiting/queued,
		/// and not a current validator.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::leave_candidate())]
		pub fn leave_candidate(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let candidate =
					candidates.iter_mut().find(|c| c.who == who).ok_or(Error::<T>::CandidateNotFound)?;
				ensure!(candidate.offline, Error::<T>::StillOnline);
				ensure!(!WaitingCandidates::<T>::get().contains(&who), Error::<T>::StillWaiting);
				ensure!(!Invulnerables::<T>::get().contains(&who), Error::<T>::StillQueuing);
				ensure!(!Self::is_current_validator(&who), Error::<T>::StillAuthoring);
				candidate.leaving = true;
				candidate.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;
			Self::remove_from_waiting(&who);
			Self::deposit_event(Event::CandidateLeft { who });
			Ok(())
		}

		/// Release a stuck reserve for an offline, non-waiting/queuing/authoring candidate and
		/// zero out its bond.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::bond_correction())]
		pub fn bond_correction(origin: OriginFor<T>, frozen_balance: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let candidate =
					candidates.iter_mut().find(|c| c.who == who).ok_or(Error::<T>::CandidateNotFound)?;
				ensure!(candidate.offline, Error::<T>::StillOnline);
				ensure!(!WaitingCandidates::<T>::get().contains(&who), Error::<T>::StillWaiting);
				ensure!(!Invulnerables::<T>::get().contains(&who), Error::<T>::StillQueuing);
				ensure!(!Self::is_current_validator(&who), Error::<T>::StillAuthoring);
				T::StakingCurrency::unreserve(&who, frozen_balance);
				candidate.bond = Zero::zero();
				candidate.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;
			Self::deposit_event(Event::CandidateBondCorrected { who });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn set_offline(who: &T::AccountId, offline: bool) -> DispatchResult {
			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let candidate =
					candidates.iter_mut().find(|c| &c.who == who).ok_or(Error::<T>::CandidateNotFound)?;
				candidate.offline = offline;
				candidate.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})
		}

		/// `pallet_session::Config::ValidatorId` is, in general, a different associated type
		/// from `frame_system::Config::AccountId` — this runtime happens to set them equal, but
		/// the pallet can't assume that generically. Both encode identically whenever they *are*
		/// the same underlying type, so round-tripping through SCALE encoding recovers the
		/// `AccountId`, the same trick the original Xode parachain pallet used.
		fn validator_to_account(validator: T::ValidatorId) -> T::AccountId {
			let bytes = validator.encode();
			T::AccountId::decode(&mut bytes.as_slice())
				.expect("ValidatorId and AccountId share the same encoding in this runtime; qed")
		}

		fn current_validators() -> Vec<T::AccountId> {
			pallet_session::Validators::<T>::get()
				.into_iter()
				.map(Self::validator_to_account)
				.collect()
		}

		fn is_current_validator(who: &T::AccountId) -> bool {
			Self::current_validators().contains(who)
		}

		fn remove_from_waiting(who: &T::AccountId) {
			WaitingCandidates::<T>::mutate(|waiting| waiting.retain(|w| w != who));
		}

		fn set_status(who: &T::AccountId, status: Status) {
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| &c.who == who) {
					candidate.status = status;
				}
			});
		}

		/// Highest `bond + total_stake` first, offline candidates last, ties broken by whoever
		/// was updated longest ago.
		fn sort_proposed_candidates() {
			ProposedCandidates::<T>::mutate(|candidates| {
				candidates.sort_by(|a, b| {
					a.offline
						.cmp(&b.offline)
						.then_with(|| {
							let a_combined = a.bond.saturating_add(a.total_stake);
							let b_combined = b.bond.saturating_add(b.total_stake);
							b_combined.cmp(&a_combined)
						})
						.then_with(|| a.last_updated.cmp(&b.last_updated))
				});
			});
		}

		fn recompute_total_stake(candidate: &T::AccountId) -> DispatchResult {
			let total_stake = Delegations::<T>::get(candidate)
				.map(|delegations| {
					delegations.iter().fold(BalanceOf::<T>::zero(), |acc, d| acc.saturating_add(d.stake))
				})
				.unwrap_or_else(Zero::zero);

			ProposedCandidates::<T>::try_mutate(|candidates| -> DispatchResult {
				let c = candidates
					.iter_mut()
					.find(|c| &c.who == candidate)
					.ok_or(Error::<T>::CandidateNotFound)?;
				c.total_stake = total_stake;
				c.last_updated = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;
			Self::sort_proposed_candidates();
			Ok(())
		}

		fn record_authored_block(who: T::AccountId) {
			ProposedCandidates::<T>::mutate(|candidates| {
				if let Some(candidate) = candidates.iter_mut().find(|c| c.who == who) {
					candidate.last_authored = frame_system::Pallet::<T>::block_number();
				}
			});
		}

		fn add_actual_author(who: T::AccountId) {
			ActualAuthors::<T>::mutate(|authors| {
				if !authors.contains(&who) {
					// Best-effort: if the bounded list is somehow full, we simply don't record
					// this author for staleness-exemption purposes this session.
					let _ = authors.try_push(who);
				}
			});
		}

		/// Move the waiting list into the validator set for the next session.
		fn queue_authors() -> DispatchResult {
			let waiting = WaitingCandidates::<T>::get();
			ensure!(!waiting.is_empty(), Error::<T>::WaitingCandidatesEmpty);

			ProposedCandidates::<T>::mutate(|candidates| {
				for candidate in candidates.iter_mut() {
					if candidate.status == Status::Waiting && waiting.contains(&candidate.who) {
						candidate.status = Status::Queuing;
					}
				}
			});

			Invulnerables::<T>::put(waiting);
			Ok(())
		}

		/// Mark validators that didn't author within `MaxStalingPeriod` as offline, exempting
		/// the permanently-desired genesis validators.
		fn slash_stale_authors() {
			let validators = Self::current_validators();
			let authors = ActualAuthors::<T>::get();
			let desired = DesiredCandidates::<T>::get();
			let now = frame_system::Pallet::<T>::block_number();
			let max_staling_period = T::MaxStalingPeriod::get();

			let stale: Vec<T::AccountId> = validators
				.into_iter()
				.filter(|v| !authors.contains(v) && !desired.contains(v))
				.collect();

			if !stale.is_empty() {
				let mut changed = false;
				ProposedCandidates::<T>::mutate(|candidates| {
					for candidate in candidates.iter_mut() {
						if stale.contains(&candidate.who)
							&& now.saturating_sub(candidate.last_authored) > max_staling_period
						{
							candidate.offline = true;
							candidate.last_updated = now;
							changed = true;
						}
					}
				});
				if changed {
					Self::sort_proposed_candidates();
				}
			}

			ActualAuthors::<T>::kill();
		}

		/// Drop leaving/offline/unbonded candidates from the pipeline and downgrade everyone
		/// else's status one step, ready to be re-waitlisted.
		fn prepare_next_waiting_list() {
			let candidates = ProposedCandidates::<T>::get();
			for candidate in candidates.iter() {
				if candidate.offline || candidate.leaving || candidate.bond.is_zero() {
					match candidate.status {
						Status::Online | Status::Offline => {
							let who = candidate.who.clone();
							ProposedCandidates::<T>::mutate(|c| c.retain(|x| x.who != who));
						},
						Status::Waiting => {
							Self::remove_from_waiting(&candidate.who);
							let who = candidate.who.clone();
							ProposedCandidates::<T>::mutate(|c| c.retain(|x| x.who != who));
						},
						Status::Queuing => Self::set_status(&candidate.who, Status::Waiting),
						Status::Authoring => Self::set_status(&candidate.who, Status::Queuing),
					}
				}
			}
		}

		/// Rebuild the waiting list for the next session: desired candidates first, then
		/// bonded/online/non-leaving proposed candidates in stake order, up to `MaxCandidates`.
		fn refresh_waiting_list() {
			let desired = DesiredCandidates::<T>::get();
			let proposed = ProposedCandidates::<T>::get();
			let mut waiting: BoundedVec<T::AccountId, T::MaxCandidates> = BoundedVec::default();

			for candidate in desired.iter() {
				if waiting.try_push(candidate.clone()).is_err() {
					break;
				}
			}

			for candidate in proposed.iter() {
				if waiting.len() >= T::MaxCandidates::get() as usize {
					break;
				}
				if !waiting.contains(&candidate.who)
					&& !candidate.bond.is_zero()
					&& !candidate.leaving
					&& !candidate.offline
				{
					if waiting.try_push(candidate.who.clone()).is_err() {
						break;
					}
					if candidate.status == Status::Online {
						Self::set_status(&candidate.who, Status::Waiting);
					}
				}
			}

			WaitingCandidates::<T>::put(waiting);
		}
	}

	impl<T: Config> SessionManager<T::AccountId> for Pallet<T> {
		fn new_session(_index: SessionIndex) -> Option<Vec<T::AccountId>> {
			let authors = Invulnerables::<T>::get().to_vec();
			for author in &authors {
				Self::set_status(author, Status::Authoring);
			}
			Some(authors)
		}

		fn start_session(_index: SessionIndex) {}

		fn end_session(_index: SessionIndex) {
			let _ = Self::queue_authors();
			Self::slash_stale_authors();
			Self::prepare_next_waiting_list();
			Self::refresh_waiting_list();
		}
	}

	impl<T: Config> pallet_authorship::EventHandler<T::AccountId, BlockNumberFor<T>> for Pallet<T> {
		fn note_author(_author: T::AccountId) {
			// Reward/fee distribution to the author (split by `commission` with its
			// delegators) is not implemented yet.
		}
	}
}
