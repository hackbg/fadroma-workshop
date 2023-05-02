## Implementing the smart contracts

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
    fadroma = { git = "https://github.com/hackbg/fadroma", tag  = "crate@0.8.6", features = ["vk"] }
    serde = { version = "1.0.114", default-features = false, features = ["derive"] }
    ```

We also need to enable the viewing key feature for our auction contract. So go to the project `Cargo.toml` file and **change** the fadroma import to:

```toml
fadroma = { git = "https://github.com/hackbg/fadroma", tag  = "crate@0.8.6", features = ["vk"] }
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
            //Import StdResult from cosmwasm_std
            .collect::<StdResult<Vec<AuctionEntry<Addr>>>>()?
    })
}
```

You may notice that we never encode the response as `Binary` which is required by CosmWasm. This is because that step is generated by the DSL for each query method inside
the query function entry point. You write the actual return type in your query instead.

### Auction contract

#### 1. Setup

Once again, we start by adding as dependency the shared library we created in the project file (`src/auction/Cargo.toml`):

```toml
shared = { path = "../shared" }
```

However, this time our starting point looks a bit different. We want our contract to implement the `Auction` interface we defined back in the shared library. In order to do this we simply use Rust's syntax for implementing a trait on a type:

```rust
#[fadroma::dsl::contract]
pub mod auction {
    use fadroma::{
        dsl::*,
        cosmwasm_std::{self, Response, StdError, Uint128},
        schemars
    };
    use shared::{Auction, Pagination, PaginatedResponse, SaleInfo, SaleStatus};

    impl Auction for Contract {
        type Error = StdError;

        #[init(entry_wasm)]
        fn new(
            admin: Option<String>,
            name: String,
            end_block: u64
        ) -> Result<Response, <Self as Auction>::Error> {
            todo!()
        }
    
        #[execute]
        fn bid() -> Result<Response, <Self as Auction>::Error> {
            todo!()
        }
    
        #[execute]
        fn retract_bid() -> Result<Response, <Self as Auction>::Error> {
            todo!()
        }

        #[execute]
        fn claim_proceeds() -> Result<Response, <Self as Auction>::Error>;
    
        #[query]
        fn view_bid(
            address: String,
            key: String
        ) -> Result<Uint128, <Self as Auction>::Error> {
            todo!()
        }
    
        #[query]
        fn active_bids(
            pagination: Pagination
        ) -> Result<PaginatedResponse<Uint128>, <Self as Auction>::Error> {
            todo!()
        }
    
        #[query]
        fn sale_status() -> Result<SaleStatus, <Self as Auction>::Error> {
            todo!()
        }
    }
}
```

We've now implemented the `Auction` trait on the `Contract` type. This is the same thing
that you would do in standard Rust. The only that the DSL expects is that you put one
of the `init`, `execute` or `query` attriutes above each method. If you don't you will get
a compile error - it won't just compile regardless. We also have to use the `Contract` type if we want the macro to include the methods as part of the contract's messages. Anything else won't be touched by the macro at all. This allows you the freedom to write any other code the way you need. Notice that we are setting the `new()` method of the
interface as the entry point now by using `#[init(entry_wasm)]`.

> You are still free to add any other methods inside an `impl Contract` block. This enables composability - you implement any interfaces that your contract needs to have and
can add methods specific to the given contract at the same time.

We've said that the error type this component's method will return is `type Error = StdError;`. The `type Error` definition is required by the `interface` attribute at the
the trait definition site and as such is required by Rust when we are implementing the 
interface. This allows you to have custom error types if you want to. Otherwise, simply use `cosmwasm_std::StdError` which is what we are doing here. Bear in mind that you are allowed to have different error types for each interface you implement and inside the
`impl Contract` block as well.

However, you will notice that if you try compiling that you will get some errors:

