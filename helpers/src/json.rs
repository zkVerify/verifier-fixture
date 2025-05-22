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

use anyhow::bail;
use serde_json::{Map, Number, Value};

pub trait TakeJType {
    fn null(self) -> anyhow::Result<()>;
    fn bool(self) -> anyhow::Result<bool>;
    fn number(self) -> anyhow::Result<Number>;
    fn string(self) -> anyhow::Result<String>;
    fn array(self) -> anyhow::Result<Vec<Value>>;
    fn object(self) -> anyhow::Result<Map<String, Value>>;
}

impl TakeJType for Value {
    fn null(self) -> anyhow::Result<()> {
        if !matches!(self, Value::Null) {
            bail!("Expected null value")
        }
        Ok(())
    }

    fn bool(self) -> anyhow::Result<bool> {
        match self {
            Value::Bool(b) => Ok(b),
            _ => bail!("Expected boolean value"),
        }
    }

    fn number(self) -> anyhow::Result<Number> {
        match self {
            Value::Number(n) => Ok(n),
            _ => bail!("Expected number value"),
        }
    }

    fn string(self) -> anyhow::Result<String> {
        match self {
            Value::String(s) => Ok(s),
            _ => bail!("Expected string value"),
        }
    }

    fn array(self) -> anyhow::Result<Vec<Value>> {
        match self {
            Value::Array(a) => Ok(a),
            _ => bail!("Expected array value"),
        }
    }

    fn object(self) -> anyhow::Result<Map<String, Value>> {
        match self {
            Value::Object(o) => Ok(o),
            _ => bail!("Expected object value"),
        }
    }
}

pub trait TryFromJson: Sized {
    fn try_from_json(value: Value) -> anyhow::Result<Self>;
}

pub trait TryFromParam: Sized {
    fn try_from_param(value: Value) -> anyhow::Result<Self>;
}

impl<T: TryFromJson> TryFromParam for T {
    fn try_from_param(value: Value) -> anyhow::Result<Self> {
        let value = value
            .object()?
            .remove("value")
            .ok_or_else(|| anyhow::anyhow!("Missing 'value' field in input"))?;
        Self::try_from_json(value)
    }
}

impl TryFromJson for [u8; 32] {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let inner = value.string()?;
        if !inner.starts_with("0x") {
            bail!("Invalid slice format: should starts with '0x'");
        }

        Ok(hex::decode(&inner[2..])
            .map_err(|_| anyhow::anyhow!("Invalid slice"))?
            .as_slice()
            .try_into()?)
    }
}

impl TryFromJson for Vec<u8> {
    fn try_from_json(value: Value) -> anyhow::Result<Self> {
        let inner = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Expected a string"))?;
        if !inner.starts_with("0x") {
            return Err(anyhow::anyhow!(
                "Invalid proof format: should starts with '0x'"
            ))?;
        }
        hex::decode(&inner[2..]).map_err(|_| anyhow::anyhow!("Invalid vk"))
    }
}
