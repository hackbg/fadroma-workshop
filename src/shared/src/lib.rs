use fadroma::{
    dsl::*,
    schemars,
    cosmwasm_std::{self, Response, Uint128},
    bin_serde::{FadromaSerialize, FadromaDeserialize},
    killswitch::Killswitch,
    scrt::vk::auth::VkAuth,
    impl_canonize_default
};
use serde::{Serialize, Deserialize};

#[interface]
pub trait Auction: Killswitch + VkAuth {
    type Error: std::fmt::Display;

    #[init]
    fn new(
        admin: Option<String>,
        name: String,
        end_block: u64
    ) -> Result<Response, <Self as Auction>::Error>;

    #[execute]
    fn bid() -> Result<Response, <Self as Auction>::Error>;

    #[execute]
    fn retract_bid() -> Result<Response, <Self as Auction>::Error>;

    #[execute]
    fn claim_proceeds() -> Result<Response, <Self as Auction>::Error>;

    #[query]
    fn view_bid(
        address: String,
        key: String
    ) -> Result<Uint128, <Self as Auction>::Error>;

    #[query]
    fn active_bids(
        pagination: Pagination
    ) -> Result<PaginatedResponse<Uint128>, <Self as Auction>::Error>;

    #[query]
    fn sale_status() -> Result<SaleStatus, <Self as Auction>::Error>;
}

#[derive(Serialize, Deserialize, FadromaSerialize, FadromaDeserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub struct SaleInfo {
    pub name: String,
    pub end_block: u64
}

impl_canonize_default!(SaleInfo);

#[derive(Serialize, Deserialize, FadromaSerialize, FadromaDeserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub struct SaleStatus {
    pub info: SaleInfo,
    pub current_highest: Uint128,
    pub is_finished: bool
}

#[derive(Serialize, Deserialize, schemars::JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Pagination {
    pub start: u64,
    pub limit: u8
}

#[derive(Serialize, Deserialize, schemars::JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct PaginatedResponse<T: Serialize> {
    pub entries: Vec<T>,
    pub total: u64
}

impl Pagination {
    pub const LIMIT: u8 = 30;
}
