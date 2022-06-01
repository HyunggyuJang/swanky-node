#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::{Decode, Encode};
use pallet_contracts::weights::WeightInfo;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{AccountIdLookup, BlakeTwo256, Block as BlockT, IdentifyAccount, Verify},
	transaction_validity::{TransactionSource, TransactionValidity},
	AccountId32, ApplyExtrinsicResult, MultiSignature, RuntimeDebug,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
pub use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU128, ConstU32, ConstU8, KeyOwnerProofSystem, Randomness, StorageInfo},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		IdentityFee, Weight,
	},
	PalletId, StorageValue,
};
use frame_support::{
	log::{error, trace},
	pallet_prelude::MaxEncodedLen,
};
pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::CurrencyAdapter;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Permill};

use pallet_contracts::chain_extension::{
	ChainExtension, Environment, Ext, InitState, RetVal, SysConfig, UncheckedFrom,
};

use sp_runtime::{ArithmeticError, DispatchError, TokenError};

use sp_runtime::MultiAddress;
pub struct PalletAssetsExtention;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;

	impl_opaque_keys! {
		pub struct SessionKeys {}
	}
}

mod weights;

// To learn more about runtime versioning and what each of the following value means:
//   https://docs.substrate.io/v3/runtime/upgrades#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("swanky-node"),
	impl_name: create_runtime_str!("swanky-node"),
	authoring_version: 1,
	// The version of the runtime specification. A full node will not attempt to use its native
	//   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
	//   `spec_version`, and `authoring_version` are the same between Wasm and native.
	// This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
	//   the compatible custom types.
	spec_version: 100,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// Id used for identifying assets.
///
/// AssetId allocation:
/// [1; 2^32-1]     Custom user assets (permissionless)
/// [2^32; 2^64-1]  Statemine assets (simple map)
/// [2^64; 2^128-1] Ecosystem assets
/// 2^128-1         Relay chain token (KSM)
pub type AssetId = u128;

// Prints debug output of the `contracts` pallet to stdout if the node is
// started with `-lruntime::contracts=debug`.
pub const CONTRACTS_DEBUG_OUTPUT: bool = true;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);

/// Constant values used within the runtime.
pub const MILLIUNIT: Balance = 1_000_000_000_000_000;
pub const UNIT: Balance = 1_000 * MILLIUNIT;

/// Charge fee for stored bytes and items.
pub const fn deposit(items: u32, bytes: u32) -> Balance {
	(items as Balance + bytes as Balance) * MILLIUNIT / 1_000_000
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub const BlockHashCount: BlockNumber = 2400;
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
		::with_sensible_defaults(2 * WEIGHT_PER_SECOND, NORMAL_DISPATCH_RATIO);
	pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
		::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = frame_support::traits::Everything;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = BlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = BlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The set code logic, just the default since we're not a parachain.
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<500>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const AssetDeposit: Balance = 1_000_000;
	pub const ApprovalDeposit: Balance = 1_000_000;
	pub const AssetsStringLimit: u32 = 50;
	/// Key = 32 bytes, Value = 36 bytes (32+1+1+1+1)
	// https://github.com/paritytech/substrate/blob/069917b/frame/assets/src/lib.rs#L257L271
	pub const MetadataDepositBase: Balance = deposit(1, 68);
	pub const MetadataDepositPerByte: Balance = deposit(0, 1);
	pub const AssetAccountDeposit: Balance = deposit(1, 18);
}

impl pallet_assets::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type AssetId = AssetId;
	type Currency = Balances;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type AssetAccountDeposit = AssetAccountDeposit;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = AssetsStringLimit;
	type Freezer = ();
	type Extra = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
}

impl pallet_transaction_payment::Config for Runtime {
	type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}

impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}

// contracts stuffs.
parameter_types! {
	pub const DepositPerItem: Balance = deposit(1, 0);
	pub const DepositPerByte: Balance = deposit(0, 1);
	// The lazy deletion runs inside on_initialize.
	pub DeletionWeightLimit: Weight = AVERAGE_ON_INITIALIZE_RATIO *
		BlockWeights::get().max_block;
	// The weight needed for decoding the queue should be less or equal than a fifth
	// of the overall weight dedicated to the lazy deletion.
	pub DeletionQueueDepth: u32 = ((DeletionWeightLimit::get() / (
			<Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(1) -
			<Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(0)
		)) / 5) as u32;
	pub Schedule: pallet_contracts::Schedule<Runtime> = Default::default();
}