```
error[E0277]: the trait bound `Contract: VkAuth` is not satisfied
  --> src/auction/src/lib.rs:10:10
   |
10 |     impl Auction for Contract {
   |          ^^^^^^^ the trait `VkAuth` is not implemented for `Contract`
   |
   = help: the trait `VkAuth` is implemented for `fadroma::scrt::vk::auth::DefaultImpl`
```

```
error[E0277]: the trait bound `Contract: Killswitch` is not satisfied
  --> src/auction/src/lib.rs:10:10
   |
10 |     impl Auction for Contract {
   |          ^^^^^^^ the trait `Killswitch` is not implemented for `Contract`
   |
   = help: the trait `Killswitch` is implemented for `fadroma::killswitch::DefaultImpl`
```

If you look back at the `Auction` trait in `src/shared/lib.rs`, you will see that we
have declared `VkAuth` and `Killswitch` as super traits:

```rust
#[interface]
pub trait Auction: Killswitch + VkAuth { ... }
```

These two come from Fadroma and represent common functionality in smart contracts. As Rust's helpful error message shows, these come with default implementations which we are
now going to use. It even tells us where we can find these types. By taking advantage of
Rust's trait system in the DSL, we unlock these helpful message and make our code more robust. We are essentially saying that if you want to implement the `Auction` interface
you will need to have viewing key and killswitch functionality since the auction contract
depends on those. So we need to add them to our contract now.

First, we add the imports so that they now look like this:

```rust
use fadroma::{
    dsl::*,
    scrt::vk::auth::{self, VkAuth}, // New
    killswitch::{self, Killswitch, ContractStatus}, // New
    cosmwasm_std::{self, Response, StdError, Uint128, Addr}, // Added Addr
    schemars
};
```

Then we implement the interfaces:

```rust
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
```

Here we implement the relevant traits on `Contract` just like we did with `Auction`.
But you will notice that we've added the `auto_impl` attribute on top of each block and
that the code looks incomplete - we are missing the method bodies! As the name suggests,
`auto_impl` takes care of the implementation for us. Although this may look like magic, it is very simple in practice. The `auto_impl` attribute takes whatever path to a type
we've given it (in our case these are `auth::DefaultImpl` and `killswitch::DefaultImpl`)
respectively. And simply calls each method in the block on that type. It uses Rust's fully qualified syntax to make sure that the type that that you've given it indeed
implements the trait that you've provided it for. This way there are not surprises.
It also fills in the `type Error` as the type defined on the implementing struct. To better illustrate this concept, this is what happens in practice:

```rust
impl VkAuth for Contract {
    type Error = <auth::DefaultImpl as VkAuth>::Error;

    fn create_viewing_key(
        mut deps: cosmwasm_std::DepsMut,
        env: cosmwasm_std::Env,
        info: cosmwasm_std::MessageInfo,
        entropy: String,
        padding: Option<String>
    ) -> Result<Response, Self::Error> {
        <auth::DefaultImpl as VkAuth>::create_viewing_key(
            deps,
            env,
            info,
            entropy,
            padding
        )
    }

    fn set_viewing_key(
        mut deps: cosmwasm_std::DepsMut,
        env: cosmwasm_std::Env,
        info: cosmwasm_std::MessageInfo,
        key: String,
        padding: Option<String>
    ) -> Result<Response, Self::Error> {
        <auth::DefaultImpl as VkAuth>::set_viewing_key(
            deps,
            env,
            info,
            key,
            padding
        )
    }
}
```

> We can go further with this - if you want to override a particular method but not all of them you can just add an implementetion to the method block and the `auto_impl` won't insert anything for you. This may be reminiscent of OOP programming where we inherit a 
class and override the methods of the base class the we need to.

We are not done just yet though. If you try to compile now, you will see a familiar error:

```
error[E0277]: the trait bound `Contract: fadroma::admin::Admin` is not satisfied
  --> src/auction/src/lib.rs:73:10
   |
73 |     impl Killswitch for Contract {
   |          ^^^^^^^^^^ the trait `fadroma::admin::Admin` is not implemented for `Contract`
   |
   = help: the following other types implement trait `fadroma::admin::Admin`:
             fadroma::admin::DefaultImpl
             fadroma::killswitch::DefaultImpl
```

