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

//! Helper functions, traits and structs to fetch and deserialize stuff.
//! Function to fetch all `submit_proof` estrinsics ID of a given pallet,
//! a fetcher to get the estrinsic data from the explorer, some json utils to
//! parse the explorer output and finally an interface for the zkverify node (archive).
//!

pub use fetch_extrinsics::ExtrinsicsFetcher;
pub use json::{TakeJType, TryFromJson, TryFromParam};
pub use squid::{Extrinsic, fetch_all_proof_extrinsics};
pub type Hash = [u8; 32];

mod fetch_extrinsics;
mod json;
mod squid;
pub mod zkverify_node;
