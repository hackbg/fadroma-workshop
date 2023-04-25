#[fadroma::dsl::contract]
pub mod auction {
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
