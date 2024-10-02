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

use alloy_primitives::{ruint::FromUintError, Address, Bytes, B256, U256};
use alloy_sol_types::{sol, SolValue};
pub use rundler_contracts::v0_6::UserOperation as ContractUserOperation;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use super::{
    random_bytes, random_bytes_array, UserOperation as UserOperationTrait, UserOperationId,
    UserOperationVariant,
};
use crate::{
    chain::ChainSpec,
    entity::{Entity, EntityType},
    EntryPointVersion,
};

/// Gas overhead required by the entry point contract for the inner call
pub const ENTRY_POINT_INNER_GAS_OVERHEAD: u128 = 5000;

/// Number of bytes in the fixed size portion of an ABI encoded user operation
/// sender = 32 bytes
/// nonce = 32 bytes
/// init_code = 32 bytes + 32 bytes num elems + var 32
/// call_data = 32 bytes + 32 bytes num elems + var 32
/// call_gas_limit = 32 bytes
/// verification_gas_limit = 32 bytes
/// pre_verification_gas = 32 bytes
/// max_fee_per_gas = 32 bytes
/// max_priority_fee_per_gas = 32 bytes
/// paymaster_and_data = 32 bytes + 32 bytes num elems + var 32
/// signature = 32 bytes + 32 bytes num elems + var 32
///
/// +1 for good luck? (aka this is what I got when I was testing)
///
/// 16 * 32 = 480
const ABI_ENCODED_USER_OPERATION_FIXED_LEN: usize = 512;

/// User Operation for Entry Point v0.6
///
/// Direct conversion to/from onchain UserOperation
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct UserOperation {
    /// Sender
    pub sender: Address,
    /// Semi-abstracted nonce
    ///
    /// The first 192 bits are the nonce key, the last 64 bits are the nonce value
    pub nonce: U256,
    /// Init code
    pub init_code: Bytes,
    /// Call data
    pub call_data: Bytes,
    /// Call gas limit
    pub call_gas_limit: u128,
    /// Verification gas limit
    pub verification_gas_limit: u128,
    /// Pre verification gas
    pub pre_verification_gas: u128,
    /// Max fee per gas
    pub max_fee_per_gas: u128,
    /// Max priority fee per gas
    pub max_priority_fee_per_gas: u128,
    /// Paymaster and data
    pub paymaster_and_data: Bytes,
    /// Signature
    pub signature: Bytes,
}

sol! {
    #[allow(missing_docs)]
    #[derive(Default, Debug, PartialEq, Eq)]
    struct UserOperationHashEncoded {
        bytes32 encodedHash;
        address entryPoint;
        uint256 chainId;
    }

    #[allow(missing_docs)]
    #[derive(Default, Debug, PartialEq, Eq)]
    struct UserOperationPackedForHash {
        address sender;
        uint256 nonce;
        bytes32 hashInitCode;
        bytes32 hashCallData;
        uint256 callGasLimit;
        uint256 verificationGasLimit;
        uint256 preVerificationGas;
        uint256 maxFeePerGas;
        uint256 maxPriorityFeePerGas;
        bytes32 hashPaymasterAndData;
    }
}

impl From<UserOperation> for UserOperationPackedForHash {
    fn from(op: UserOperation) -> UserOperationPackedForHash {
        UserOperationPackedForHash {
            sender: op.sender,
            nonce: op.nonce,
            hashInitCode: alloy_primitives::keccak256(op.init_code),
            hashCallData: alloy_primitives::keccak256(op.call_data),
            callGasLimit: U256::from(op.call_gas_limit),
            verificationGasLimit: U256::from(op.verification_gas_limit),
            preVerificationGas: U256::from(op.pre_verification_gas),
            maxFeePerGas: U256::from(op.max_fee_per_gas),
            maxPriorityFeePerGas: U256::from(op.max_priority_fee_per_gas),
            hashPaymasterAndData: alloy_primitives::keccak256(op.paymaster_and_data),
        }
    }
}

impl UserOperationTrait for UserOperation {
    type OptionalGas = UserOperationOptionalGas;

    fn entry_point_version() -> EntryPointVersion {
        EntryPointVersion::V0_6
    }

    fn hash(&self, entry_point: Address, chain_id: u64) -> B256 {
        let packed = UserOperationPackedForHash::from(self.clone());
        let encoded = UserOperationHashEncoded {
            encodedHash: alloy_primitives::keccak256(packed.abi_encode()),
            entryPoint: entry_point,
            chainId: U256::from(chain_id),
        };

        alloy_primitives::keccak256(encoded.abi_encode())
    }

