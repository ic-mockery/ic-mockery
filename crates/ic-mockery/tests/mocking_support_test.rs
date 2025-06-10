use std::fs;

use candid::{CandidType, Principal};
use canister::{GreetRequest, GreetResponse};
use ic_mockery::mocking_support::AsyncMocker;
use pocket_ic::{PocketIc, PocketIcBuilder};
use serde::Deserialize;
use serde_json::{json, to_value};

// A dummy CandidType for testing decode_one
#[derive(Debug, PartialEq, CandidType, Deserialize)]
struct Dummy(u8);

#[test]
#[should_panic(expected = "Missing call")]
fn execute_without_call_panics() {
    let pic = PocketIc::new();
    // no .with_call → should panic on .execute()
    AsyncMocker::new(&pic).execute::<Dummy>().unwrap();
}

#[test]
fn builder_methods_chain() {
    let pic = PocketIc::new();
    // just make sure these compile and don't panic immediately
    let _m = AsyncMocker::new(&pic)
        .with_call(|| {
            // in a real test, you'd call `pic.some_method()` to get a RawMessageId
            unimplemented!()
        })
        .mock("foo", |_req| json!({ "foo": 42 }));
}

#[test]
fn basic_execute_flow_should_return_value() {
    let pic = PocketIcBuilder::new()
        .with_application_subnet() // to deploy the test depp
        .build();
    let canister = pic.create_canister();

    const WASM: &str = "../../target/wasm32-unknown-unknown/release/canister.wasm";

    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let wasm = fs::read(format!("{}/{}", root, WASM)).expect("Wasm file not found.");
    pic.add_cycles(canister, 2_000_000_000_000); // 2T Cycles

    pic.install_canister(canister, wasm, vec![], None);

    AsyncMocker::new(&pic)
        .call(
            canister,
            Principal::anonymous(),
            "greet",
            GreetRequest {
                name: "Wizard".into(),
            },
        )
        .mock("greet", |args| {
            // Args should be a GreeRequest
            let response = GreetResponse {
                message: args["args"][0]["name"].as_str().unwrap().into(),
                status: canister::Status::Success,
            };
            to_value(response).unwrap()
        })
        .mock("prepare_greet", |_| to_value::<()>(()).unwrap())
        .execute::<GreetResponse>()
        .expect("mocking failed");
}
