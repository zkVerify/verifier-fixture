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

//! Provide `Filters` that leverage on Bloom filter to map hashable to `Response`.
//! `Response` is a possible verification result.

use bloomfilter::Bloom;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(VerifyError),
}

impl Response {
    fn all() -> impl Iterator<Item = Self> {
        Some(Response::Ok)
            .into_iter()
            .chain(enum_iterator::all::<VerifyError>().map(Self::Error))
    }
}

impl<T> From<Result<T, hp_verifiers::VerifyError>> for Response {
    fn from(value: Result<T, hp_verifiers::VerifyError>) -> Self {
        match value {
            Ok(_) => Response::Ok,
            Err(e) => Response::Error(e.into()),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Sequence)]
pub enum VerifyError {
    InvalidInput,
    VerifyError,
    InvalidProofData,
    InvalidVerificationKey,
}

impl From<hp_verifiers::VerifyError> for VerifyError {
    fn from(e: hp_verifiers::VerifyError) -> Self {
        match e {
            hp_verifiers::VerifyError::InvalidInput => VerifyError::InvalidInput,
            hp_verifiers::VerifyError::InvalidProofData => VerifyError::InvalidProofData,
            hp_verifiers::VerifyError::VerifyError => VerifyError::VerifyError,
            hp_verifiers::VerifyError::InvalidVerificationKey => {
                VerifyError::InvalidVerificationKey
            }
        }
    }
}

impl<T: Hash> From<&Filters<T>> for SerializableFilters {
    fn from(value: &Filters<T>) -> Self {
        let mut data = HashMap::new();
        for (response, params) in &value.data {
            data.insert(*response, params.to_bytes());
        }
        Self {
            data,
            exceptions: value.exceptions.clone(),
        }
    }
}

impl<T: Hash> TryFrom<SerializableFilters> for Filters<T> {
    type Error = FilterError;

