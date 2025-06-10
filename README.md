# IC-Mockery

A testing and mocking framework for Internet Computer (IC) canister development in Rust.

## Overview

The core of IC-Mockery is a procedural macro that automatically transforms async methods returning a Result into methods that use HTTP outcalls. This transformation enables the use of PocketIC to mock these calls during testing, without changing your production code.

## Features

- **Mock Async HTTP Calls**: Easily mock responses for asynchronous HTTP calls made by your canisters
- **Integration with PocketIC**: Built on top of `pocket-ic` for local IC environment simulation
- **Procedural Macros**: Automatic transformation of async functions that return a Result into methods that use HTTP outcalls, which can then be mocked
- **Fluent API**: Simple and expressive API for setting up mocks and verifying interactions

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
ic-mockery = { git = "https://github.com/ic-mockery/ic-mockery" }
ic-mockery-macro = { git = "https://github.com/ic-mockery/ic-mockery" }
```

Once the crates are published to crates.io, you'll be able to use version numbers instead:

```toml
# Not yet available - coming soon
# [dependencies]
# ic-mockery = "0.1.0"
# ic-mockery-macro = "0.1.0"
```

## Usage

### Real Example

```rust
use candid::{CandidType, Principal};
use ic_mockery::mocking_support::AsyncMocker;
use ic_mockery_macro::mock_async_calls;
use pocket_ic::{PocketIc, PocketIcBuilder};
use serde::Deserialize;
use serde_json::{json, to_value};

// Define your request and response types
#[derive(CandidType, Deserialize)]
struct GreetRequest {
    name: String,
}

#[derive(CandidType, Deserialize, Debug, PartialEq)]
struct GreetResponse {
    message: String,
    status: Status,
}

#[derive(CandidType, Deserialize, Debug, PartialEq)]
enum Status {
    Success,
    Error,
}

// Define your canister with async calls
#[mock_async_calls]
impl GreeterCanister {
    // This async method will be transformed to use HTTP outcalls
    async fn greet(request: GreetRequest) -> Result<GreetResponse, String> {
        // In production, this would contain your actual implementation
        // The macro transforms this to use HTTP outcalls that can be mocked
        Ok(GreetResponse {
            message: format!("Hello, {}!", request.name),
            status: Status::Success,
        })
    }
    
    // Another method that can be mocked
    async fn prepare_greet() -> Result<(), String> {
        // Some preparation logic
        Ok(())
    }
}

// In your test
#[test]
fn test_greet_functionality() {
    // Set up PocketIC environment
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .build();
    let canister = pic.create_canister();
    
    // Install your canister WASM
    pic.add_cycles(canister, 2_000_000_000_000); // 2T Cycles
    pic.install_canister(canister, wasm_bytes, vec![], None);
    
    // Set up the mock and execute the call
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
            // Extract name from the request args and create a response
            let response = GreetResponse {
                message: args["args"][0]["name"].as_str().unwrap().into(),
                status: Status::Success,
            };
            to_value(response).unwrap()
        })
        .mock("prepare_greet", |_| to_value::<()>(()).unwrap())
        .execute::<GreetResponse>()
        .expect("mocking failed");
    
    // Verify the response
    assert_eq!(response.message, "Wizard");
    assert_eq!(response.status, Status::Success);
}
```

### Advanced Usage

The library supports more advanced scenarios like:

- Mocking multiple HTTP calls in sequence
- Verifying expected calls were made
- Customizing response based on request parameters

### How the Macro Works

The `mock_async_calls` macro transforms your async methods that return a Result into methods that:

1. Serialize the method name and arguments into a JSON payload
2. Make an HTTP outcall using the IC's HTTP request API
3. Deserialize the response back into your expected return type

During testing, PocketIC intercepts these HTTP requests and allows you to provide mock responses, making it possible to test your canister code without actual network dependencies.

## Project Structure

- `crates/ic-mockery`: Core library with the mocking infrastructure
- `crates/ic-mockery-macro`: Procedural macros for automatic transformation
- `canister`: Example canister implementation

## License

MIT OR Apache-2.0

## Authors

- [Dan B](https://x.com/0xVersion)