    fn id(&self) -> UserOperationId {
        UserOperationId {
            sender: self.sender,
            nonce: self.nonce,
        }
    }

    fn sender(&self) -> Address {
        self.sender
    }

    fn nonce(&self) -> U256 {
        self.nonce
    }

    fn factory(&self) -> Option<Address> {
        Self::get_address_from_field(&self.init_code)
    }

    fn paymaster(&self) -> Option<Address> {
        Self::get_address_from_field(&self.paymaster_and_data)
    }

    fn call_data(&self) -> &Bytes {
        &self.call_data
    }

    fn max_gas_cost(&self) -> U256 {
        let mul: u128 = if self.paymaster().is_some() { 3 } else { 1 };
        U256::from(
            self.max_fee_per_gas
                * (self.pre_verification_gas
                    + self.call_gas_limit
                    + self.verification_gas_limit * mul),
        )
    }

    fn heap_size(&self) -> usize {
        self.init_code.len()
            + self.call_data.len()
            + self.paymaster_and_data.len()
            + self.signature.len()
    }

    fn entities(&self) -> Vec<Entity> {
        EntityType::iter()
            .filter_map(|entity| {
                self.entity_address(entity)
                    .map(|address| Entity::new(entity, address))
            })
            .collect()
    }

    fn max_fee_per_gas(&self) -> u128 {
        self.max_fee_per_gas
    }

    fn max_priority_fee_per_gas(&self) -> u128 {
        self.max_priority_fee_per_gas
    }

    fn call_gas_limit(&self) -> u128 {
        self.call_gas_limit
    }

    fn pre_verification_gas(&self) -> u128 {
        self.pre_verification_gas
    }

    fn verification_gas_limit(&self) -> u128 {
        self.verification_gas_limit
    }

    fn total_verification_gas_limit(&self) -> u128 {
        let mul: u128 = if self.paymaster().is_some() { 2 } else { 1 };
        self.verification_gas_limit * mul
    }

    fn required_pre_execution_buffer(&self) -> u128 {
        self.verification_gas_limit + ENTRY_POINT_INNER_GAS_OVERHEAD
    }

    fn calc_static_pre_verification_gas(
        &self,
        chain_spec: &ChainSpec,
        include_fixed_gas_overhead: bool,
    ) -> u128 {
        super::op_calldata_gas_cost(
            ContractUserOperation::from(self.clone()),
            chain_spec.calldata_zero_byte_gas as u128,
            chain_spec.calldata_non_zero_byte_gas as u128,
            chain_spec.per_user_op_word_gas as u128,
        ) + chain_spec.per_user_op_v0_6_gas as u128
            + (if self.factory().is_some() {
                chain_spec.per_user_op_deploy_overhead_gas as u128
            } else {
                0
            })
            + (if include_fixed_gas_overhead {
                chain_spec.transaction_intrinsic_gas as u128
            } else {
                0
            })
    }

    fn clear_signature(&mut self) {
        self.signature = Bytes::default();
    }

    fn abi_encoded_size(&self) -> usize {
        ABI_ENCODED_USER_OPERATION_FIXED_LEN
            + super::byte_array_abi_len(&self.init_code)
            + super::byte_array_abi_len(&self.call_data)
            + super::byte_array_abi_len(&self.paymaster_and_data)
            + super::byte_array_abi_len(&self.signature)
    }
}

impl From<UserOperation> for ContractUserOperation {
    fn from(op: UserOperation) -> Self {
        ContractUserOperation {
            sender: op.sender,
            nonce: op.nonce,
            initCode: op.init_code,
            callData: op.call_data,
            callGasLimit: U256::from(op.call_gas_limit),
            verificationGasLimit: U256::from(op.verification_gas_limit),
            preVerificationGas: U256::from(op.pre_verification_gas),
            maxFeePerGas: U256::from(op.max_fee_per_gas),
            maxPriorityFeePerGas: U256::from(op.max_priority_fee_per_gas),
            paymasterAndData: op.paymaster_and_data,
            signature: op.signature,
        }
    }
}

impl TryFrom<ContractUserOperation> for UserOperation {
    type Error = FromUintError<u128>;

    fn try_from(op: ContractUserOperation) -> Result<Self, Self::Error> {
        Ok(UserOperation {
            sender: op.sender,
            nonce: op.nonce,
            init_code: op.initCode,
            call_data: op.callData,
            call_gas_limit: op.callGasLimit.try_into()?,
            verification_gas_limit: op.verificationGasLimit.try_into()?,
            pre_verification_gas: op.preVerificationGas.try_into()?,
            max_fee_per_gas: op.maxFeePerGas.try_into()?,
            max_priority_fee_per_gas: op.maxPriorityFeePerGas.try_into()?,
            paymaster_and_data: op.paymasterAndData,
            signature: op.signature,
        })
    }
}

