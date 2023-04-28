use fadroma::{
    core::*,
    ensemble::{
        ContractEnsemble, ContractHarness,
        MockEnv, EnsembleResult, AnyResult
    },
    cosmwasm_std::{
        DepsMut, Deps, Env, MessageInfo, Addr,
        Response, Binary, Reply, Uint128, from_binary, coin
    },
    tokens::one_token,
    impl_contract_harness
};
use ::factory::factory::{self, AuctionEntry};
use auction::auction;
use shared::{Pagination, PaginatedResponse, SaleStatus};

const FACTORY: &str = "factory";
const ADMIN: &str = "admin";

impl_contract_harness!(Auction, auction);

struct Factory;

impl ContractHarness for Factory {
    fn instantiate(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: Binary
    ) -> AnyResult<Response> {
        let resp = factory::instantiate(deps, env, info, from_binary(&msg)?)?;

        Ok(resp)
    }

    fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: Binary
    ) -> AnyResult<Response> {
        let resp = factory::execute(deps, env, info, from_binary(&msg)?)?;

        Ok(resp)
    }

    fn query(
        &self,
        deps: Deps,
        env: Env,
        msg: Binary
    ) -> AnyResult<Binary> {
        let resp = factory::query(deps, env, from_binary(&msg)?)?;

        Ok(resp)
    }

    fn reply(&self, deps: DepsMut, env: Env, reply: Reply) -> AnyResult<Response> {
        let resp = factory::Contract::reply(deps, env, reply)?;

        Ok(resp)
    }
}

struct Suite {
    ensemble: ContractEnsemble,
    factory: ContractLink<Addr>
}

impl Suite {
    fn new() -> Self {
        let mut ensemble = ContractEnsemble::new();

        // Upload contracts
        let auction = ensemble.register(Box::new(Auction));
        let factory = ensemble.register(Box::new(Factory));

        // Instantiate factory
        let factory = ensemble.instantiate(
            factory.id,
            &factory::InstantiateMsg { auction },
            MockEnv::new("sender", FACTORY)
        )
        .unwrap()
        .instance;

        Self { ensemble, factory }
    }

    fn new_auction(&mut self, end_block: u64) -> EnsembleResult<AuctionEntry<Addr>> {
        self.ensemble.execute(
            &factory::ExecuteMsg::CreateAuction {
                admin: Some(ADMIN.into()),
                name: "Road 23".into(),
                end_block
            },
            MockEnv::new("sender", self.factory.address.clone())
        )?;

        let auctions: PaginatedResponse<AuctionEntry<Addr>> = self.ensemble.query(
            &self.factory.address,
            &factory::QueryMsg::ListAuctions {
                pagination: Pagination {
                    start: 0,
                    limit: 30
                }
            }
        )?;

        Ok(auctions.entries.into_iter().rev().last().unwrap())
    }
}

#[test]
fn instantiate_auction() {
    let mut suite = Suite::new();
    let block = suite.ensemble.block().height + 1000;

    let auction = suite.new_auction(block).unwrap();
    assert_eq!(auction.info.name, "Road 23");
    assert_eq!(auction.info.end_block, block);

    let status: SaleStatus = suite.ensemble.query(
        &auction.contract.address,
        &auction::QueryMsg::SaleStatus { }
    ).unwrap();

    assert_eq!(status.info.name, "Road 23");
    assert_eq!(status.info.end_block, block);
    assert_eq!(status.current_highest, Uint128::zero());
    assert_eq!(status.is_finished, false);

    let admin: Option<Addr> = suite.ensemble.query(
        &auction.contract.address,
        &auction::QueryMsg::Admin { }
    ).unwrap();

    assert_eq!(admin, Some(Addr::unchecked(ADMIN)));
}

#[test]
fn cannot_instantiate_auction_with_end_block_in_the_past() {
    let mut suite = Suite::new();
    let block = suite.ensemble.block().height;

    let err = suite.new_auction(block).unwrap_err();
    assert_eq!(
        err.unwrap_contract_error().to_string(),
        "Generic error: End block has already passed."
    );
}

