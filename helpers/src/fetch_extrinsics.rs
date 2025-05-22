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

use super::{Extrinsic, TakeJType, TryFromJson};
use anyhow::Context;
use serde_json::json;
use url::Url;

pub struct ExtrinsicsFetcher {
    explorer: Url,
    page_size: usize,
}

impl ExtrinsicsFetcher {
    pub fn new(explorer: Url, page_size: usize) -> Self {
        Self {
            explorer,
            page_size,
        }
    }

    pub async fn fetch<T: TryFromJson>(
        &self,
        extrinsics: Box<dyn Iterator<Item = Extrinsic>>,
    ) -> anyhow::Result<Vec<T>> {
        let extrinsics = extrinsics.collect::<Vec<_>>();
        let pages = extrinsics.chunks(self.page_size);

        let mut results = Vec::new();

        for page in pages {
            let proofs = self.fetch_page::<T>(page).await?;
            results.extend(proofs);
        }
        Ok(results)
    }

    async fn fetch_page<T: TryFromJson>(&self, page: &[Extrinsic]) -> anyhow::Result<Vec<T>> {
        self.fetch_json_data(page)
            .await
            .with_context(|| "Failed to fetch extrinsics")?
            .object()?
            .remove("data")
            .ok_or_else(|| anyhow::anyhow!("Expected data field"))?
            .array()?
            .into_iter()
            .map(T::try_from_json)
            .collect::<anyhow::Result<Vec<_>>>()
    }

    async fn fetch_json_data(&self, extrinsics: &[Extrinsic]) -> anyhow::Result<serde_json::Value> {
        let extrinsic_indexes = extrinsics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let url = self
            .explorer
            .join("/api/scan/extrinsic/params")
            .with_context(|| "Wrong url")?;
        surf::post(url)
            .body_json(&json!({
                "extrinsic_index": extrinsic_indexes
            }))
            .map_err(surf::Error::into_inner)?
            .await
            .map_err(surf::Error::into_inner)?
            .body_json()
            .await
            .map_err(surf::Error::into_inner)
    }
}