impl pallet_contracts::Config for Runtime {
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Currency = Balances;
	type Event = Event;
	type Call = Call;
	/// The safest default is to allow no calls at all.
	///
	/// Runtimes should whitelist dispatchables that are allowed to be called from contracts
	/// and make sure they are stable. Dispatchables exposed to contracts are not allowed to
	/// change because that would break already deployed contracts. The `Call` structure itself
	/// is not allowed to change the indices of existing pallets, too.
	type CallFilter = frame_support::traits::Nothing;
	type DepositPerItem = DepositPerItem;
	type DepositPerByte = DepositPerByte;
	type WeightPrice = pallet_transaction_payment::Pallet<Self>;
	type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
	type ChainExtension = ();
	type DeletionQueueDepth = DeletionQueueDepth;
	type DeletionWeightLimit = DeletionWeightLimit;
	type Schedule = Schedule;
	type CallStack = [pallet_contracts::Frame<Self>; 31];
	type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
}

parameter_types! {
	pub const DappsStakingPalletId: PalletId = PalletId(*b"py/dpsst");
	pub const BlockPerEra: BlockNumber = 60;
	pub const RegisterDeposit: Balance = 100 * UNIT;
	pub const MaxNumberOfStakersPerContract: u32 = 512;
	pub const MinimumStakingAmount: Balance = 10 * UNIT;
	pub const MinimumRemainingAmount: Balance = 1 * UNIT;
	pub const MaxEraStakeValues: u32 = 5;
	pub const MaxUnlockingChunks: u32 = 2;
	pub const UnbondingPeriod: u32 = 2;
}

impl pallet_dapps_staking::Config for Runtime {
	type Currency = Balances;
	type BlockPerEra = BlockPerEra;
	type SmartContract = SmartContract;
	type RegisterDeposit = RegisterDeposit;
	type Event = Event;
	type WeightInfo = weights::pallet_dapps_staking::WeightInfo<Runtime>;
	type MaxNumberOfStakersPerContract = MaxNumberOfStakersPerContract;
	type MinimumStakingAmount = MinimumStakingAmount;
	type PalletId = DappsStakingPalletId;
	type MaxUnlockingChunks = MaxUnlockingChunks;
	type UnbondingPeriod = UnbondingPeriod;
	type MinimumRemainingAmount = MinimumRemainingAmount;
	type MaxEraStakeValues = MaxEraStakeValues;
}

const DEFAULT_ACCOUNT: AccountId = AccountId32::new([0; 32]);

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub enum SmartContract {
	/// Wasm smart contract instance.
	Wasm(AccountId),
}

impl Default for SmartContract {
	fn default() -> Self {
		SmartContract::Wasm(DEFAULT_ACCOUNT)
	}
}

