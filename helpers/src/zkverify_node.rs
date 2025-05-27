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

//! Interface for the zkverify node and an utility class to implement
//! fetch the Vks.

use super::Hash;
use anyhow::Context;
use sp_core::H256;
use std::collections::HashMap;
use std::marker::PhantomData;
use subxt::storage::Storage;
use subxt::{Error, OnlineClient, PolkadotConfig};
pub use zkverify::runtime_types::pallet_verifiers::pallet::VkEntry;
pub use zkverify::storage;

#[subxt::subxt(runtime_metadata_path = "schemas/verifiers.scale")]
pub mod zkverify {}

pub type Api = OnlineClient<PolkadotConfig>;
#[async_trait::async_trait]
pub trait VkFetch<VK> {
    async fn fetch(api: Api, vk_hash: Hash, block: Option<u32>) -> anyhow::Result<Option<VK>>;
}

pub struct VkResolver<VK, R> {
    api: Api,
    cache: HashMap<Hash, VK>,
    _resolver: PhantomData<R>,
}

impl<VK, R> VkResolver<VK, R> {
    pub fn length(&self) -> usize {
        self.cache.len()
    }
}

impl<VK, R> VkResolver<VK, R>
where
    R: VkFetch<VK>,
{
    pub fn new(api: Api) -> Self {
        Self {
            api,
            cache: HashMap::new(),
            _resolver: PhantomData,
        }
    }

    pub async fn from_url(url: impl AsRef<str>) -> Result<Self, Error> {
        Ok(Self::new(OnlineClient::from_url(url).await?))
    }

    pub fn get(&self, vk_hash: &Hash) -> Option<&VK> {
        self.cache.get(vk_hash)
    }

    pub async fn resolve_vk(
        &mut self,
        vk_hash: Hash,
        block: Option<u32>,
    ) -> anyhow::Result<Option<&VK>> {
        if !self.cache.contains_key(&vk_hash) {
            if let Some(vk) = R::fetch(self.api.clone(), vk_hash, block).await? {
                self.cache.insert(vk_hash, vk);
            } else {
                // If the vk doesn't exist means that the submit proof returned an early error: VerificationKeyNotFound
                log::debug!(
                    "VerificationKeyNotFound for vk_hash: 0x{}@{block:?}",
                    hex::encode(vk_hash)
                );
            }
        }
        Ok(self.get(&vk_hash))
    }
}

pub async fn block_hash(api: Api, block_number: u32) -> anyhow::Result<Option<H256>> {
    let address = zkverify::storage().system().block_hash(block_number);
    get_storage(api, None)
        .await?
        .fetch(&address)
        .await
        .with_context(|| "Failed to fetch block hash")
}

pub async fn get_storage(
    api: Api,
    block: Option<H256>,
) -> anyhow::Result<Storage<PolkadotConfig, OnlineClient<PolkadotConfig>>> {
    let storage = api.storage();
    match block {
        Some(block) => Ok(storage.at(block)),
        None => storage
            .at_latest()
            .await
            .with_context(|| "Failed to connect storage"),
    }
}