impl UserOperation {
    fn get_address_from_field(data: &Bytes) -> Option<Address> {
        if data.len() < 20 {
            None
        } else {
            Some(Address::from_slice(&data[..20]))
        }
    }

    fn entity_address(&self, entity: EntityType) -> Option<Address> {
        match entity {
            EntityType::Account => Some(self.sender),
            EntityType::Paymaster => self.paymaster(),
            EntityType::Factory => self.factory(),
            EntityType::Aggregator => None,
        }
    }
}

impl From<UserOperationVariant> for UserOperation {
    /// Converts a UserOperationVariant to a UserOperation 0.6
    ///
    /// # Panics
    ///
    /// Panics if the variant is not v0.6. This is for use in contexts
    /// where the variant is known to be v0.6.
    fn from(value: UserOperationVariant) -> Self {
        value.into_v0_6().expect("Expected UserOperationV0_6")
    }
}

impl From<UserOperation> for super::UserOperationVariant {
    fn from(op: UserOperation) -> Self {
        super::UserOperationVariant::V0_6(op)
    }
}

impl AsRef<UserOperation> for super::UserOperationVariant {
    /// # Panics
    ///
    /// Panics if the variant is not v0.6. This is for use in contexts
    /// where the variant is known to be v0.6.
    fn as_ref(&self) -> &UserOperation {
        match self {
            super::UserOperationVariant::V0_6(op) => op,
            _ => panic!("Expected UserOperationV0_6"),
        }
    }
}

impl AsMut<UserOperation> for super::UserOperationVariant {
    /// # Panics
    ///
    /// Panics if the variant is not v0.6. This is for use in contexts
    /// where the variant is known to be v0.6.
    fn as_mut(&mut self) -> &mut UserOperation {
        match self {
            super::UserOperationVariant::V0_6(op) => op,
            _ => panic!("Expected UserOperationV0_6"),
        }
    }
}

/// User operation with optional gas fields for gas estimation
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserOperationOptionalGas {
    /// Sender (required)
    pub sender: Address,
    /// Nonce (required)
    pub nonce: U256,
    /// Init code (required)
    pub init_code: Bytes,
    /// Call data (required)
    pub call_data: Bytes,
    /// Call gas limit (optional, set to maximum if unset)
    pub call_gas_limit: Option<u128>,
    /// Verification gas limit (optional, set to maximum if unset)
    pub verification_gas_limit: Option<u128>,
    /// Pre verification gas (optional, ignored if set)
    pub pre_verification_gas: Option<u128>,
    /// Max fee per gas (optional, ignored if set)
    pub max_fee_per_gas: Option<u128>,
    /// Max priority fee per gas (optional, ignored if set)
    pub max_priority_fee_per_gas: Option<u128>,
    /// Paymaster and data (required, dummy value for gas estimation)
    pub paymaster_and_data: Bytes,
    /// Signature (required, dummy value for gas estimation)
    pub signature: Bytes,
}

impl UserOperationOptionalGas {
    /// Fill in the optional and dummy fields of the user operation with values
    /// that will cause the maximum possible calldata gas cost.
    pub fn max_fill(&self, max_call_gas: u128, max_verification_gas: u128) -> UserOperation {
        UserOperation {
            call_gas_limit: u128::MAX,
            verification_gas_limit: u128::MAX,
            pre_verification_gas: u128::MAX,
            max_fee_per_gas: u128::MAX,
            max_priority_fee_per_gas: u128::MAX,
            signature: vec![255_u8; self.signature.len()].into(),
            paymaster_and_data: vec![255_u8; self.paymaster_and_data.len()].into(),
            ..self
                .clone()
                .into_user_operation(max_call_gas, max_verification_gas)
        }
    }