Although, we've never explicitly stated that the `Auction` contract should implement
admin functionality, the `Killswitch` interface that we just implemented says that it
requires it and we cannot continue further until we implement it in our contract. We implement it the same way we did with the `Killswitch` and `VkAuth` interfaces:

First we import the relevant types:

```rust
use fadroma::{
    ...,
    admin::{self, Admin, Mode}
};
```

And then write the implementation:

```rust
#[auto_impl(admin::DefaultImpl)]
impl Admin for Contract {
    #[execute]
    fn change_admin(mode: Option<Mode>) -> Result<Response, Self::Error> { }

    #[query]
    fn admin() -> Result<Option<Addr>, Self::Error> { }
}
```

We can now finally successfully compile!

Unfortunately, we can't just do everything automatically. Different modules have different
requirements and as such require different setups that we can't just implement for you. So you will still have to learn how set them up. You can see the [examples](https://github.com/hackbg/fadroma/blob/master/examples/derive-contract-components/src/lib.rs) where this is described for every available module. We are going to do the same here since we already started.

#### 2. Setting up the admin module

Doing so is very simple, we just need to assign an address as the admin during the instantiation of our contract. In our case, this will be the `new` method inside the `impl Auction for Contract` block which should now look like this:

```rust
#[init(entry_wasm)]
fn new(
    admin: Option<String>,
    name: String,
    end_block: u64
) -> Result<Response, <Self as Auction>::Error> {
    admin::init(deps.branch(), admin.as_deref(), &info)?;

    Ok(Response::default())
}
```

#### 3. Setting up the killswitch module

For this one, we will have to do slightly more work. Essentially, we need a way
to stop executing any messages if the contract has been paused or has migrated to a new
instance. We could check if the contract is operation in every single one of our execute
messages but that will quickly become repetitive and error prone. Instead we will make use
of the last piece of functionality that the DSL offers. We implement this like so:

```rust
impl Contract {
    #[execute_guard]
    pub fn guard(msg: &ExecuteMsg) -> Result<(), StdError> {
        let operational = killswitch::assert_is_operational(deps.as_ref());

        if operational.is_err() && !matches!(msg, ExecuteMsg::SetStatus { .. }) {
            Err(operational.unwrap_err())
        } else {
            Ok(())
        }
    }
}
```

