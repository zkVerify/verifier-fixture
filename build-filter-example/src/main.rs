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

use crate::ultraplonk::{ProofInfo, VerifyParams, VkOrHash, VkResolver, verify_params};
use anyhow::anyhow;
use clap::Parser;
use helpers::{ExtrinsicsFetcher, fetch_all_proof_extrinsics};
use log::LevelFilter;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use url::Url;
use verifier_fixture::{Filters, FiltersStats, Response};

mod ultraplonk;

#[derive(Parser)]
#[command(about = r#"
An example program that fetch all the ultraplonk proofs between `start` and `end` blocks
create a filter and serialize it. Then deserialize the filter and check if all the proofs
return the expected result."#, long_about = None)]
struct Cli {
    /// Start block number
    #[arg(short = 'S', long, default_value_t = 0)]
    start: u32,
    #[arg(short, long, default_value_t = 700000)]
    /// End block number
    end: u32,
    /// Subscan explorer url
    #[arg(short, long, default_value = "https://zkverify-testnet.api.subscan.io", value_parser = clap::value_parser!(Url))]
    explorer: Url,
    /// Archive RPC url
    #[arg(short, long, default_value = "wss://volta-rpc.zkverify.io", value_parser = clap::value_parser!(Url))]
    rpc: Url,
    /// Subsquid endpoint to query and get all proofs "coordinates" (block-extrinsic_id)
    #[arg(short, long, default_value = "https://zkpvnetwork.squids.live/zkverify@v3/api/graphql", value_parser = clap::value_parser!(Url))]
    subsquid: Url,
    /// Subscan query page size (proof per query)
    #[arg(short, long, default_value_t = 1000)]
    page_size: usize,
    /// Output file path: if it's provided, save the tiler binary to this file
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_default_env()
        .filter_module("crate", LevelFilter::Info)
        .init();

    log::info!(
        "Started: [{}..={}] blocks page size = {}",
        cli.start,
        cli.end,
        cli.page_size
    );
    log::info!("Fetching submitted proofs coordinates [{}]", cli.subsquid);
    let extrinsics = fetch_all_proof_extrinsics(
        &cli.subsquid,
        "SettlementUltraplonkPallet",
        cli.start,
        cli.end,
    )
    .await?;
    log::info!("Fetching proofs params [{}]", cli.explorer);
    let proofs = ExtrinsicsFetcher::new(cli.explorer.clone(), cli.page_size)
        .fetch::<ProofInfo>(Box::new(extrinsics.into_iter()))
        .await?;
    log::info!("Fetched all {} proofs params", proofs.len());

    let mut vk_resolver = VkResolver::from_url(&cli.rpc).await?;
    log::info!("Fetching vks... [{}]", cli.rpc);
    // Fill the resolver with vks...
    for p in proofs.iter() {
        if let VkOrHash::Hash(hash) = &p.proof_data.vk {
            let _vk = vk_resolver
                .resolve_vk(*hash, Some(p.extrinsic_index.block_number.try_into()?))
                .await?;
        }
    }
    log::info!("Available  vks = {}", vk_resolver.length());
    let vk_resolver = vk_resolver;
    log::info!("Verifying proofs... [{}]", proofs.len());
    let mut last_announced = 0;
    let mut response_set = HashSet::new();
    let mut data = Vec::new();

    let proofs_tot = proofs.len();

    for (id, p) in proofs.into_iter().enumerate() {
        if id > last_announced + proofs_tot / 20 {
            log::info!("Computed {}/{} proofs", id, proofs_tot);
            last_announced = id;
        }
        let ProofInfo {
            proof_data,
            extrinsic_index,
        } = p;
        if let Some(params) = verify_params(&vk_resolver, proof_data) {
            if response_set.contains(&params) {
                log::debug!("Duplicate proof at {}", extrinsic_index);
                continue;
            }
            let result: Response = ultraplonk::verify_proof(&params);
            let params = Rc::new(params);
            response_set.insert(params.clone());
            data.push((params.clone(), result));
        }
    }

    log::info!("Build the filter");
    let filters = Filters::<VerifyParams>::try_from(data.clone())
        .map_err(|e| anyhow!("Invalid config filter: {e}"))?;
    let FiltersStats {
        filtered,
        exceptions,
    } = filters.stats();
    for (response, size) in filtered {
        log::info!("Response: {:?} size: {}", response, size);
    }
    log::info!("Exceptions size: {exceptions}");

    log::info!("Serializing the filter");
    let serialized = filters.serialize()?;

    if let Some(path) = cli.output {
        log::info!("Write the filter to {}", path.display());
        std::fs::write(path, &serialized)?;
    }

    log::info!("Deserialize the filter");
    let decoded = Filters::<VerifyParams>::deserialize(&serialized)?;

    log::info!("Check");
    for (params, response) in &data {
        assert_eq!(response, &decoded.response(params.as_ref()).unwrap())
    }

    log::info!("All done!!!!");
    Ok(())
}