#[test]
fn bidding() {
    let mut suite = Suite::new();
    let block = suite.ensemble.block().height + 1000;

    let auction = suite.new_auction(block).unwrap().contract;

    let bidder = "bidder";
    let vk = "bidder_vk";
    let bid_amount = one_token(6) * 100;

    // Simulate the bidder having the needed amount of uscrt on chain.
    // If you comment out this line, you will see an error about the
    // bidder not having sufficient balance to send to the auction contract.
    suite.ensemble.add_funds(bidder, vec![coin(bid_amount, "uscrt")]);

    suite.ensemble.execute(
        &auction::ExecuteMsg::Bid { },
        MockEnv::new(bidder, &auction.address)
            .sent_funds(vec![coin(bid_amount, "uscrt")])
    ).unwrap();

    let status: SaleStatus = suite.ensemble.query(
        &auction.address,
        &auction::QueryMsg::SaleStatus { }
    ).unwrap();

    assert_eq!(status.current_highest.u128(), bid_amount);

    // We check that the auction contract has indeed received the uscrt
    // sent by the bidder
    let auction_balances = suite.ensemble.balances(&auction.address).unwrap();
    assert_eq!(auction_balances["uscrt"].u128(), bid_amount);

    suite.ensemble.execute(
        &auction::ExecuteMsg::SetViewingKey {
            key: vk.into(),
            padding: None
        },
        MockEnv::new(bidder, &auction.address)
    ).unwrap();

    let stored_amount: Uint128 = suite.ensemble.query(
        &auction.address,
        &auction::QueryMsg::ViewBid {
            address: bidder.into(),
            key: vk.into()
        }
    ).unwrap();

    assert_eq!(stored_amount.u128(), bid_amount);
}

#[test]
fn cannot_retract_bid_before_the_end_or_if_winner() {
    let mut suite = Suite::new();
    let block = suite.ensemble.block().height + 1000;

    let auction = suite.new_auction(block).unwrap().contract;

    let bidder = "bidder";
    let bid_amount = one_token(6) * 100;

    suite.ensemble.add_funds(bidder, vec![coin(bid_amount, "uscrt")]);
    suite.ensemble.execute(
        &auction::ExecuteMsg::Bid { },
        MockEnv::new(bidder, &auction.address)
            .sent_funds(vec![coin(bid_amount, "uscrt")])
    ).unwrap();

    let err = suite.ensemble.execute(
        &auction::ExecuteMsg::RetractBid { },
        MockEnv::new(bidder, &auction.address)
    ).unwrap_err();

    assert_eq!(
        err.unwrap_contract_error().to_string(),
        "Generic error: Sale hasn't finished yet."
    );

    // We manually set the current block height to simulate
    // the passage of time.
    suite.ensemble.block_mut().height = block + 1;

    let err = suite.ensemble.execute(
        &auction::ExecuteMsg::RetractBid { },
        MockEnv::new(bidder, &auction.address)
    ).unwrap_err();

    // Now that the sale has ended we see the error message change...
    assert_eq!(
        err.unwrap_contract_error().to_string(),
        "Generic error: You have won the sale and cannot retract your bid."
    );
}

#[test]
fn highest_bid_gets_updated() {
    let mut suite = Suite::new();
    let block = suite.ensemble.block().height + 1000;

    let auction = suite.new_auction(block).unwrap().contract;

    let bidder_1 = ("bidder_2", one_token(6) * 100);
    let bidder_2 = ("bidder_1", (one_token(6) * 100) + 1);

    suite.ensemble.add_funds(bidder_1.0, vec![coin(bidder_1.1, "uscrt")]);
    suite.ensemble.add_funds(bidder_2.0, vec![coin(bidder_2.1, "uscrt")]);

    suite.ensemble.execute(
        &auction::ExecuteMsg::Bid { },
        MockEnv::new(bidder_1.0, &auction.address)
            .sent_funds(vec![coin(bidder_1.1, "uscrt")])
    ).unwrap();

    suite.ensemble.execute(
        &auction::ExecuteMsg::Bid { },
        MockEnv::new(bidder_2.0, &auction.address)
            .sent_funds(vec![coin(bidder_2.1, "uscrt")])
    ).unwrap();

    let status: SaleStatus = suite.ensemble.query(
        &auction.address,
        &auction::QueryMsg::SaleStatus { }
    ).unwrap();

    assert_eq!(status.current_highest.u128(), bidder_2.1);

    suite.ensemble.block_mut().height = block + 1;

    suite.ensemble.execute(
        &auction::ExecuteMsg::RetractBid { },
        MockEnv::new(bidder_1.0, &auction.address)
    ).unwrap();

    // Check that the contract has indeed sent the uscrt
    // back to the losing bidder
    let auction_balances = suite.ensemble.balances(&auction.address).unwrap();
    assert_eq!(auction_balances["uscrt"].u128(), bidder_2.1);

    let bidder_1_balances = suite.ensemble.balances(bidder_1.0).unwrap();
    assert_eq!(bidder_1_balances["uscrt"].u128(), bidder_1.1);
}