You can have a single method in your contract annotated with #[execute_guard]. It must have
a single parameter which is the contract's execute message enum generated by the DSL. You can use this to check the message being execute which is what we do here. You must also put that method inside the `impl Contract` block since it is specific the given contract and it doesn't really makes sense for it to be a part of any interface anyways (which is why you can't use that attribute in interface definitions or implementations). To illustrate how this works, in
CosmWasm, you'd have the `execute` function which is the entry point of your contract which
gets passed an instance of your message. Then you would match against that message and call
the relevant function:

```rust
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg { ... }
}
```

Since the DSL generates this function for you and as such you don't have acess to it, the
`#[execute_guard]` method allows you to hook into it. So when it has been defined, the execute
function will look like this:

```rust
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    Contract::guard(deps.branch(), &env, &info, &msg)?;

    match msg { ... }
}
```

As you can see the guard function first runs before matching any messages and if it returns
an error the execution stops right there. You will also notice that as any other methods
you get passed the usual CosmWasm types but here `Env` and `MessageInfo` are references.

#### 4. Defining the storage types

For this contract we want to be able store the balance for each bidder while also making
it possible to iterate through all bids revealing only the amounts (a bit contrived I know..). We also need to be able to tell who the highest bidder is at any time as well as providing the info we get in the instantiate message (name of the sale and the end block).
For the last two we can use the already familiar `SingleItem` type:

Let's first import all the storage types that we will be needing:

```rust
use fadroma::{
    ...,
    storage::{SingleItem, TypedKey, map::InsertOnlyMap}
};
```

> Note that we use `InsertOnlyMap` here since we will never be removing any entries -
only inserting and updating. If you need the ability to remove entries then use
`Map` instead. Both types have almost identical API but `InsertOnlyMap` is more efficient
which is why you should use that if you can. Also both maps only support iterating through
the values, not the keys.

Then we define those like so:

```rust
namespace!(InfoNs, b"info");
const INFO: SingleItem<SaleInfo, InfoNs> = SingleItem::new();

namespace!(HighestBidNs, b"highest_bid");
const HIGHEST_BID: SingleItem<CanonicalAddr, HighestBidNs> = SingleItem::new();
```

For the `InsertOnlyMap` we define a helper function like we did with `IterableStorage`
since it also has internal state that it needs to mutate:

```rust
namespace!(BiddersNs, b"bidders");
#[inline]
fn bidders() -> InsertOnlyMap<
    TypedKey<'static, CanonicalAddr>,
    Uint128,
    BiddersNs
> {
    InsertOnlyMap::new()
}
```

We defined a map that uses `CanonicalAddr` as keys and stores `Uint128` values under the
`bidders` namespace. Notice the `TypedKey` here - this is a type of `Key` that enables
strongly-typed keys using arbitrary types. Any type that implements the [Segment](https://docs.rs/fadroma/0.7.0/fadroma/storage/trait.Segment.html) trait can be used as part of a
`TypedKey` (you will see that there are typed key version for varying number of segments).
Fadroma already implements it for commonly used key types such as addresses and numerics.

#### 5. Finishing the instantiate method

Now that we have defined all storage types we can finish our instantiate method which
should now look like this:

```rust
#[init(entry_wasm)]
fn new(
    admin: Option<String>,
    name: String,
    end_block: u64
) -> Result<Response, <Self as Auction>::Error> {
    if end_block <= env.block.height {
        return Err(StdError::generic_err("End block has already passed."));
    }

    admin::init(deps.branch(), admin.as_deref(), &info)?;
    INFO.save(deps.storage, &SaleInfo { name, end_block })?;

    Ok(Response::default()
        .set_data(to_binary(&env.contract.address)?)
    )
}
```

We check that the provided end block for the sale hasn't passed already. Also, note that we are setting the contract's address as the response data here since that is
expected in the reply handler of the factory contract.

#### 6. Implementing the execute methods

In the `bid()` method we want to check if the sale has finished in which case we return
an error. Otherwise, we increase the bidder's balance using the amount of `uscrt` sent.
We also have to check if the bidder's balance now exceeds the current highest and 
if so - update the `HIGHEST_BID` storage with their address:

```rust
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
```

In `retract_bid()` we only should return the bidder's `uscrt` if the sale has finished and the bidder
is not the winner.

```rust
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
```

Finally, we implement the admin-only method to claim the `uscrt` from the winner but only after the
sale has finished:

```rust
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
```

> Note the `#[admin::require_admin]` attribute macro. It inserts a single call to
`admin::assert(deps.as_ref(), &info)?;` at the beginning of the body. You can use
either approach, it's matter of preference.

#### 6. Implementing the query methods

In the `view_bid()` query we simply need to verify if the bidder's viewing is valid
and return their balance. We do this using the `auth` module's `authenticate` function.
Because we've already implemented the viewing key interface automatically, user's can
set or create their own viewing keys which is what we check for in here:

```rust
#[query]
fn view_bid(
    address: String,
    key: String
) -> Result<Uint128, <Self as Auction>::Error> {
    let address = address.as_str().canonize(deps.api)?;
    auth::authenticate(deps.storage, &ViewingKey::from(key), &address)?;

    bidders().get_or_default(deps.storage, &address)
}
```

Next, the `active_bids()` bids is pretty much identical to what we did in the factory
contract for listing all auctions but here we use the `values()` method of the map:

```rust
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
```

Finally, we implement the `sale_status()` query which simply returns the sale info
that we stored during instantiation, the current highest bid and boolean flag for convenience indicating whether the sale has finished:

```rust
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
```

### Integration tests with Fadroma Ensemble

Now that we have implemented our contracts we want to test if they actually work as
intended not only in isolation but also when interacting with eachother. Testing the
contracts on testnet or even using a localnet container can be tedious, slow and
limiting. We want something better. The Fadroma Ensemble implements the blockhain
semantics and allows us to inspect and manipulate pretty much anything we need so that we
can test all possible scenarios. It's important to note that you should still test your
contracts on the actual blockchain. However the Ensemble helps you catch any logic errors
in your contracts which is where most of our concerns and bugs are rooted. The Ensemble
allows us to:

- Run all tests immediately without having to wait for blocks.
- Use an actual debugger to step through the code and inspect variables.
- Manipulate block time and height in order to test interactions that rely on them.
- Manipulate contract storage directly in order to test different states more easily.
- Distribute native funds at will.
- Have human-friendly addresses.

We will create a new crate that will contain all of our tests by following the same
process as we did with the `shared` library in the beginning. In the `src` directory we
run `cargo new --lib tests --vcs none` and add the path to the `Cargo.toml` workspace file
at the root. The in the `Cargo.toml` file located inside the crate we just generated we 
add the following dependencies:

```toml
[dependencies]
fadroma = { git = "https://github.com/hackbg/fadroma", tag = "crate@0.8.6", features = ["scrt", "ensemble"] }
factory = { path = "../factory" }
auction = { path = "../auction" }
shared = { path = "../shared" }
```

In the `lib.rs` that was generated for us we add the following contents:

```rust
#[cfg(test)]
mod auction;
```

Then we create a file named `auction.rs` based on the module that we just declared
(in the same directory as `lib.rs`). First, let's import everything that we will need:

```rust
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
```

In order to tell the Ensemble how to use our contract, we have to make use of the
`ContractHarness` trait. If you inspect that trait you will notice that its methods
are the familiar CosmWasm entry points (`instantiate`, `execute`, etc.). So we simply
need to call the corresponding functions of the contracts that were generated by the DSL
in order to implement it. First, we do this for the factory contract:

```rust
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
```

Fadroma provides a convenient macro that will implement this boilerplate for you. It
currently doesn't support the optional `reply` entry point which is why we had to
implement the trait manually for the factory. We make use of it for the auction contract:

```rust
impl_contract_harness!(Auction, auction);
```

The first argument here is the struct name the we want generated and the second one is
the path to the module containing our entry points.

Then we declare two constants that will represent some common addresses that we will be
using in our tests.

```rust
const FACTORY: &str = "factory";
const ADMIN: &str = "admin";
```

We will create a convenience struct that we will be using in all of our tests. It will
do an initial setup for us.

```rust
struct Suite {
    ensemble: ContractEnsemble,
    factory: ContractLink<Addr>
}
```

The `ensemble` field is what we use to interact with the Fadroma Ensemble. It holds all
of the relevant state and executes the contracts for us. The other field is the info for
factory contract that we will instantiate in every tests since we have to use it in order
to create auctions.

In an `impl Suite` block we add the following method:

```rust
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
```

The `register` method the we call here twice corresponds to the contract upload phase on
the blockchain. It takes a struct that implements the `ContractHarness` trait. It returns
a the already familiar `ContractInstantiationInfo` struct that we can now use to create
new instances of the registered contracts. We do this by calling the `instantiate` method
on the next line. Just as on the actual chain, we pass the id of the uploaded contract and
the instantitate message the it's expecting. The first parameter of the `MockEnv` struct
that we are passing is the address that call the instantiate message. In our case that's
not important so we hardcode it as `"sender"` (though it could be any other string). The
second parameter is the address that we want our instance to have - we use the constant
that we declared above. We could now use this constant to call this particular instance
in our tests, or we can use `factory.address` field on the `Suite` instance which will 
contain the same string. We will take latter approach in our tests. You can use whichever
way you prefer in your tests, but hopefully this illustrates how we use addresses in the
Ensemble.

We will add another method to the `Suite` struct now:

```rust
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
```

It will call the factory that we instantiated in `new()` and return its info. We grab
the info by querying the factory itself. By doing this we also test that we've
implemented the message correctly. Again, as on the actual chain, we call the execute
method using the relevant execute message. The first parameter in the `MockEnv` struct is
again the address who initiated the message which we don't care about. The seconds
parameter this time is the contract the we are calling which in our case is the factory
contract. This is where we could pass the `FACTORY` constant instead to get the same
result.

We follow the same pattern in the `query` method as well. However, since query messages
are not signed there isn't actually a sender address, so only the contract address and the
relevant query message is required. We also need to indicate the type that we are
expecting the query to return and it will be deserialized automatically for us.

> You may notice that we are not passing a code hash here as we would. This is because
it won't actually help prevent anything since these top level functions would be the 
equivalent of calling a contract from client code so you either have to correct code hash
or you don't. However, passing a wrong code hash in any instantiate, execute or query
messages that are sent by a contract will cause the Ensemble to return an error as this
would actually hint to a logic error in your contract.

Now we can finally start writing some tests. First we write a test to check that the
auction contract is initialized with the correct state:

```rust
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
```

We use the concepts described above to query the state of the instantiated auction.
On the second line you will notice that we can fetch the current block height by
calling `ensemble.block()`.

We also want to check that it is not possible to create auctions where the end block has
already passed:

```rust
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
```

Note that we call `unwrap_err()` instead of `unwrap()` on the `new_auction` method since
we are expecting the transaction to have failed. The returned error type is an enum that
represents different kinds of errors that could occur. These include errors that could
occur on chain during execution like trying to execute a contract with a wrong code hash or some bank module message where the sender doesn't have the amount it's trying to send.
Those errors usually indicate an error with your test setup. But here we are interested
in the `ContractError` variant which is returned when the transaction fails due to an
error returned by a contract. There is a convenience function on the error enum to get it:
`unwrap_contract_error()` which is what we use in our test.

Now we add a tests to cover the bidding process:

```rust
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
```

We won't go in detail here but this code showcases how we can use the Ensemble to
setup and check native coin balances held by addresses and contracts. Since our auction
contract requires `uscrt` to be sent by the bidders we need to simulate the bidder having
that kind of balance. We do that at the beginning with this code

```rust
suite.ensemble.add_funds(bidder, vec![coin(bid_amount, "uscrt")]);
```

The first parameter here is the address to add the balances to and the second is a list
of coins (amount + denomination).

After successfully executing the bid we want to check if the auction contract has
successfully received the `uscrt` sent by the bidder:

```rust
let auction_balances = suite.ensemble.balances(&auction.address).unwrap();
assert_eq!(auction_balances["uscrt"].u128(), bid_amount);
```

We simply provide the address (the auction contract in our case) that we need to read
the balances for. The returned type is a regular Rust hash map of all the balances
recorded for the given address where the keys are the denominations and the values are the
amounts.

Now we want to test that bidders cannot retract their bids before the sale has finished:

```rust
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
```

Here the point of interest is this line:

```rust
suite.ensemble.block_mut().height = block + 1;
```

This is where we set the block height to the block after the one which the sale is
supposed to end. We need to do this in order to be able to test retract bid message.
You could also have the Ensemble stop incrementing blocks and time by calling the
`freeze()` method on the returned mutable reference by `block_mut()`. It's event possible
to set the strategy by which block height and time are incremented after each run using
the `exact_increments` and `random_increments` methods.

You can find more tests in this repository inside the `src/tests/src/auction.rs` file.
