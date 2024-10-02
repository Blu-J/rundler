// This file is part of Rundler.
//
// Rundler is free software: you can redistribute it and/or modify it under the
// terms of the GNU Lesser General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later version.
//
// Rundler is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
// without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with Rundler.
// If not, see https://www.gnu.org/licenses/.

use alloy_json_rpc::{RpcParam, RpcReturn};
use alloy_primitives::{Address, Bytes, TxHash, B256, U256};
use alloy_rpc_types_eth::{
    state::StateOverride, Block, BlockId, BlockNumberOrTag, FeeHistory, Filter, Log, Transaction,
    TransactionReceipt, TransactionRequest,
};
use alloy_rpc_types_trace::geth::{
    GethDebugTracingCallOptions, GethDebugTracingOptions, GethTrace,
};
use rundler_contracts::utils::GetGasUsed::GasUsedResult;
use rundler_types::{
    v0_6, v0_7, GasFees, UserOpsPerAggregator, ValidationOutput, ValidationRevert,
};

use super::error::ProviderResult;
use crate::{
    AggregatorOut, BundleHandler, DepositInfo, EntryPoint, EvmCall,
    EvmProvider as EvmProviderTrait, ExecutionResult, HandleOpsOut, L1GasProvider,
    SignatureAggregator, SimulationProvider,
};

