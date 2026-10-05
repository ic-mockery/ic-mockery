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

// PocketIC's manual rounds advance simulated time by 1ns. This bound catches
// the old 500-round wait without depending on the speed of the test host.
const MAX_COMPLETION_ADVANCE_NANOS: u64 = 100;

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

fn setup_canister() -> (PocketIc, Principal) {
    let pic = PocketIcBuilder::new()
        .with_application_subnet() // to deploy the test depp
        .build();
    let canister = pic.create_canister();

    const WASM: &str = "../../target/wasm32-unknown-unknown/release/canister.wasm";

    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let wasm = fs::read(format!("{}/{}", root, WASM)).expect("Wasm file not found.");
    pic.add_cycles(canister, 2_000_000_000_000); // 2T Cycles

    pic.install_canister(canister, wasm, vec![], None);

    (pic, canister)
}

#[test]
fn basic_execute_flow_should_return_value() {
    let (pic, canister) = setup_canister();
    let before = pic.get_time().as_nanos_since_unix_epoch();

    let response = AsyncMocker::new(&pic)
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
        .mock("unused", |_| panic!("unused response must not be consumed"))
        .execute::<GreetResponse>()
        .expect("mocking failed");

    assert_eq!(response.message, "Wizard");
    assert!(matches!(response.status, canister::Status::Success));
    let advanced_nanos = pic.get_time().as_nanos_since_unix_epoch() - before;
    assert!(
        advanced_nanos < MAX_COMPLETION_ADVANCE_NANOS,
        "completed reply advanced {advanced_nanos}ns"
    );
}

#[test]
fn rejected_ingress_does_not_wait_for_unused_responses() {
    let (pic, canister) = setup_canister();
    let before = pic.get_time().as_nanos_since_unix_epoch();

    let error = AsyncMocker::new(&pic)
        .call(canister, Principal::anonymous(), "missing_method", ())
        .mock("unused", |_| panic!("unused response must not be consumed"))
        .execute_no_ticks::<()>()
        .expect_err("missing method must reject the ingress call");

    assert!(error.contains("missing_method"), "{error}");
    let advanced_nanos = pic.get_time().as_nanos_since_unix_epoch() - before;
    assert!(
        advanced_nanos < MAX_COMPLETION_ADVANCE_NANOS,
        "completed rejection advanced {advanced_nanos}ns"
    );
}
