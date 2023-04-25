## Chapter 2: Implementing the smart contracts

### Creating the shared library

In larger projects it is very common to create a shared library that all contracts can consume. Such library contains the interface (messages) definitions of all contracts in the project and other common/useful functionality. This is also done because of very practical constraints - cyclical dependencies. If two contracts need to know about eachother they have to reference eachother's crates, but that creates a cyclical dependency between the two which Cargo disallows. So a third crate is needed which they can both reference instead. Shared crates can also be useful for only exposing the interface of contracts without their source code.

In our case we don't really need such a shared crate, but will utilize one regardless in order to showcase how that works when using Fadroma DSL.

Assuming we are in the root directory of the project:

 1. Move to the `src` directory
 2. Run `cargo new --lib shared --vcs none`
 3. Back in the root, in the workspace `Cargo.toml` file, add the path to the `members` array: "src/shared"
 4. Finally, in the `Cargo.toml` project file of the library we just created (`src/shared/Cargo.toml`) add the following dependencies:

    ```toml
    [dependencies]
    fadroma = { git = "https://github.com/hackbg/fadroma", tag = "crate@0.8.0", features = ["vk"] }
    serde = { version = "1.0.114", default-features = false, features = ["derive"] }
    ```

We also need to enable the viewing key feature for our auction contract. So go to the project `Cargo.toml` file and **change** the fadroma import to:

```toml
fadroma = { git = "https://github.com/hackbg/fadroma", tag = "crate@0.8.0", features = ["vk"] }
```

We replaced the "scrt" with the "vk" feature which itself enables "scrt".

Now in the newly created `shared` crate, **replace** the contents of the `lib.rs` file with the following:

```rust
use fadroma::{
    dsl::*,
    schemars,
    core::{Humanize, Canonize},
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
    fn bid(amount: Uint128) -> Result<Response, <Self as Auction>::Error>;

    #[execute]
    fn retract_bid() -> Result<Response, <Self as Auction>::Error>;

    #[query]
    fn view_bid(
        address: String,
        key: String
    ) -> Result<Uint128, <Self as Auction>::Error>;

    #[query]
    fn active_bids(
        pagination: Pagination
    ) -> Result<PaginatedResponse<Vec<Uint128>>, <Self as Auction>::Error>;

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
```

We will unpack this later, but it boils down to defining the auction contract interface
and some types that are used in both contracts.

### Auction Factory

#### 1. Setup

We start by adding as dependency the shared library we created in the project file (`src/factory/Cargo.toml`):

```toml
shared = { path = "../shared" }
```

Now we can start writing the contract skeleton using Fadroma DSL. This is the minimal code that compiles:

```rust
#[fadroma::dsl::contract]
pub mod factory {
    use fadroma::{
        dsl::*,
        cosmwasm_std::{self, Response, StdError},
        schemars
    };

    impl Contract {
        #[init(entry_wasm)]
        pub fn new() -> Result<Response, StdError> {
            Ok(Response::default())
        }
    }
}
```

All contract interface methods must go inside the `impl Contract`. This allows you to have any other functions, type declarations with their implementations or nested `mod` blocks you need and they will be ignored by the DSL. You are also free to have any helper methods inside the `impl Contract` as well. As long as they are **not** marked with any DSL attributes, they will remain "private". We will also be writing everything inside the 
`pub mod factory` block.

All that is required is an instantiate method which in our case is the `new` method. Instantiate methods are marked with the `init` DSL attribute and must have the `entry_wasm` meta tag in order to generate the WASM boilerplate. You can also have `entry` instead of `entry_wasm` which will generate the `instantiate`, `execute` and `query` entry point functions but without the WASM boilerplate. This allows you to use the contract as a library. However, we won't be using that in this example.

Since we currently have no execute or query methods, the macro will generate default noop methods so that we can have a valid contract.

