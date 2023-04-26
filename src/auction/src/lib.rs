#[fadroma::dsl::contract]
pub mod auction {
    use fadroma::{
        dsl::*,
        core::*,
        scrt::vk::{auth::{self, VkAuth}, ViewingKey},
        killswitch::{self, Killswitch, ContractStatus},
        admin::{self, Admin, Mode},
        storage::{SingleItem, TypedKey, map::InsertOnlyMap},
        cosmwasm_std::{
            self, Response, StdError, Uint128, BankMsg,
            Addr, CanonicalAddr, StdResult, to_binary, coin
        },
        schemars,
        namespace
    };
    use shared::{Auction, Pagination, PaginatedResponse, SaleInfo, SaleStatus};

    namespace!(InfoNs, b"info");
    const INFO: SingleItem<SaleInfo, InfoNs> = SingleItem::new();

    namespace!(HighestBidNs, b"highest_bid");
    const HIGHEST_BID: SingleItem<CanonicalAddr, HighestBidNs> = SingleItem::new();

    namespace!(BiddersNs, b"bidders");
    #[inline]
    fn bidders() -> InsertOnlyMap<
        TypedKey<'static, CanonicalAddr>,
        Uint128,
        BiddersNs
    > {
        InsertOnlyMap::new()
    }

    impl Contract {
        // This runs before executing any messages.
        #[execute_guard]
        pub fn guard(msg: &ExecuteMsg) -> Result<(), StdError> {
            let operational = killswitch::assert_is_operational(deps.as_ref());
    
            // Only allow the killswitch module messages so that we can resume the
            // the contract if it was paused for example.
            // However, if the contract has been set to the "migrating" status,
            // Even the admin cannot reverse that anymore.
            if operational.is_err() && !matches!(msg, ExecuteMsg::SetStatus { .. }) {
                Err(operational.unwrap_err())
            } else {
                Ok(())
            }
        }
    }

    impl Auction for Contract {
        type Error = StdError;

        #[init(entry_wasm)]
        fn new(
            admin: Option<String>,
            name: String,
            end_block: u64
        ) -> Result<Response, <Self as Auction>::Error> {
            admin::init(deps.branch(), admin.as_deref(), &info)?;
            INFO.save(deps.storage, &SaleInfo { name, end_block })?;
    
            Ok(Response::default()
                .set_data(to_binary(&env.contract.address)?)
            )
        }
    
        #[execute]
        fn bid() -> Result<Response, <Self as Auction>::Error> {
            let sale_info = INFO.load_or_error(deps.storage)?;
            if sale_info.end_block < env.block.height {
                return Err(StdError::generic_err("Sale has finished."));
            }

            let sender = info.sender.canonize(deps.api)?;

            let mut bidders = bidders();
            let mut balance = bidders.get_or_default(deps.storage, &sender)?;
            balance += info.funds.into_iter()
                .find(|x| x.denom == "uscrt")
                .map(|x| x.amount)
                .unwrap_or_default();

            bidders.insert(deps.storage, &sender, &balance)?;

            if let Some(addr) = HIGHEST_BID.load(deps.storage)? {
                if addr != sender {
                    let current_highest = bidders.get_or_error(deps.storage, &addr)?;

                    if balance > current_highest {
                        HIGHEST_BID.save(deps.storage, &sender)?;
                    }
                }
            } else {
                // This is the first bid.
                HIGHEST_BID.save(deps.storage, &sender)?;
            };

            Ok(Response::default())
        }
    
