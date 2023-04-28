#[fadroma::dsl::contract]
pub mod factory {
    use fadroma::{
        dsl::*,
        core::*,
        schemars,
        cosmwasm_std::{
            self, Response, StdError, SubMsg, WasmMsg, Binary,
            Reply, CanonicalAddr, Addr, StdResult, to_binary, from_binary
        },
        storage::{iterable::IterableStorage, SingleItem, StaticKey},
        bin_serde::{FadromaSerialize, FadromaDeserialize},
        namespace
    };
    use shared::{
        InstantiateMsg as AuctionInitMsg, SaleInfo,
        Pagination, PaginatedResponse
    };
    use serde::{Serialize, Deserialize};

    namespace!(ContractNs, b"contract");
    const AUCTION_CONTRACT: SingleItem<
        ContractInstantiationInfo,
        ContractNs
    > = SingleItem::new();

    #[derive(Serialize, Deserialize, FadromaSerialize, FadromaDeserialize, Canonize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct AuctionEntry<A> {
        pub contract: ContractLink<A>,
        pub info: SaleInfo
    }

    impl Contract {
        #[init(entry_wasm)]
        pub fn new(auction: ContractInstantiationInfo) -> Result<Response, StdError> {
            AUCTION_CONTRACT.save(deps.storage, &auction)?;

            Ok(Response::default())
        }

        #[execute]
        pub fn create_auction(
            admin: Option<String>,
            name: String,
            end_block: u64
        ) -> Result<Response, StdError> {
            let auction = AUCTION_CONTRACT.load_or_error(deps.storage)?;
            auctions().push(
                deps.storage,
                &AuctionEntry {
                    contract: ContractLink {
                        address: CanonicalAddr(Binary::default()),
                        code_hash: auction.code_hash.clone()
                    },
                    info: SaleInfo {
                        name: name.clone(),
                        end_block
                    }
                }
            )?;

            let label = format!(
                "Auction: {}, started at: {}, ending at {}",
                name,
                env.block.height,
                env.block.height + end_block
            );
        
            let msg = SubMsg::reply_on_success(
                WasmMsg::Instantiate {
                    code_id: auction.id,
                    code_hash: auction.code_hash,
                    msg: to_binary(&AuctionInitMsg { admin, name, end_block })?,
                    funds: vec![],
                    label
                },
                0
            );
        
            Ok(Response::default().add_submessage(msg))
        }

        #[reply]
        pub fn reply(reply: Reply) -> Result<Response, StdError> {
            if reply.id != 0 {
                return Err(StdError::generic_err("Unexpected reply id."));
            }

            let resp = reply.result.unwrap();
            let address: Addr = from_binary(resp.data.as_ref().unwrap())?;

            let auctions = auctions();

            let index = auctions.len(deps.storage)? - 1;
            auctions.update(deps.storage, index, |mut entry| {
                entry.contract.address = address.canonize(deps.api)?;

                Ok(entry)
            })?;

            Ok(Response::default())
        }

        #[query]
        pub fn list_auctions(
            pagination: Pagination
        ) -> Result<PaginatedResponse<AuctionEntry<Addr>>, StdError> {
            let limit = pagination.limit.min(Pagination::LIMIT);

            let auctions = auctions();
            let iterator = auctions
                .iter(deps.storage)?
                .skip(pagination.start as usize)
                .take(limit as usize);

            Ok(PaginatedResponse {
                total: auctions.len(deps.storage)?,
                entries: iterator.into_iter()
                    .map(|x| x?.humanize(deps.api))
                    .collect::<StdResult<Vec<AuctionEntry<Addr>>>>()?
            })
        }
    }

    #[inline]
    fn auctions() -> IterableStorage<
        AuctionEntry<CanonicalAddr>,
        StaticKey
    > {
        IterableStorage::new(StaticKey(b"auctions"))
    }
}
