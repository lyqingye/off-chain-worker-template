pub mod chain;
pub mod config;
pub mod error;
pub mod events;
pub mod keyring;
pub mod macros;
pub mod subscribe;
pub mod util;

/// prost definition
pub const COSMOS_SDK_VERSION: &str = "7648bfca45b9d0897103ec739210607dce77c4fb";
pub mod cosmos {
    pub mod auth {
        pub mod v1beta1 {
            include!("prost/cosmos.auth.v1beta1.rs");
        }
    }
    pub mod staking {
        pub mod v1beta1 {
            include!("prost/cosmos.staking.v1beta1.rs");
        }
    }
    pub mod base {
        pub mod abci {
            pub mod v1beta1 {
                include!("prost/cosmos.base.abci.v1beta1.rs");
            }
        }
        pub mod kv {
            pub mod v1beta1 {
                include!("prost/cosmos.base.kv.v1beta1.rs");
            }
        }
        pub mod query {
            pub mod v1beta1 {
                include!("prost/cosmos.base.query.v1beta1.rs");
            }

            pub mod pagination {
                use super::v1beta1::PageRequest;

                pub fn all() -> Option<PageRequest> {
                    Some(PageRequest {
                        limit: u64::MAX,
                        ..Default::default()
                    })
                }
            }
        }
        pub mod reflection {
            pub mod v1beta1 {
                include!("prost/cosmos.base.reflection.v1beta1.rs");
            }
        }
        pub mod store {
            pub mod v1beta1 {
                include!("prost/cosmos.base.store.v1beta1.rs");
            }
        }
        pub mod v1beta1 {
            include!("prost/cosmos.base.v1beta1.rs");
        }
        pub mod tendermint {
            pub mod v1beta1 {
                include!("prost/cosmos.base.tendermint.v1beta1.rs");
            }
        }
    }
    pub mod crypto {
        pub mod multisig {
            pub mod v1beta1 {
                include!("prost/cosmos.crypto.multisig.v1beta1.rs");
            }
        }
    }
    pub mod tx {
        pub mod signing {
            pub mod v1beta1 {
                include!("prost/cosmos.tx.signing.v1beta1.rs");
            }
        }
        pub mod v1beta1 {
            include!("prost/cosmos.tx.v1beta1.rs");
        }
    }
    pub mod upgrade {
        pub mod v1beta1 {
            include!("prost/cosmos.upgrade.v1beta1.rs");
        }
    }
    pub mod gov {
        pub mod v1beta1 {
            include!("prost/cosmos.gov.v1beta1.rs");
        }
    }
}

pub mod ibc {
    pub mod apps {
        pub mod transfer {
            pub mod v1 {
                include!("prost/ibc.applications.transfer.v1.rs");
            }
        }
    }
    pub mod core {
        pub mod channel {
            pub mod v1 {
                include!("prost/ibc.core.channel.v1.rs");
            }
        }
        pub mod client {
            pub mod v1 {
                include!("prost/ibc.core.client.v1.rs");
            }
        }
        pub mod commitment {
            pub mod v1 {
                include!("prost/ibc.core.commitment.v1.rs");
            }
        }
        pub mod connection {
            pub mod v1 {
                include!("prost/ibc.core.connection.v1.rs");
            }
        }
        pub mod types {
            pub mod v1 {
                include!("prost/ibc.core.types.v1.rs");
            }
        }
    }
    pub mod lightclients {
        pub mod localhost {
            pub mod v1 {
                include!("prost/ibc.lightclients.localhost.v1.rs");
            }
        }
        pub mod solomachine {
            pub mod v1 {
                include!("prost/ibc.lightclients.solomachine.v1.rs");
            }
        }
        pub mod tendermint {
            pub mod v1 {
                include!("prost/ibc.lightclients.tendermint.v1.rs");
            }
        }
    }
    pub mod mock {
        include!("prost/ibc.mock.rs");
    }
}

pub mod ics23 {
    include!("prost/ics23.rs");
}
