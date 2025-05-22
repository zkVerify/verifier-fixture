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

use anyhow::Context;
use cynic::QueryBuilder;
use cynic::http::SurfExt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[cynic::schema("zkverify")]
mod schema {}

#[derive(cynic::QueryVariables, Debug)]
pub struct SubmittedProofsVariables<'a> {
    pub greater_than_or_equal_to: Option<i32>,
    pub less_than_or_equal_to: Option<i32>,
    pub pallet: &'a str,
}

pub async fn fetch_all_proof_extrinsics(
    uri: impl AsRef<str>,
    pallet: &str,
    start: u32,
    end: u32,
) -> anyhow::Result<Box<dyn Iterator<Item = Extrinsic>>> // This type is defined in the schema.graphql file
{
    let operation = SubmittedProofs::build(SubmittedProofsVariables {
        greater_than_or_equal_to: Some(start as i32),
        less_than_or_equal_to: Some(end as i32),
        pallet,
    });
    Ok(Box::new(
        surf::post(uri)
            .run_graphql(operation)
            .await
            .map_err(surf::Error::into_inner)
            .with_context(|| "Failed to execute GraphQL query")?
            .data
            .with_context(|| "Failed to extract data from GraphQL response")?
            .extrinsics
            .with_context(|| "Failed to get extrinsics connections from GraphQL response")?
            .edges
            .into_iter()
            .filter_map(|edge| edge.node),
    ))
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SubmittedProofsVariables")]
pub struct SubmittedProofs {
    #[arguments(condition: { call: "submit_proof", pallet: $pallet }, filter: { blockNumber: { greaterThanOrEqualTo: $greater_than_or_equal_to, lessThanOrEqualTo: $less_than_or_equal_to } }, orderBy: "BLOCK_NUMBER_ASC"
    )]
    pub extrinsics: Option<ExtrinsicsConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ExtrinsicsConnection {
    pub edges: Vec<ExtrinsicsEdge>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ExtrinsicsEdge {
    pub node: Option<Extrinsic>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
pub struct Extrinsic {
    pub extrinsic_idx: i32,
    pub block_number: i32,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum ExtrinsicsOrderBy {
    Natural,
    IdAsc,
    IdDesc,
    BlockNumberAsc,
    BlockNumberDesc,
    PalletAsc,
    PalletDesc,
    CallAsc,
    CallDesc,
    SuccessAsc,
    SuccessDesc,
    TimestampAsc,
    TimestampDesc,
    ExtrinsicIdxAsc,
    ExtrinsicIdxDesc,
    AddressAsc,
    AddressDesc,
    HashAsc,
    HashDesc,
    PrimaryKeyAsc,
    PrimaryKeyDesc,
}

impl Display for Extrinsic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.block_number, self.extrinsic_idx)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseExtrinsicError;

impl Display for ParseExtrinsicError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid extrinsic ID format")
    }
}

impl std::error::Error for ParseExtrinsicError {}

impl FromStr for Extrinsic {
    type Err = ParseExtrinsicError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let components = s
            .split('-')
            .map(|seg| seg.parse())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ParseExtrinsicError)?;
        if components.len() != 2 {
            return Err(ParseExtrinsicError);
        }
        Ok(Extrinsic {
            block_number: components[0],
            extrinsic_idx: components[1],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitted_proofs_output() {
        use cynic::QueryBuilder;

        let operation = SubmittedProofs::build(SubmittedProofsVariables {
            greater_than_or_equal_to: Some(0),
            less_than_or_equal_to: Some(700000),
            pallet: "SettlementUltraplonkPallet",
        });

        insta::assert_snapshot!(operation.query);
    }
}