#[cfg(not(feature = "runtime-benchmarks"))]
impl pallet_dapps_staking::traits::IsContract for SmartContract {
	fn is_valid(&self) -> bool {
		match self {
			// temporarilly no AccountId validation.
			// we want getter function here, so that we can check the existence of contract by
			// AccountId. https://github.com/paritytech/substrate/blob/7a28c62246406839b746af2201309d0ed9a3f526/frame/contracts/src/lib.rs#L792
			SmartContract::Wasm(_account) => true,
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_dapps_staking::traits::IsContract for SmartContract {
	fn is_valid(&self) -> bool {
		match self {
			SmartContract::Wasm(_account) => true,
		}
	}
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system,
		RandomnessCollectiveFlip: pallet_randomness_collective_flip,
		Timestamp: pallet_timestamp,
		Balances: pallet_balances,
		Assets: pallet_assets,
		TransactionPayment: pallet_transaction_payment,
		Sudo: pallet_sudo,
		Contracts: pallet_contracts,
		DappsStaking: pallet_dapps_staking,
	}
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[frame_benchmarking, BaselineBench::<Runtime>]
		[frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_timestamp, Timestamp]
	);
}

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block);
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl pallet_contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash> for Runtime {
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: u64,
			storage_deposit_limit: Option<Balance>,
			input_data: Vec<u8>,
		) -> pallet_contracts_primitives::ContractExecResult<Balance> {
			Contracts::bare_call(origin, dest, value, gas_limit, storage_deposit_limit, input_data, CONTRACTS_DEBUG_OUTPUT)
		}

		fn instantiate(
			origin: AccountId,
			value: Balance,
			gas_limit: u64,
			storage_deposit_limit: Option<Balance>,
			code: pallet_contracts_primitives::Code<Hash>,
			data: Vec<u8>,
			salt: Vec<u8>,
		) -> pallet_contracts_primitives::ContractInstantiateResult<AccountId, Balance>
		{
			Contracts::bare_instantiate(origin, value, gas_limit, storage_deposit_limit, code, data, salt, CONTRACTS_DEBUG_OUTPUT)
		}

		fn upload_code(
			origin: AccountId,
			code: Vec<u8>,
			storage_deposit_limit: Option<Balance>,
		) -> pallet_contracts_primitives::CodeUploadResult<Hash, Balance>
		{
			Contracts::bare_upload_code(origin, code, storage_deposit_limit)
		}

		fn get_storage(
			address: AccountId,
			key: [u8; 32],
		) -> pallet_contracts_primitives::GetStorageResult {
			Contracts::get_storage(address, key)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();

			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch, TrackedStorageKey};

			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			impl frame_system_benchmarking::Config for Runtime {}
			impl baseline::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade() -> (Weight, Weight) {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here. If any of the pre/post migration checks fail, we shall stop
			// right here and right now.
			let weight = Executive::try_runtime_upgrade().unwrap();
			(weight, BlockWeights::get().max_block)
		}

		fn execute_block_no_check(block: Block) -> Weight {
			Executive::execute_block_no_check(block)
		}
	}
}
// struct Origin{}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
enum OriginType {
	Caller,
	Address,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
struct PalletAssetRequest {
	origin_type: OriginType,
	asset_id: u32,
	target_address: [u8; 32],
	amount: u128,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
struct PalletAssetBalanceRequest {
	asset_id: u32,
	address: [u8; 32],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
pub enum PalletAssetErr {
	/// Some error occurred.
	Other,
	/// Failed to lookup some data.
	CannotLookup,
	/// A bad origin.
	BadOrigin,
	/// A custom error in a module.
	Module,
	/// At least one consumer is remaining so the account cannot be destroyed.
	ConsumerRemaining,
	/// There are no providers so the account cannot be created.
	NoProviders,
	/// There are too many consumers so the account cannot be created.
	TooManyConsumers,
	/// An error to do with tokens.
	Token(PalletAssetTokenErr),
	/// An arithmetic error.
	Arithmetic(PalletAssetArithmeticErr),
	//unknown error
	Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
pub enum PalletAssetArithmeticErr {
	/// Underflow.
	Underflow,
	/// Overflow.
	Overflow,
	/// Division by zero.
	DivisionByZero,
	//unknown error
	Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen)]
pub enum PalletAssetTokenErr {
	/// Funds are unavailable.
	NoFunds,
	/// Account that must exist would die.
	WouldDie,
	/// Account cannot exist with the funds that would be given.
	BelowMinimum,
	/// Account cannot be created.
	CannotCreate,
	/// The asset in question is unknown.
	UnknownAsset,
	/// Funds exist but are frozen.
	Frozen,
	/// Operation is not supported by the asset.
	Unsupported,
	//unknown error
	Unknown,
}

impl From<DispatchError> for PalletAssetErr {
	fn from(e: DispatchError) -> Self {
		match e {
			DispatchError::Other(_) => PalletAssetErr::Other,
			DispatchError::CannotLookup => PalletAssetErr::CannotLookup,
			DispatchError::BadOrigin => PalletAssetErr::BadOrigin,
			DispatchError::Module(_) => PalletAssetErr::Module,
			DispatchError::ConsumerRemaining => PalletAssetErr::ConsumerRemaining,
			DispatchError::NoProviders => PalletAssetErr::NoProviders,
			DispatchError::TooManyConsumers => PalletAssetErr::TooManyConsumers,
			DispatchError::Token(token_err) =>
				PalletAssetErr::Token(PalletAssetTokenErr::from(token_err)),
			DispatchError::Arithmetic(arithmetic_error) =>
				PalletAssetErr::Arithmetic(PalletAssetArithmeticErr::from(arithmetic_error)),
			_ => PalletAssetErr::Unknown,
		}
	}
}

impl From<ArithmeticError> for PalletAssetArithmeticErr {
	fn from(e: ArithmeticError) -> Self {
		match e {
			ArithmeticError::Underflow => PalletAssetArithmeticErr::Underflow,
			ArithmeticError::Overflow => PalletAssetArithmeticErr::Overflow,
			ArithmeticError::DivisionByZero => PalletAssetArithmeticErr::DivisionByZero,
			_ => PalletAssetArithmeticErr::Unknown,
		}
	}
}

impl From<TokenError> for PalletAssetTokenErr {
	fn from(e: TokenError) -> Self {
		match e {
			TokenError::NoFunds => PalletAssetTokenErr::NoFunds,
			TokenError::WouldDie => PalletAssetTokenErr::WouldDie,
			TokenError::BelowMinimum => PalletAssetTokenErr::BelowMinimum,
			TokenError::CannotCreate => PalletAssetTokenErr::CannotCreate,
			TokenError::UnknownAsset => PalletAssetTokenErr::UnknownAsset,
			TokenError::Frozen => PalletAssetTokenErr::Frozen,
			TokenError::Unsupported => PalletAssetTokenErr::Unsupported,
			_ => PalletAssetTokenErr::Unknown,
		}
	}
}

impl ChainExtension<Runtime> for PalletAssetsExtention {
	fn call<E: Ext>(
		func_id: u32,
		mut env: Environment<E, InitState>,
	) -> Result<RetVal, DispatchError>
	where
		<E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
	{
		match func_id {
			//create
			1101 => {
				let ext = env.ext();
				let address: &<<E as Ext>::T as SysConfig>::AccountId = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let caller_accountId = AccountId::decode(&mut caller_ref).unwrap();

				use frame_support::dispatch::{DispatchError, DispatchResult};

				let mut address_ref = caller.as_ref();
				let address_account = AccountId::decode(&mut address_ref).unwrap();
				let create_result = pallet_assets::Pallet::<Runtime>::create(
					Origin::signed(caller_accountId.clone()),
					1,
					MultiAddress::Id(address_account.clone()),
					1,
				);
				match create_result {
					DispatchResult::Ok(_) => error!("OK"),
					DispatchResult::Err(e) => error!("{:#?}", e),
				}
				//enum (caller, address_account)
				//asset id
				let mint_result = pallet_assets::Pallet::<Runtime>::mint(
					Origin::signed(caller_accountId),
					1,
					MultiAddress::Id(address_account),
					10,
				);
				match mint_result {
					DispatchResult::Ok(_) => error!("OK"),
					DispatchResult::Err(e) => error!("{:#?}", e),
				}

				let r = pallet_assets::Pallet::<Runtime>::total_supply(1);
				error!("total_supply: {:}", r);
				//return Err(DispatchError::Other("Unimplemented func_id"))
				let mut env = env.buf_in_buf_out();
				let arg: [u8; 32] = env.read_as()?;
				// let random_seed = crate::RandomnessCollectiveFlip::random(&arg).0;
				// let random_slice = random_seed.encode();
				// trace!(
				//     target: "runtime",
				//     "[ChainExtension]|call|func_id:{:}",
				//     func_id
				// );
				env.write(&arg, false, None)
					.map_err(|_| DispatchError::Other("ChainExtension failed to call random"))?;
			},

			1100 => {
				let ext = env.ext();
				let mut env = env.buf_in_buf_out();
				error!("ERROR test");
				let err = Result::<u8, PalletAssetErr>::Err(PalletAssetErr::Other);
				env.write(err.encode().as_ref(), false, None)
					.map_err(|_| DispatchError::Other("ChainExtension failed to call test"))?;
			},

			//create
			1102 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let create_asset: PalletAssetRequest = env.read_as()?;

				let origin_address = match create_asset.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &create_asset.target_address.to_vec()[..];
				let admin_address = AccountId::decode(&mut vec).unwrap();
				let create_result = pallet_assets::Pallet::<Runtime>::create(
					Origin::signed(origin_address),
					create_asset.asset_id.into(),
					MultiAddress::Id(admin_address),
					create_asset.amount,
				);

				error!("create input {:#?}", create_asset);
				error!("create output {:#?}", create_result);
				match create_result {
					DispatchResult::Ok(_) => {
						error!("OK create");
						let err = Result::<(), PalletAssetErr>::Ok(());
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call create")
						})?;
					},
					DispatchResult::Err(e) => {
						error!("ERROR create");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call create")
						})?;
					},
				}
			},

