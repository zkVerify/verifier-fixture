// Copyright 2024, Horizen Labs, Inc.
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

//! This module contains all the types, logic and wrapping to handle the ultraplonk-verifier proofs.
//! You can use it as a template for all other verifiers. Take care of the use of the _new type_
//! pattern for `Proof`, `Vk` and `Pubs`: is the only way to implement our json deserializing traits
//! for the verifier's types.
//!
//! Follow a (maybe not complete) list of wath you should change to support a different verifier:
//!
//! * `Proof`, `Vk` and `Pubs`: change the inner types and the `From` implementations
//! * `Config`: should implement your verifier config (if any)
//! * `verify_proof`: should call your verifier `verify_proof` function
//! * `TryFromJson` for `Proof`, `Vk` and `Pubs`: Here you should implement the parser like you're
//!   parsing the verifier types and then convert it to our own types with the last
//!   `map(Proof|Vk|Pubs)` call. Maybe the ultraplonk's `Proof` one is the simpler to understand
//!   it because the `Vec<u8>` already implements the `TryFromJson` trait.
//! * `From<VkEntry<...>>` implementation: change the pallet path.
//!
//! Please remember to add some tests to ensure everything works as expected.
//!
//!

use anyhow::{Context, anyhow};
use helpers::zkverify_node::{
    Api, VkEntry, VkFetch as VkFetchGen, VkResolver as VkResolverGen, block_hash, get_storage,
    storage,
};
use helpers::{Extrinsic, Hash, TakeJType, TryFromJson, TryFromParam};
use hp_verifiers::Verifier;
use serde_json::Value;
use sp_core::{Get, H256};
use verifier_fixture::Response;

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct Vk(pallet_ultraplonk_verifier::Vk);

impl From<pallet_ultraplonk_verifier::Vk> for Vk {
    fn from(vk: pallet_ultraplonk_verifier::Vk) -> Self {
        Vk(vk)
    }
}

#[derive(Hash, Eq, PartialEq)]
pub struct Proof(pallet_ultraplonk_verifier::Proof);
impl From<pallet_ultraplonk_verifier::Proof> for Proof {
    fn from(proof: pallet_ultraplonk_verifier::Proof) -> Self {
        Proof(proof)
    }
}

#[derive(Hash, Eq, PartialEq)]
pub struct Pubs(pallet_ultraplonk_verifier::Pubs);

impl From<pallet_ultraplonk_verifier::Pubs> for Pubs {
    fn from(pubs: pallet_ultraplonk_verifier::Pubs) -> Self {
        Pubs(pubs)
    }
}

/// That the type alias for this verifier `VkResolver`
pub type VkResolver = VkResolverGen<Vk, VkFetch>;

pub fn verify_proof(params: &VerifyParams) -> Response {
    pallet_ultraplonk_verifier::Ultraplonk::<Config>::verify_proof(
        &params.vk.0,
        &params.proof.0,
        &params.pubs.0,
    )
    .into()
}

struct MaxPubs;
impl Get<u32> for MaxPubs {
    fn get() -> u32 {
        16
    }
}

struct Config;
impl pallet_ultraplonk_verifier::Config for Config {
    type MaxPubs = MaxPubs;
}

pub struct ProofData {
    pub proof: Proof,
    pub vk: VkOrHash,
    pub pubs: Pubs,
}

#[derive(Hash, Eq, PartialEq)]
pub struct VerifyParams {
    pub vk: Vk,
    pub proof: Proof,
    pub pubs: Pubs,
}

pub struct ProofInfo {
    pub extrinsic_index: Extrinsic,

    pub proof_data: ProofData,
}

impl TryFromJson for ProofData {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let mut params = value.array()?;
        params.truncate(3);
        let [vk, proof, pubs]: [Value; 3] = params
            .try_into()
            .map_err(|orig| anyhow!("Should be at least 3 values array {orig:#?}"))?;
        Ok(Self {
            proof: Proof::try_from_param(proof)?,
            vk: VkOrHash::try_from_param(vk)?,
            pubs: Pubs::try_from_param(pubs)?,
        })
    }
}

impl TryFromJson for ProofInfo {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let mut outer = value.object()?;
        let extrinsic_index = outer
            .remove("extrinsic_index")
            .ok_or_else(|| anyhow::anyhow!("Expected extrinsic_index field"))?
            .string()?
            .parse()?;
        let proof_data = outer
            .remove("params")
            .ok_or_else(|| anyhow::anyhow!("Expected params field"))
            .and_then(ProofData::try_from_json)?;
        Ok(Self {
            proof_data,
            extrinsic_index,
        })
    }
}

#[derive(PartialEq, Hash)]
#[allow(clippy::large_enum_variant)]
pub enum VkOrHash {
    Vk(Vk),
    Hash(Hash),
}

impl TryFromJson for VkOrHash {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let mut value = value.object()?;