> You can read the full documentation for Fadroma DSL on the crate page at [https://crates.io/crates/fadroma-dsl](https://crates.io/crates/fadroma-dsl). It's important to remember that the macro will give you error messages telling you exactly what is expected or what the issue is so you don't have to worry about remembering every detail.

#### 2. Implementing the instantiate method

In order to be able to instantiate auction contracts we will need an information about the code which should already be stored on chain - a code id and a code hash. Fadroma already has such a structure defined called `ContractInstantiationInfo`. Import it as `core::*` since there will be other basic types that we will be using from the `core` module later on.

We will also need to store this data for later usage. Since there will only ever be a single instance of this data that we will make use of, we can use Fadroma's simplest storage type - `SingleItem`. As the name suggests, it stores a single value under a given hardcoded key. In Fadroma, we differentiate such hardcoded keys from keys that are dynamically generate as namespaces represented by the `Namespace` trait.

> It will become evident how this is useful when we get to the more advanced storage scenarios later, but it basically helps us ensure that we are storing different logical units of data under uniquely prefixed keys. This means that if we store different data under the same type of key, for example addresses, these will automatically be separated because of the strongly typed nature of the storage types. The overarching design goal for the Fadroma storage types is that we don't only want the data that we store to be strongly-typed, but the keys that it's stored under to be strongly-typed as well.

So we will have to declare a namespace that `SingleItem` will use. Because `Namespace` is a trait you will need to declare a type and implement it on it but all this is redundant so you can do so in a single line using the `namespace!` macro which does this for you. So let's import that along with the storage type which is in the `storage` module. Your imports should now look like this:

```rust
use fadroma::{
    dsl::*,
    core::*,
    schemars,
    cosmwasm_std::{self, Response, StdError},
    storage::SingleItem,
    namespace
};
```

Then we declare the namespace and our auction contract storage:
```rust
    namespace!(ContractNs, b"contract");
    const AUCTION_CONTRACT: SingleItem<
        ContractInstantiationInfo,
        ContractNs
    > = SingleItem::new();
```

We then change our `new()` method to have an `auction: ContractInstantiationInfo` parameter that will have to be passed to us in the instantiate message. Inside the function body, before the returned `Response`, we then store the data:

```rust
AUCTION_CONTRACT.save(deps.storage, &auction)?;
```

> You might wonder where the `deps` variable is coming from. All the types that are passed to the relevant CosmWasm entry points (instantiate, execute, query, etc.) are implicitly generated by the macro to be a part of the relevant function signature using the most commonly used variable names. See the [docs](https://github.com/hackbg/fadroma/tree/master/crates/fadroma-dsl#usage-1) for the full list. In this case since it's the instantiate method we also implicitly have `deps: DepsMut`, `env: Env`, `info: MessageInfo` as part of the function signature.

So far the entire code should look something like this:

```rust
#[fadroma::dsl::contract]
pub mod factory {
    use fadroma::{
        dsl::*,
        core::*,
        schemars,
        cosmwasm_std::{self, Response, StdError},
        storage::SingleItem,
        namespace
    };

    namespace!(ContractNs, b"contract");
    const AUCTION_CONTRACT: SingleItem<
        ContractInstantiationInfo,
        ContractNs
    > = SingleItem::new();

    impl Contract {
        #[init(entry_wasm)]
        pub fn new(auction: ContractInstantiationInfo) -> Result<Response, StdError> {
            AUCTION_CONTRACT.save(deps.storage, &auction)?;

            Ok(Response::default())
        }
    }
}
```

#### 3. Instantiating auctions

First, we need to think about the ways we are going to be using the data. We want the
factory to serve as a registry of all created auctions, past and present. We want to be able to list all of them but don't really need to fetch specific auctions. For this we can
use the `IterableStorage` which conceptually works the almost the same way as a `Vec`. Among other operations, it allows us to append items and iterate over all of them as well.
It can be imported from the `storage::iterable` module. We want to store the address and code hash of the created auctions but also their name and end block in order to avoid a lot of extra queries when calling from the frontend. For the latter two, we can use the `SaleInfo` struct that we defined in the shared library and for the former two Fadroma has the `ContractLink` type defined since its usage is extremely common. It is generic over the address field which allows to use it for both `Addr` and `CanonicalAddr` depending on our needs. It should already be imported from the `core` module. So we group
all this data into the following struct:

```rust
#[derive(Serialize, Deserialize, FadromaSerialize, FadromaDeserialize, Canonize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AuctionEntry<A> {
    pub contract: ContractLink<A>,
    pub info: SaleInfo
}
```

> Note the `FadromaSerialize` and `FadromaDeserialize` traits that we derived. These enable Fadroma's binary serialization which is [vastly more efficient](https://github.com/hackbg/fadroma/pull/147) than serializing to JSON and we need them in order to use Fadroma's storage types. They are semantically equivalent to `serde`'s own traits. Import them from the `bin_serde` module.

> Notice the `Canonize` trait derived as well. Fadroma's `Humanize` and `Canonize` traits can be derived on both generic and non-generic types (deriving `Canonize` implements `Humanize` as well since both are connected) that contain addresses so that they can easily be converted to the equivalent type but with the relevant address representation. Once derived, you get access to the `humanize` and `canonize` methods respectively. If you need your type to implement `Canonize` but it has no address fields itself use the `impl_canonize_default!` macro instead like we did for `SaleInfo` back in the `shared` crate. See the [docs](https://docs.rs/fadroma/0.7.0/fadroma/core/addr/trait.Canonize.html) for more info.

Now we can finally define our storage type. This time it won't be a `const` because unlike
`SingleItem`, `IterableStorage` has internal state which needs to mutate, so we define a
convenience function instead (below and outside of the `impl Contract` block):

```rust
// Import CanonicalAddr from cosmwasm_std
#[inline]
fn auctions() -> IterableStorage<
    AuctionEntry<CanonicalAddr>,
    StaticKey
> {
    IterableStorage::new(StaticKey(b"auctions"))
}
```

`IterableStorage` takes a `Key` trait which allows flexibility under what key path the
items are inserted. In our case we just need a static path, so for this we use `StaticKey` from the `storage` module.

> There are several types of `Key` defined in Fadroma where, again, the goal is to have
strongly typed keys. But also the `Key` interface is designed in such a way to allow
the resulting raw bytes that are written as the key in CosmWasm's storage to always be
constructed as a single allocation. To learn more about keys in Fadroma, refer to the [module documentation](https://docs.rs/fadroma/0.7.0/fadroma/storage/index.html).

We will need to use the auction interface that we defined in the shared library, more specifically - the instantiate message. Import it as:

```rust
use shared::InstantiateMsg as AuctionInitMsg;
```

Then inside `impl Contract` block we add the following:

```rust
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
        "Auction contract {}, started at: {}, ending at {}",
        name,
        end_block,
        env.block.height
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
```

The implementation here is straightforward code that you'd write in any contract.
Since we have a reply message we will need to define a reply entry point on our contract.
Just below the `create_auction` method we just implemented, insert the following:

```rust
#[reply]
pub fn reply(reply: Reply) -> Result<Response, StdError> {
    if reply.id != 0 {
        return Err(StdError::generic_err("Unexpected reply id."));
    }

    let resp = reply.result.unwrap();
    let address: Addr = from_binary(resp.data.as_ref().unwrap())?;

    let auctions = auctions();

    let index = auctions.len(deps.storage)? - 1;
    auctions.update_at(deps.storage, index, |mut entry| {
        entry.contract.address = address.canonize(deps.api)?;

        Ok(entry)
    })?;

    Ok(Response::default())
}
```

Here we fetch the last entry in the auctions collection that we just appended in
`create_auction` and use the `update_at` method of `IterableStorage` to set its address
to then one we got in the reply response.

#### 4. Listing auctions

Finally, we will implement our query function that lists all registered auctions in pages.
We use the `Pagination` and `PaginatedResponse` structs that we defined in the shared library. `IterableStorage` implements Rust's `Iterator` trait so we can conveniently use it like so:

```rust
#[query]
pub fn query(
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
            //Import StdResult from cosmwasm_std
            .collect::<StdResult<Vec<AuctionEntry<Addr>>>>()?
    })
}
```

You may notice that we never encode the response as `Binary` which is required by CosmWasm. This is because that step is generated by the DSL for each query method inside
the query function entry point. You write the actual return type in your query instead.
