use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for NodeId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|error| IdError::NodeId(error.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdError {
    #[error("invalid node id: {0}")]
    NodeId(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_node_id_round_trips() {
        let node_id = NodeId::generate();
        let parsed = NodeId::from_str(&node_id.to_string()).expect("parse node id");

        assert_eq!(node_id, parsed);
        assert!(node_id.to_string().contains('-'));
    }

    #[test]
    fn rejects_invalid_node_id() {
        assert!(NodeId::from_str("not-a-node-id").is_err());
    }
}