        #[execute]
        fn retract_bid() -> Result<Response, <Self as Auction>::Error> {
            let sale_info = INFO.load_or_error(deps.storage)?;
            if sale_info.end_block > env.block.height {
                return Err(StdError::generic_err("Sale hasn't finished yet."));
            }

            let sender = info.sender.as_str().canonize(deps.api)?;
            let highest_bidder = HIGHEST_BID.load_or_error(deps.storage)?;

            if highest_bidder == sender {
                return Err(StdError::generic_err("You have won the sale and cannot retract your bid."));
            }

            let mut bidders = bidders();

            let balance = bidders.get_or_default(deps.storage, &sender)?;
            bidders.insert(deps.storage, &sender, &Uint128::zero())?;

            let send_msg = if balance > Uint128::zero() {
                vec![BankMsg::Send {
                    to_address: info.sender.into_string(),
                    amount: vec![coin(balance.u128(), "uscrt")]
                }]
            } else {
                vec![]
            };

            Ok(Response::default().add_messages(send_msg))
        }

        #[execute]
        #[admin::require_admin]
        fn claim_proceeds() -> Result<Response, <Self as Auction>::Error> {
            let sale_info = INFO.load_or_error(deps.storage)?;
            if sale_info.end_block > env.block.height {
                return Err(StdError::generic_err("Sale hasn't finished yet."));
            }

            let send_msg = if let Some(addr) = HIGHEST_BID.load(deps.storage)? {
                let mut bidders = bidders();

                let balance = bidders.get_or_default(deps.storage, &addr)?;
                bidders.insert(deps.storage, &addr, &Uint128::zero())?;

                vec![BankMsg::Send {
                    to_address: info.sender.into_string(),
                    amount: vec![coin(balance.u128(), "uscrt")]
                }]
            } else {
                // No one made any bids on this sale
                vec![]
            };

            Ok(Response::default().add_messages(send_msg))
        }
    
        #[query]
        fn view_bid(
            address: String,
            key: String
        ) -> Result<Uint128, <Self as Auction>::Error> {
            let address = address.as_str().canonize(deps.api)?;
            auth::authenticate(deps.storage, &ViewingKey::from(key), &address)?;

            bidders().get_or_default(deps.storage, &address)
        }
    
        #[query]
        fn active_bids(
            pagination: Pagination
        ) -> Result<PaginatedResponse<Uint128>, <Self as Auction>::Error> {
            let bidders = bidders().values(deps.storage)?;
            let len = bidders.len();

            let limit = pagination.limit.min(Pagination::LIMIT);
            let iterator = bidders
                .skip(pagination.start as usize)
                .take(limit as usize);

            Ok(PaginatedResponse {
                total: len as u64,
                entries: iterator
                    .into_iter()
                    .collect::<StdResult<Vec<Uint128>>>()?
            })
        }
    
        #[query]
        fn sale_status() -> Result<SaleStatus, <Self as Auction>::Error> {
            let info = INFO.load_or_error(deps.storage)?;

            let current_highest = if let Some(addr) = HIGHEST_BID.load(deps.storage)? {
                bidders().get_or_error(deps.storage, &addr)?
            } else {
                Uint128::zero()
            };

            Ok(SaleStatus {
                current_highest,
                is_finished: info.end_block < env.block.height,
                info
            })
        }
    }

    #[auto_impl(auth::DefaultImpl)]
    impl VkAuth for Contract {
        #[execute]
        fn create_viewing_key(
            entropy: String,
            padding: Option<String>
        ) -> Result<Response, Self::Error> { }

        #[execute]
        fn set_viewing_key(
            key: String,
            padding: Option<String>
        ) -> Result<Response, Self::Error> { }
    }


    #[auto_impl(killswitch::DefaultImpl)]
    impl Killswitch for Contract {
        #[execute]
        fn set_status(
            status: ContractStatus<Addr>,
        ) -> Result<Response, <Self as Killswitch>::Error> { }
    
        #[query]
        fn status() -> Result<ContractStatus<Addr>, <Self as Killswitch>::Error> { }
    }

    #[auto_impl(admin::DefaultImpl)]
    impl Admin for Contract {
        #[execute]
        fn change_admin(mode: Option<Mode>) -> Result<Response, Self::Error> { }
    
        #[query]
        fn admin() -> Result<Option<Addr>, Self::Error> { }
    }
}