    /// Fill in the optional and dummy fields of the user operation with random values.
    ///
    /// When estimating pre-verification gas, specifically on networks that use
    /// compression algorithms on their data that they post to their data availability
    /// layer (like Arbitrum), it is important to make sure that the data that is
    /// random such that it compresses to a representative size.
    //
    /// Note that this will slightly overestimate the calldata gas needed as it uses
    /// the worst case scenario for the unknown gas values and paymaster_and_data.
    pub fn random_fill(&self, max_call_gas: u128, max_verification_gas: u128) -> UserOperation {
        UserOperation {
            call_gas_limit: u128::from_le_bytes(random_bytes_array::<16, 4>()), // 30M max
            verification_gas_limit: u128::from_le_bytes(random_bytes_array::<16, 4>()), // 30M max
            pre_verification_gas: u128::from_le_bytes(random_bytes_array::<16, 4>()), // 30M max
            max_fee_per_gas: u128::from_le_bytes(random_bytes_array::<16, 8>()), // 2^64 max
            max_priority_fee_per_gas: u128::from_le_bytes(random_bytes_array::<16, 8>()), // 2^64 max
            signature: random_bytes(self.signature.len()),
            paymaster_and_data: random_bytes(self.paymaster_and_data.len()),
            ..self
                .clone()
                .into_user_operation(max_call_gas, max_verification_gas)
        }
    }

    /// Convert into a full user operation.
    /// Fill in the optional fields of the user operation with default values if unset
    pub fn into_user_operation(
        self,
        max_call_gas: u128,
        max_verification_gas: u128,
    ) -> UserOperation {
        // If unset or zero, default these to gas limits from settings
        // Cap their values to the gas limits from settings
        let cgl = super::default_if_none_or_equal(self.call_gas_limit, max_call_gas, 0);
        let vgl =
            super::default_if_none_or_equal(self.verification_gas_limit, max_verification_gas, 0);
        let pvg = super::default_if_none_or_equal(self.pre_verification_gas, max_call_gas, 0);

        UserOperation {
            sender: self.sender,
            nonce: self.nonce,
            init_code: self.init_code,
            call_data: self.call_data,
            paymaster_and_data: self.paymaster_and_data,
            signature: self.signature,
            verification_gas_limit: vgl,
            call_gas_limit: cgl,
            pre_verification_gas: pvg,
            // These aren't used in gas estimation, set to if unset 0 so that there are no payment attempts during gas estimation
            max_fee_per_gas: self.max_fee_per_gas.unwrap_or_default(),
            max_priority_fee_per_gas: self.max_priority_fee_per_gas.unwrap_or_default(),
        }
    }

    /// Abi encoded size of the user operation (with its dummy fields)
    pub fn abi_encoded_size(&self) -> usize {
        ABI_ENCODED_USER_OPERATION_FIXED_LEN
            + super::byte_array_abi_len(&self.init_code)
            + super::byte_array_abi_len(&self.call_data)
            + super::byte_array_abi_len(&self.paymaster_and_data)
            + super::byte_array_abi_len(&self.signature)
    }
}

impl From<super::UserOperationOptionalGas> for UserOperationOptionalGas {
    /// # Panics
    ///
    /// Panics if the variant is not v0.6. This is for use in contexts
    /// where the variant is known to be v0.6.
    fn from(op: super::UserOperationOptionalGas) -> Self {
        match op {
            super::UserOperationOptionalGas::V0_6(op) => op,
            _ => panic!("Expected UserOperationOptionalGasV0_6"),
        }
    }
}

#[cfg(test)]
mod tests {

    use alloy_primitives::{address, b256, bytes};

    use super::*;

    #[test]
    fn test_hash_zeroed() {
        // Testing a user operation hash against the hash generated by the
        // entrypoint contract getUserOpHash() function with entrypoint address
        // at 0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc and chain ID 1337.
        //
        // UserOperation = {
        //     sender: '0x0000000000000000000000000000000000000000',
        //     nonce: 0,
        //     initCode: '0x',
        //     callData: '0x',
        //     callGasLimit: 0,
        //     verificationGasLimit: 0,
        //     preVerificationGas: 0,
        //     maxFeePerGas: 0,
        //     maxPriorityFeePerGas: 0,
        //     paymasterAndData: '0x',
        //     signature: '0x',
        //   }
        //
        // Hash: 0xdca97c3b49558ab360659f6ead939773be8bf26631e61bb17045bb70dc983b2d
        let operation = UserOperation {
            sender: address!("0000000000000000000000000000000000000000"),
            nonce: U256::ZERO,
            init_code: Bytes::default(),
            call_data: Bytes::default(),
            call_gas_limit: 0,
            verification_gas_limit: 0,
            pre_verification_gas: 0,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: Bytes::default(),
            signature: Bytes::default(),
        };
        let entry_point = address!("66a15edcc3b50a663e72f1457ffd49b9ae284ddc");
        let chain_id = 1337;
        let hash = operation.hash(entry_point, chain_id);
        assert_eq!(
            hash,
            b256!("dca97c3b49558ab360659f6ead939773be8bf26631e61bb17045bb70dc983b2d")
        );
    }