mockall::mock! {
    pub EvmProvider {}

    #[async_trait::async_trait]
    impl EvmProviderTrait for EvmProvider {
        async fn request<P, R>(&self, method: &'static str, params: P) -> ProviderResult<R>
        where
            P: RpcParam + 'static,
            R: RpcReturn;

        async fn fee_history(
            &self,
            block_count: u64,
            block_number: BlockNumberOrTag,
            reward_percentiles: &[f64],
        ) -> ProviderResult<FeeHistory>;

        async fn call(
            &self,
            tx: &TransactionRequest,
            block: Option<BlockId>,
            state_overrides: &StateOverride,
        ) -> ProviderResult<Bytes>;

        async fn get_block_number(&self) -> ProviderResult<u64>;

        async fn get_block(&self, block_id: BlockId) -> ProviderResult<Option<Block>>;

        async fn get_balance(&self, address: Address, block: Option<BlockId>) -> ProviderResult<U256>;

        async fn get_transaction_by_hash(&self, tx: TxHash) -> ProviderResult<Option<Transaction>>;

        async fn get_transaction_receipt(
            &self,
            tx: TxHash,
        ) -> ProviderResult<Option<TransactionReceipt>>;

        async fn debug_trace_transaction(
            &self,
            tx_hash: TxHash,
            trace_options: GethDebugTracingOptions,
        ) -> ProviderResult<GethTrace>;

        async fn debug_trace_call(
            &self,
            tx: TransactionRequest,
            block_id: Option<BlockId>,
            trace_options: GethDebugTracingCallOptions,
        ) -> ProviderResult<GethTrace>;

        async fn get_latest_block_hash_and_number(&self) -> ProviderResult<(B256, u64)>;

        async fn get_pending_base_fee(&self) -> ProviderResult<u128>;

        async fn get_max_priority_fee(&self) -> ProviderResult<u128>;

        async fn get_code(&self, address: Address, block: Option<BlockId>) -> ProviderResult<Bytes>;

        async fn get_transaction_count(&self, address: Address) -> ProviderResult<u64>;

        async fn get_logs(&self, filter: &Filter) -> ProviderResult<Vec<Log>>;

        async fn get_gas_used(&self, call: EvmCall) -> ProviderResult<GasUsedResult>;

        async fn batch_get_storage_at(
            &self,
            address: Address,
            slots: Vec<B256>,
        ) -> ProviderResult<Vec<B256>>;

        async fn get_code_hash(
            &self,
            addresses: Vec<Address>,
            block: Option<BlockId>,
        ) -> ProviderResult<B256>;
    }
}

mockall::mock! {
    pub EntryPointV0_6 {}

    #[async_trait::async_trait]
    impl EntryPoint for EntryPointV0_6 {
        fn address(&self) -> &Address;
        async fn balance_of(&self, address: Address, block_id: Option<BlockId>)
            -> ProviderResult<U256>;
        async fn get_deposit_info(&self, address: Address) -> ProviderResult<DepositInfo>;
        async fn get_balances(&self, addresses: Vec<Address>) -> ProviderResult<Vec<U256>>;
    }

    #[async_trait::async_trait]
    impl SignatureAggregator for EntryPointV0_6 {
        type UO = v0_6::UserOperation;
        async fn aggregate_signatures(
            &self,
            aggregator_address: Address,
            ops: Vec<v0_6::UserOperation>,
        ) -> ProviderResult<Option<Bytes>>;
        async fn validate_user_op_signature(
            &self,
            aggregator_address: Address,
            user_op: v0_6::UserOperation,
            gas_cap: u64,
        ) -> ProviderResult<AggregatorOut>;
    }

    #[async_trait::async_trait]
    impl SimulationProvider for EntryPointV0_6 {
        type UO = v0_6::UserOperation;
        fn get_tracer_simulate_validation_call(
            &self,
            user_op: v0_6::UserOperation,
            max_validation_gas: u64,
        ) -> ProviderResult<(TransactionRequest, StateOverride)>;
        async fn simulate_validation(
            &self,
            user_op: v0_6::UserOperation,
            max_validation_gas: u64,
            block_id: Option<BlockId>
        ) -> ProviderResult<Result<ValidationOutput, ValidationRevert>>;
        fn get_simulate_handle_op_call(
            &self,
            op: v0_6::UserOperation,
            state_override: StateOverride,
        ) -> crate::EvmCall;
        async fn simulate_handle_op(
            &self,
            op: v0_6::UserOperation,
            target: Address,
            target_call_data: Bytes,
            block_id: BlockId,
            gas: u64,
            state_override: StateOverride,
        ) -> ProviderResult<Result<ExecutionResult, ValidationRevert>>;
        fn decode_simulate_handle_ops_revert(
            revert_data: &Bytes,
        ) -> ProviderResult<Result<ExecutionResult, ValidationRevert>>;
        fn simulation_should_revert(&self) -> bool;
    }

    #[async_trait::async_trait]
    impl L1GasProvider for EntryPointV0_6 {
        type UO = v0_6::UserOperation;
        async fn calc_l1_gas(
            &self,
            entry_point_address: Address,
            op: v0_6::UserOperation,
            gas_price: u128,
        ) -> ProviderResult<u128>;
    }

    #[async_trait::async_trait]
    impl BundleHandler for EntryPointV0_6 {
        type UO = v0_6::UserOperation;
        async fn call_handle_ops(
            &self,
            ops_per_aggregator: Vec<UserOpsPerAggregator<v0_6::UserOperation>>,
            beneficiary: Address,
            gas: u64,
        ) -> ProviderResult<HandleOpsOut>;
        fn get_send_bundle_transaction(
            &self,
            ops_per_aggregator: Vec<UserOpsPerAggregator<v0_6::UserOperation>>,
            beneficiary: Address,
            gas: u64,
            gas_fees: GasFees,
        ) -> TransactionRequest;
    }
}

mockall::mock! {
    pub EntryPointV0_7 {}

    #[async_trait::async_trait]
    impl EntryPoint for EntryPointV0_7 {
        fn address(&self) -> &Address;
        async fn balance_of(&self, address: Address, block_id: Option<BlockId>)
            -> ProviderResult<U256>;
        async fn get_deposit_info(&self, address: Address) -> ProviderResult<DepositInfo>;
        async fn get_balances(&self, addresses: Vec<Address>) -> ProviderResult<Vec<U256>>;
    }

    #[async_trait::async_trait]
    impl SignatureAggregator for EntryPointV0_7 {
        type UO = v0_7::UserOperation;
        async fn aggregate_signatures(
            &self,
            aggregator_address: Address,
            ops: Vec<v0_7::UserOperation>,
        ) -> ProviderResult<Option<Bytes>>;
        async fn validate_user_op_signature(
            &self,
            aggregator_address: Address,
            user_op: v0_7::UserOperation,
            gas_cap: u64,
        ) -> ProviderResult<AggregatorOut>;
    }

    #[async_trait::async_trait]
    impl SimulationProvider for EntryPointV0_7 {
        type UO = v0_7::UserOperation;
        fn get_tracer_simulate_validation_call(
            &self,
            user_op: v0_7::UserOperation,
            max_validation_gas: u64,
        ) -> ProviderResult<(TransactionRequest, StateOverride)>;
        async fn simulate_validation(
            &self,
            user_op: v0_7::UserOperation,
            max_validation_gas: u64,
            block_id: Option<BlockId>
        ) -> ProviderResult<Result<ValidationOutput, ValidationRevert>>;
        fn get_simulate_handle_op_call(
            &self,
            op: v0_7::UserOperation,
            state_override: StateOverride,
        ) -> crate::EvmCall;
        async fn simulate_handle_op(
            &self,
            op: v0_7::UserOperation,
            target: Address,
            target_call_data: Bytes,
            block_id: BlockId,
            gas: u64,
            state_override: StateOverride,
        ) -> ProviderResult<Result<ExecutionResult, ValidationRevert>>;
        fn decode_simulate_handle_ops_revert(
            revert_data: &Bytes,
        ) -> ProviderResult<Result<ExecutionResult, ValidationRevert>>;
        fn simulation_should_revert(&self) -> bool;
    }

    #[async_trait::async_trait]
    impl L1GasProvider for EntryPointV0_7 {
        type UO = v0_7::UserOperation;
        async fn calc_l1_gas(
            &self,
            entry_point_address: Address,
            op: v0_7::UserOperation,
            gas_price: u128,
        ) -> ProviderResult<u128>;
    }

    #[async_trait::async_trait]
    impl BundleHandler for EntryPointV0_7 {
        type UO = v0_7::UserOperation;
        async fn call_handle_ops(
            &self,
            ops_per_aggregator: Vec<UserOpsPerAggregator<v0_7::UserOperation>>,
            beneficiary: Address,
            gas: u64,
        ) -> ProviderResult<HandleOpsOut>;
        fn get_send_bundle_transaction(
            &self,
            ops_per_aggregator: Vec<UserOpsPerAggregator<v0_7::UserOperation>>,
            beneficiary: Address,
            gas: u64,
            gas_fees: GasFees,
        ) -> TransactionRequest;
    }
}