			//mint
			1103 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let mint_asset_request: PalletAssetRequest = env.read_as()?;

				let origin_address = match mint_asset_request.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &mint_asset_request.target_address.to_vec()[..];
				let beneficiary_address = AccountId::decode(&mut vec).unwrap();
				let mint_result = pallet_assets::Pallet::<Runtime>::mint(
					Origin::signed(origin_address),
					mint_asset_request.asset_id.into(),
					MultiAddress::Id(beneficiary_address),
					mint_asset_request.amount,
				);

				error!("mint input {:#?}", mint_asset_request);
				error!("mint output {:#?}", mint_result);
				match mint_result {
					DispatchResult::Ok(_) => {
						error!("OK mint")
					},
					DispatchResult::Err(e) => {
						error!("ERROR mint");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call mint")
						})?;
					},
				}
			},

			//burn
			1104 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let burn_asset_request: PalletAssetRequest = env.read_as()?;

				let origin_address = match burn_asset_request.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &burn_asset_request.target_address.to_vec()[..];
				let who_address = AccountId::decode(&mut vec).unwrap();
				let burn_result = pallet_assets::Pallet::<Runtime>::burn(
					Origin::signed(origin_address),
					burn_asset_request.asset_id.into(),
					MultiAddress::Id(who_address),
					burn_asset_request.amount,
				);

				error!("burn request {:#?}", burn_asset_request);
				error!("burn result {:#?}", burn_result);
				match burn_result {
					DispatchResult::Ok(_) => {
						error!("OK burn")
					},
					DispatchResult::Err(e) => {
						error!("ERROR burn");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call burn")
						})?;
					},
				}
			},

			//transfer
			1105 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let transfer_asset_request: PalletAssetRequest = env.read_as()?;

				let origin_address = match transfer_asset_request.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &transfer_asset_request.target_address.to_vec()[..];
				let target_address = AccountId::decode(&mut vec).unwrap();
				let tranfer_result = pallet_assets::Pallet::<Runtime>::transfer(
					Origin::signed(origin_address),
					transfer_asset_request.asset_id.into(),
					MultiAddress::Id(target_address),
					transfer_asset_request.amount,
				);

				trace!("transfer request {:#?}", transfer_asset_request);
				trace!("transfer result {:#?}", tranfer_result);
				match tranfer_result {
					DispatchResult::Ok(_) => {
						error!("OK transfer");
						/*
						write buffer as responce for smart contract
						let b = [1u8;32];
						env.write(&b, false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call random")
						})?;
						*/
					},
					DispatchResult::Err(e) => {
						error!("ERROR transfer");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call burn")
						})?;
					},
				}
			},

			//balance
			1106 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let balance_asset_request: PalletAssetBalanceRequest = env.read_as()?;

				let mut vec = &balance_asset_request.address.to_vec()[..];
				let balance_of_address = AccountId::decode(&mut vec).unwrap();
				let balance_result: Balance = pallet_assets::Pallet::<Runtime>::balance(
					balance_asset_request.asset_id.into(),
					balance_of_address,
				);

				error!("OK! balance_of : {:#?}", balance_result);
				error!("{:#?}", balance_asset_request);

				let b = balance_result.to_be_bytes();
				//write buffer as responce for smart contract
				env.write(&b, false, None)
					.map_err(|_| DispatchError::Other("ChainExtension failed to call balance"))?;
			},

			//total_supply
			1107 => {
				let ext = env.ext();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let asset_id: u32 = env.read_as()?;

				let total_supply: Balance =
					pallet_assets::Pallet::<Runtime>::total_supply(asset_id.into());

				error!("total_supply : {:#?}", total_supply);
				error!("total_supply asset_id {:#?}", asset_id);

				let b = total_supply.to_be_bytes();
				//write buffer as responce for smart contract
				env.write(&b, false, None).map_err(|_| {
					DispatchError::Other("ChainExtension failed to call total_supply")
				})?;
			},

			//approve_transfer
			1108 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let approve_transfer_request: PalletAssetRequest = env.read_as()?;

				let origin_address = match approve_transfer_request.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &approve_transfer_request.target_address.to_vec()[..];
				let target_address = AccountId::decode(&mut vec).unwrap();
				let approve_transfer_result = pallet_assets::Pallet::<Runtime>::approve_transfer(
					Origin::signed(origin_address),
					approve_transfer_request.asset_id.into(),
					MultiAddress::Id(target_address),
					approve_transfer_request.amount,
				);

				trace!("approve_transfer request {:#?}", approve_transfer_request);
				trace!("approve_transfer result {:#?}", approve_transfer_result);
				match approve_transfer_result {
					DispatchResult::Ok(_) => {
						error!("OK approve_transfer")
					},
					DispatchResult::Err(e) => {
						error!("ERROR approve_transfer");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call 'approve transfer'")
						})?;
					},
				}
			},

			//transfer_approved
			1109 => {
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let approve_transfer_request: ([u8; 32], PalletAssetRequest) = env.read_as()?;
				let owner = approve_transfer_request.0;
				let transfer_approved_request = approve_transfer_request.1;

				let origin_address = match transfer_approved_request.origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let mut vec = &owner.to_vec()[..];
				let owner_address = AccountId::decode(&mut vec).unwrap();

				let mut vec = &transfer_approved_request.target_address.to_vec()[..];
				let target_address = AccountId::decode(&mut vec).unwrap();

				let transfer_approved_result = pallet_assets::Pallet::<Runtime>::transfer_approved(
					Origin::signed(origin_address),
					transfer_approved_request.asset_id.into(),
					MultiAddress::Id(owner_address),
					MultiAddress::Id(target_address),
					transfer_approved_request.amount,
				);

				trace!("transfer_approved request {:#?}", transfer_approved_request);
				trace!("transfer_approved result {:#?}", transfer_approved_result);
				match transfer_approved_result {
					DispatchResult::Ok(_) => {
						error!("OK transfer_approved")
					},
					DispatchResult::Err(e) => {
						error!("ERROR transfer_approved");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other(
								"ChainExtension failed to call 'transfer approved'",
							)
						})?;
					},
				}
			},

			//allowance
			1110 => {
				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let allowance_request: (u32, [u8; 32], [u8; 32]) = env.read_as()?;
				let asset_id = allowance_request.0;

				let owner = allowance_request.1;
				let delegate = allowance_request.2;

				let mut vec = &owner.to_vec()[..];
				let owner_address = AccountId::decode(&mut vec).unwrap();

				let mut vec = &delegate.to_vec()[..];
				let delegate_address = AccountId::decode(&mut vec).unwrap();

				use crate::sp_api_hidden_includes_construct_runtime::hidden_include::traits::fungibles::approvals::Inspect;
				let allowance: u128 =
					Assets::allowance(asset_id.into(), &owner_address, &delegate_address);

				trace!("allowance request {:#?}", allowance_request);
				trace!("allowance result {:#?}", allowance);
				let b = allowance.to_be_bytes();
				//write buffer as responce for smart contract
				env.write(&b, false, None)
					.map_err(|_| DispatchError::Other("ChainExtension failed to call balance"))?;
			},

			//increase_allowance/decrease_allowance
			1111 => {
				use frame_support::dispatch::DispatchResult;

				let mut env = env.buf_in_buf_out();
				let request: (u32, [u8; 32], [u8; 32], u128, bool) = env.read_as()?;
				let (asset_id, owner, delegate, amount, is_increase) = request;

				let mut vec = &owner.to_vec()[..];
				let owner_address = AccountId::decode(&mut vec).unwrap();

				let mut vec = &delegate.to_vec()[..];
				let delegate_address = AccountId::decode(&mut vec).unwrap();

				use crate::sp_api_hidden_includes_construct_runtime::hidden_include::traits::fungibles::approvals::Inspect;
				let allowance: u128 =
					Assets::allowance(asset_id.into(), &owner_address, &delegate_address);

				let new_allowance = if is_increase {
					allowance + amount
				} else {
					if allowance < amount {
						0
					} else {
						allowance - amount
					}
				};

				let cancel_approval_result = pallet_assets::Pallet::<Runtime>::cancel_approval(
					Origin::signed(owner_address.clone()),
					asset_id.into(),
					MultiAddress::Id(delegate_address.clone()),
				);
				match cancel_approval_result {
					DispatchResult::Ok(_) => {
						error!("OK cancel_approval")
					},
					DispatchResult::Err(e) => {
						error!("ERROR cancel_approval");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call 'approve transfer'")
						})?;
					},
				}

				if cancel_approval_result.is_ok() {
					let approve_transfer_result =
						pallet_assets::Pallet::<Runtime>::approve_transfer(
							Origin::signed(owner_address),
							asset_id.into(),
							MultiAddress::Id(delegate_address),
							new_allowance,
						);

					error!("old allowance {}", allowance);
					error!("new allowance {}", new_allowance);
					error!("increase_allowance input {:#?}", request);
					error!("increase_allowance output {:#?}", approve_transfer_result);
					match approve_transfer_result {
						DispatchResult::Ok(_) => {
							error!("OK increase_allowance")
						},
						DispatchResult::Err(e) => {
							error!("ERROR increase_allowance");
							error!("{:#?}", e);
							let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
							env.write(err.encode().as_ref(), false, None).map_err(|_| {
								DispatchError::Other(
									"ChainExtension failed to call 'approve transfer'",
								)
							})?;
						},
					}
				}
			},

			//set_metadata
			1112 => {
				use frame_support::dispatch::DispatchResult;
				let ext = env.ext();
				let address = ext.address();
				let caller = ext.caller();
				let mut caller_ref = caller.as_ref();
				let mut address_ref = address.as_ref();
				let caller_account = AccountId::decode(&mut caller_ref).unwrap();
				let address_account = AccountId::decode(&mut address_ref).unwrap();

				let mut env = env.buf_in_buf_out();
				let input: (OriginType, u32, [u8; 32], [u8; 32], u8) = env.read_as()?;
				let (origin_type, asset_id, name, symbol, decimals) = input;

				let origin_address = match origin_type {
					OriginType::Caller => caller_account,
					OriginType::Address => address_account,
				};

				let result = pallet_assets::Pallet::<Runtime>::set_metadata(
					Origin::signed(origin_address),
					asset_id.into(),
					name.to_vec(),
					symbol.to_vec(),
					decimals,
				);

				error!("set_metadata : {:#?}", result);
				// error!("set_metadata input {:#?}", input);

				match result {
					DispatchResult::Ok(_) => {
						error!("OK set_metadata")
					},
					DispatchResult::Err(e) => {
						error!("ERROR set_metadata");
						error!("{:#?}", e);
						let err = Result::<(), PalletAssetErr>::Err(PalletAssetErr::from(e));
						env.write(err.encode().as_ref(), false, None).map_err(|_| {
							DispatchError::Other("ChainExtension failed to call 'set_metadata'")
						})?;
					},
				}
			},

			_ => {
				error!("Called an unregistered `func_id`: {:}", func_id);
				return Err(DispatchError::Other("Unimplemented func_id"))
			},
		}

		//let r = pallet_assets::Pallet::<Runtime>::total_supply(1);

		Ok(RetVal::Converging(0))
	}

	fn enabled() -> bool {
		true
	}
}