    fn try_from(value: SerializableFilters) -> Result<Self, Self::Error> {
        let mut data = HashMap::new();
        for (response, bytes) in value.data {
            data.insert(
                response,
                Bloom::<T>::from_bytes(bytes).map_err(FilterError::DeserializeFilters)?,
            );
        }
        Ok(Self {
            data,
            exceptions: value.exceptions,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SerializableFilters {
    data: HashMap<Response, Vec<u8>>,
    exceptions: HashMap<u64, Response>,
}

pub struct Filters<T: Hash> {
    data: HashMap<Response, Bloom<T>>,
    exceptions: HashMap<u64, Response>,
}

/// Create a lazy `Filters` from the given bytes
/// ```
///     # use verifier_fixture::{once, Filters, Response};
///     # #[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Deserialize)]
///     # struct ProofData {
///     #     inner: u128,
///     # }
///     # let (results, _): (Vec<(ProofData, Response)>, _) = bincode::serde::decode_from_slice(
///     #     &std::fs::read("src/resources/once_verifier_results.bin").unwrap(),
///     #     bincode::config::standard(),
///     # ).unwrap();
///     let local: core::cell::LazyCell<Filters<ProofData>> =
///         once!(include_bytes!("resources/once_verifier.bin"));
///
///     for result in results.iter() {
///         let (params, response) = result;
///         assert_eq!(local.response(params), Some(*response),);
///     }
/// ```
#[macro_export]
macro_rules! once {
    ($bytes:expr) => {
        core::cell::LazyCell::new(|| {
            Filters::deserialize($bytes).expect("Failed to deserialize filters")
        })
    };
}

/// Create a lazy static `Filters` from the given bytes
/// ```
///     # use verifier_fixture::{once_lock, Filters, Response};
///     # #[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Deserialize)]
///     # struct ProofData {
///     #     inner: u128,
///     # }
///     # let (results, _): (Vec<(ProofData, Response)>, _) = bincode::serde::decode_from_slice(
///     #     &std::fs::read("src/resources/once_verifier_results.bin").unwrap(),
///     #     bincode::config::standard(),
///     # ).unwrap();
///     static GLOBAL: std::sync::LazyLock<Filters<ProofData>> =
///         once_lock!(include_bytes!("resources/once_verifier.bin"));
///
///     for result in results.iter() {
///         let (params, response) = result;
///         assert_eq!(GLOBAL.response(params), Some(*response),);
///     }
/// ```
#[macro_export]
macro_rules! once_lock {
    ($bytes:expr) => {
        std::sync::LazyLock::new(|| {
            Filters::deserialize($bytes).expect("Failed to deserialize filters")
        })
    };
}

#[derive(thiserror::Error, Debug)]
pub enum FilterError {
    #[error("Cannot encode filter: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("Cannot decode filter: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("Invalid bytes: {0}")]
    DeserializeFilters(&'static str),

    #[error("Filter initialization: {0}")]
    InitFilters(&'static str),

    #[error("Filter initialization: find an unresolvable exception")]
    IncoherentResponses,
}

impl<T: Hash> Filters<T> {
    /// Serialize the filter in binary format
    pub fn serialize(&self) -> Result<Vec<u8>, FilterError> {
        let serialized = SerializableFilters::from(self);

        bincode::serde::encode_to_vec(&serialized, bincode::config::standard())
            .map_err(FilterError::Encode)
    }

    /// Deserialize the filter from binary format
    pub fn deserialize(data: &[u8]) -> Result<Self, FilterError> {
        let (serialized, _): (SerializableFilters, _) =
            bincode::serde::decode_from_slice(data, bincode::config::standard())?;
        serialized.try_into()
    }
}

/// `Filters` statistics
pub struct FiltersStats {
    /// The number of entries per response
    pub filtered: Vec<(Response, usize)>,
    /// The number of exceptions
    pub exceptions: usize,
}

impl<T: Hash> Filters<T> {
    /// Return the registered response for the given parameters if any
    pub fn response(&self, params: &T) -> Option<Response> {
        if let Some(response) = self.exception(params) {
            return Some(*response);
        }
        self.response_filter(params)
    }

    /// Return the registered response for the given parameters if any
    pub fn stats(&self) -> FiltersStats {
        FiltersStats {
            filtered: Response::all()
                .map(|response| {
                    (
                        response,
                        self.data
                            .get(&response)
                            .map(|v| v.len() as usize)
                            .unwrap_or_default(),
                    )
                })
                .collect(),
            exceptions: self.exceptions.len(),
        }
    }

    fn response_filter(&self, params: &T) -> Option<Response> {
        for response in Response::all() {
            if let Some(true) = self.data.get(&response).map(|v| v.check(params)) {
                return Some(response);
            }
        }
        None
    }

    fn exception(&self, params: &T) -> Option<&Response> {
        self.exceptions.get(&self.exception_hash(params))
    }

    fn exception_hash(&self, params: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::default();
        params.hash(&mut hasher);
        hasher.finish()
    }

    fn add_exception(&mut self, response: Response, params: &T) -> Result<(), FilterError> {
        let hash = self.exception_hash(params);
        if matches!(self.exceptions.get(&hash), Some(r) if r != &response) {
            return Err(FilterError::IncoherentResponses);
        }
        self.exceptions.insert(hash, response);
        Ok(())
    }
}

impl<T: Hash, R: AsRef<T> + 'static> TryFrom<Box<dyn Iterator<Item = (R, Response)>>>
    for Filters<T>
{
    type Error = FilterError;

    fn try_from(it: Box<dyn Iterator<Item = (R, Response)>>) -> Result<Self, Self::Error> {
        let mut map: HashMap<_, Vec<_>> = HashMap::new();
        for (data, response) in it {
            map.entry(response).or_default().push(data);
        }
        map.try_into()
    }
}

impl<T: Hash, R: AsRef<T> + 'static> TryFrom<Vec<(R, Response)>> for Filters<T> {
    type Error = FilterError;

    fn try_from(v: Vec<(R, Response)>) -> Result<Self, Self::Error> {
        (Box::new(v.into_iter()) as Box<dyn Iterator<Item = (R, Response)>>).try_into()
    }
}

impl<T: Hash, R: AsRef<T>> TryFrom<HashMap<Response, Vec<R>>> for Filters<T> {
    type Error = FilterError;

    fn try_from(value: HashMap<Response, Vec<R>>) -> Result<Self, Self::Error> {
        let mut filter = Filters {
            data: value
                .iter()
                .map(|(response, params)| {
                    Ok((*response, Bloom::new_for_fp_rate(params.len(), 0.001)?))
                })
                .collect::<Result<_, _>>()
                .map_err(FilterError::InitFilters)?,
            exceptions: HashMap::new(),
        };

        for response in Response::all() {
            if let Some(params) = value.get(&response) {
                for params in params {
                    let params = params.as_ref();
                    if matches!(filter.response_filter(params), Some(r) if r != response) {
                        // That's an exception
                        filter.add_exception(response, params)?;
                    } else {
                        filter
                            .data
                            .get_mut(&response)
                            .expect("All responses should be already mapped. qed")
                            .set(params);
                    }
                }
            }
        }
        Ok(filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use rand_distr::Distribution;
    use rand_distr::weighted::WeightedIndex;
    use std::sync::LazyLock;

    #[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
    struct ProofData {
        inner: u128,
    }

    impl AsRef<ProofData> for ProofData {
        fn as_ref(&self) -> &ProofData {
            self
        }
    }

    impl ProofData {
        fn new(inner: u128) -> Self {
            ProofData { inner }
        }
    }

    fn generate_set_of_proof_results(size: usize) -> Vec<(ProofData, Response)> {
        let choices = [
            (Response::Ok, 0.99),
            (Response::Error(VerifyError::InvalidVerificationKey), 0.004),
            (Response::Error(VerifyError::VerifyError), 0.004),
            (Response::Error(VerifyError::InvalidInput), 0.001),
            (Response::Error(VerifyError::InvalidProofData), 0.001),
        ];
        let distribution = WeightedIndex::new(choices.iter().map(|item| item.1)).unwrap();
        let mut rng = rand::rng();
        (0..size)
            .map(|_| {
                (
                    ProofData::new(rng.random()),
                    choices[distribution.sample(&mut rng)].0,
                )
            })
            .collect()
    }

    #[test]
    fn crate_a_filter_from_iterator_of_proof_results() {
        let results = generate_set_of_proof_results(1000000);

        let filter = Filters::<ProofData>::try_from(Box::new(results.clone().into_iter())
            as Box<dyn Iterator<Item = (ProofData, Response)>>)
        .unwrap();

        for result in results.iter() {
            let (params, response) = result;
            assert_eq!(filter.response(params), Some(*response),);
        }
    }

    #[test]
    fn serialize_deserialize() {
        let results = generate_set_of_proof_results(10000);

        let filter = Filters::<ProofData>::try_from(Box::new(results.clone().into_iter())
            as Box<dyn Iterator<Item = (ProofData, Response)>>)
        .unwrap();

        let serialized = filter.serialize().unwrap();

        let deserialized = Filters::<ProofData>::deserialize(&serialized).unwrap();

        for result in results.iter() {
            let (params, response) = result;
            assert_eq!(deserialized.response(params), Some(*response),);
        }
    }

    #[test]
    fn serialized_size() {
        let results = generate_set_of_proof_results(1000000);

        let filter = Filters::<ProofData>::try_from(Box::new(results.clone().into_iter())
            as Box<dyn Iterator<Item = (ProofData, Response)>>)
        .unwrap();

        let serialized = filter.serialize().unwrap();

        assert!((serialized.len()) < 2000000); // The serialized filter should be significantly smaller than the original one
    }

    #[test]
    fn once_verifier() {
        let (results, _): (Vec<(ProofData, Response)>, _) = bincode::serde::decode_from_slice(
            &std::fs::read("src/resources/once_verifier_results.bin").unwrap(),
            bincode::config::standard(),
        )
        .unwrap();

        let global = once!(include_bytes!("resources/once_verifier.bin"));

        for result in results.iter() {
            let (params, response) = result;
            assert_eq!(global.response(params), Some(*response),);
        }
    }

    #[test]
    fn once_lock_verifier() {
        let (results, _): (Vec<(ProofData, Response)>, _) = bincode::serde::decode_from_slice(
            &std::fs::read("src/resources/once_verifier_results.bin").unwrap(),
            bincode::config::standard(),
        )
        .unwrap();

        static GLOBAL: LazyLock<Filters<ProofData>> =
            once_lock!(include_bytes!("resources/once_verifier.bin"));

        for result in results.iter() {
            let (params, response) = result;
            assert_eq!(GLOBAL.response(params), Some(*response),);
        }
    }
}