    #[test]
    fn test_hash() {
        // Testing a user operation hash against the hash generated by the
        // entrypoint contract getUserOpHash() function with entrypoint address
        // at 0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc and chain ID 1337.
        //
        // UserOperation = {
        //     sender: '0x1306b01bc3e4ad202612d3843387e94737673f53',
        //     nonce: 8942,
        //     initCode: '0x6942069420694206942069420694206942069420',
        //     callData: '0x0000000000000000000000000000000000000000080085',
        //     callGasLimit: 10000,
        //     verificationGasLimit: 100000,
        //     preVerificationGas: 100,
        //     maxFeePerGas: 99999,
        //     maxPriorityFeePerGas: 9999999,
        //     paymasterAndData:
        //       '0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        //     signature:
        //       '0xda0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a',
        //   }
        //
        // Hash: 0x484add9e4d8c3172d11b5feb6a3cc712280e176d278027cfa02ee396eb28afa1
        let operation = UserOperation {
            sender: "0x1306b01bc3e4ad202612d3843387e94737673f53"
                .parse()
                .unwrap(),
            nonce: U256::from(8942),
            init_code: "0x6942069420694206942069420694206942069420"
                .parse()
                .unwrap(),
            call_data: "0x0000000000000000000000000000000000000000080085"
                .parse()
                .unwrap(),
            call_gas_limit: 10_000,
            verification_gas_limit: 100_000,
            pre_verification_gas: 100,
            max_fee_per_gas: 99_999,
            max_priority_fee_per_gas: 9_999_999,
            paymaster_and_data: bytes!(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            ),
            signature: bytes!("da0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a"),
        };
        let entry_point = address!("66a15edcc3b50a663e72f1457ffd49b9ae284ddc");
        let chain_id = 1337;
        let hash = operation.hash(entry_point, chain_id);
        assert_eq!(
            hash,
            b256!("484add9e4d8c3172d11b5feb6a3cc712280e176d278027cfa02ee396eb28afa1")
        );
    }

    #[test]
    fn test_get_address_from_field() {
        let paymaster_and_data: Bytes =
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .parse()
                .unwrap();
        let address = UserOperation::get_address_from_field(&paymaster_and_data).unwrap();
        assert_eq!(
            address,
            address!("0123456789abcdef0123456789abcdef01234567")
        );
    }

    #[test]
    fn test_abi_encoded_size() {
        let operation = UserOperation {
            sender: "0x1306b01bc3e4ad202612d3843387e94737673f53"
                .parse()
                .unwrap(),
            nonce: U256::from(8942),
            init_code: "0x6942069420694206942069420694206942069420"
                .parse()
                .unwrap(),
            call_data: "0x0000000000000000000000000000000000000000080085"
                .parse()
                .unwrap(),
            call_gas_limit: 10_000,
            verification_gas_limit: 100_000,
            pre_verification_gas: 100,
            max_fee_per_gas: 99_999,
            max_priority_fee_per_gas: 9_999_999,
            paymaster_and_data: bytes!(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            ),
            signature: bytes!("da0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a"),
        };

        let size = operation.abi_encoded_size();
        let cuo = ContractUserOperation::from(operation).abi_encode();
        assert_eq!(size, cuo.len());
    }

    #[test]
    fn test_abi_encoded_size_min() {
        let operation = UserOperation {
            sender: address!("0000000000000000000000000000000000000000"),
            nonce: U256::ZERO,
            init_code: Bytes::default(),
            call_data: Bytes::default(),
            call_gas_limit: 0,
            verification_gas_limit: 0,
            pre_verification_gas: 0,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: Bytes::default(),
            signature: Bytes::default(),
        };
        let size = operation.abi_encoded_size();
        let cuo = ContractUserOperation::from(operation).abi_encode();
        assert_eq!(size, cuo.len());
    }

    #[test]
    fn test_abi_encoded_size_max() {
        let max_op = UserOperationOptionalGas {
            sender: "0x1306b01bc3e4ad202612d3843387e94737673f53"
                .parse()
                .unwrap(),
            nonce: U256::MAX,
            init_code: "0x6942069420694206942069420694206942069420"
                .parse()
                .unwrap(),
            call_data: "0x0000000000000000000000000000000000000000080085"
                .parse()
                .unwrap(),
            paymaster_and_data: bytes!(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            ),
            signature: bytes!("da0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a"),
            call_gas_limit: None,
            verification_gas_limit: None,
            pre_verification_gas: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
        .max_fill(u128::MAX, u128::MAX);

        let size = max_op.abi_encoded_size();
        let cuo = ContractUserOperation::from(max_op).abi_encode();
        assert_eq!(size, cuo.len());
    }
}