        if value.contains_key("Hash") {
            return Ok(VkOrHash::Hash(Hash::try_from_json(
                value.remove("Hash").unwrap(),
            )?));
        }
        if value.contains_key("Vk") {
            return Ok(VkOrHash::Vk(Vk::try_from_json(
                value.remove("Vk").unwrap(),
            )?));
        }
        Err(anyhow::anyhow!("Should be a Vk or Hash enum"))
    }
}

impl TryFromJson for Vk {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let inner = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Expected a string"))?;
        if !inner.starts_with("0x") {
            return Err(anyhow::anyhow!(
                "Invalid vk format: should starts with '0x'"
            ))?;
        }
        Ok(hex::decode(&inner[2..])
            .map_err(|_| anyhow::anyhow!("Invalid vk"))?
            .as_slice()
            .try_into()
            .map(Vk)?)
    }
}

impl TryFromJson for Proof {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        pallet_ultraplonk_verifier::Proof::try_from_json(value).map(Proof)
    }
}

impl TryFromJson for Pubs {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        value
            .array()?
            .into_iter()
            .map(TryFromJson::try_from_json)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(Pubs)
    }
}

pub struct VkFetch;

#[async_trait::async_trait]
impl VkFetchGen<Vk> for VkFetch {
    async fn fetch(api: Api, vk_hash: Hash, block: Option<u32>) -> anyhow::Result<Option<Vk>> {
        let hash = H256(vk_hash);

        let block = match block {
            Some(block) => block_hash(api.clone(), block).await?,
            None => None,
        };

        // Address to a storage entry we'd like to access.
        let address = storage().settlement_ultraplonk_pallet().vks(hash);

        get_storage(api, block)
            .await?
            .fetch(&address)
            .await
            .map(|entry| entry.map(Into::into))
            .with_context(|| "Failed to fetch vk entry")
    }
}

pub fn verify_params<F: VkFetchGen<Vk>>(
    resolver: &VkResolverGen<Vk, F>,
    data: ProofData,
) -> Option<VerifyParams> {
    let ProofData { proof, vk, pubs } = data;
    match vk {
        VkOrHash::Vk(vk) => Some(VerifyParams { vk, proof, pubs }),
        VkOrHash::Hash(vk_hash) => resolver.get(&vk_hash).map(|vk| VerifyParams {
            vk: vk.clone(),
            proof,
            pubs,
        }),
    }
}

impl From<VkEntry<pallet_ultraplonk_verifier::Vk>> for Vk {
    fn from(entry: VkEntry<pallet_ultraplonk_verifier::Vk>) -> Self {
        Vk(entry.vk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helpers::TryFromParam;
    use serde_json::json;

    #[test]
    fn convert_vk_hash_param_hash() {
        let json = json!({
            "value": {
                "Hash": "0x3cc9cada1efaff3f5bc9e060a53fab05ee5ad907457dad517cba96572e0656ac"
            }
        });
        let vk_or_hash: VkOrHash = VkOrHash::try_from_param(json).unwrap();

        assert!(
            VkOrHash::Hash(hex_literal::hex!(
                "3cc9cada1efaff3f5bc9e060a53fab05ee5ad907457dad517cba96572e0656ac"
            )) == vk_or_hash
        );
    }

    #[test]
    fn convert_vk_hash_param_vk() {
        let data = (0..51)
            .map(|_| "3cc9cada1efaff3f5bc9e060a53fab05ee5ad907457dad517cba96572e0656ac")
            .collect::<String>();
        let hex_str = format!("0x{data}");
        let json = json!({
            "value": {
                "Vk": hex_str
            }
        });
        let vk_or_hash: VkOrHash = VkOrHash::try_from_param(json).unwrap();

        assert!(VkOrHash::Vk(Vk(hex::decode(&data).unwrap().try_into().unwrap())) == vk_or_hash);
    }

    #[test]
    fn convert_proof_param() {
        let s = "0b0239d7f84da98beadd1882648c8869f7222175644be708c4b0052ea863dce0";

        let json = json!({
            "value": format!("0x{s}")
        });
        let proof = Proof::try_from_param(json).unwrap();

        assert!(Proof(hex::decode(s).unwrap()) == proof);
    }

    #[test]
    fn convert_pubs_param() {
        let values = vec![
            hex_literal::hex!("000000000000000000000000a18ddcbb4e9b6cd9bc4f03071cc511f4c9945c87"),
            hex_literal::hex!("2e05a5199bf0a4d591fdf1219f41ae0dde4605dd9ac85fc49c4530a6225498ad"),
            hex_literal::hex!("00000000000000000000000000b0a2b826da8afbf65c0037a0173bb4dae4dc9f"),
            hex_literal::hex!("00000000000000000000000000000000000000000000000043b93e2507e80000"),
            hex_literal::hex!("12005fbb8501c38e9ea4cec6aca49d8ec986544d419cad7816c10074ebccd860"),
        ];

        let jvalues = values
            .iter()
            .map(|v| format!("0x{}", hex::encode(v)))
            .collect::<Vec<_>>();

        let json = json!({
            "value": jvalues
        });
        let pubs = Pubs::try_from_param(json).unwrap();

        assert!(Pubs(values) == pubs);
    }
}
