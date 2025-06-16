# IC-Mockery

A testing and mocking framework for Internet Computer (IC) canister development in Rust.

## Overview

IC-Mockery provides a seamless way to test and mock Internet Computer canisters. The core of the framework is a procedural macro that automatically transforms async methods returning a Result into methods that use HTTP outcalls. This transformation enables the use of PocketIC to mock these calls during testing, without requiring any changes to your production code.

## Features

- **Mock Async HTTP Calls**: Easily mock responses for asynchronous HTTP calls made by your canisters
- **Integration with PocketIC**: Built on top of `pocket-ic` for local IC environment simulation
- **Procedural Macros**: Automatic transformation of async functions that return a Result into methods that use HTTP outcalls, which can then be mocked
- **Fluent API**: Simple and expressive API for setting up mocks and verifying interactions
- **Type Safety**: Full type safety for request and response data using Candid and Serde
- **Error Handling**: Proper handling of different error types, including String and RejectionCode pairs

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
use candid::{CandidType, Deserialize, Principal};
use ic_mockery::mocking_support::AsyncMocker;
use ic_mockery_macro::mock_async_calls;
use pocket_ic::{PocketIc, PocketIcBuilder};
use serde::Serialize;
use serde_json::{json, to_value};

// Define your request and response types
#[derive(CandidType, Deserialize, Serialize, Clone)]
pub struct GreetRequest {
    pub name: String,
}

#[derive(CandidType, Deserialize, Serialize, Debug, PartialEq)]
pub struct GreetResponse {
    pub message: String,
    pub status: Status,
}

#[derive(CandidType, Deserialize, Serialize, Debug, PartialEq)]
pub enum Status {
    Success,
    Error,
}

// Define your service with async calls
pub struct HelloService;

#[mock_async_calls]
impl HelloService {
    // This async method will be transformed to use HTTP outcalls
    pub async fn greet(req: GreetRequest) -> Result<GreetResponse, String> {
        // In production, this would contain your actual implementation
        // The macro transforms this to use HTTP outcalls that can be mocked
        Ok(GreetResponse {
            message: format!("Hello, {}!", req.name),
            status: Status::Success,
        })
    }
    
    // Another method that can be mocked
    pub async fn prepare_greet(req: GreetRequest) -> Result<(), String> {
        // Some preparation logic
        Ok(())
    }
}

// Canister update function that uses the service
#[ic_cdk::update]
async fn greet(req: GreetRequest) -> GreetResponse {
    HelloService::prepare_greet(req.clone()).await.unwrap();
    HelloService::greet(req).await.unwrap()
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
            let name = args["args"][0]["name"].as_str().unwrap();
            let response = GreetResponse {
                message: format!("Hello, {}!", name),
                status: Status::Success,
            };
            to_value(response).unwrap()
        })
        .mock("prepare_greet", |_| to_value::<()>(()).unwrap())
        .execute::<GreetResponse>()
        .expect("mocking failed");
    
    // Verify the response
    assert_eq!(response.message, "Hello, Wizard!");
    assert_eq!(response.status, Status::Success);
}

// Example of mocking a failure case
#[test]
fn test_greet_failure() {
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .build();
    let canister = pic.create_canister();
    
    pic.add_cycles(canister, 2_000_000_000_000);
    pic.install_canister(canister, wasm_bytes, vec![], None);
    
    // Set up a mock that returns an error
    let result = AsyncMocker::new(&pic)
        .call(
            canister,
            Principal::anonymous(),
            "greet",
            GreetRequest {
                name: "Error".into(),
            },
        )
        // Mock prepare_greet to succeed
        .mock("prepare_greet", |_| to_value::<()>(()).unwrap())
        // Mock greet to fail with an error message
        .mock("greet", |_| {
            // Return an error instead of a successful response
            Err("Invalid name provided".to_string())
        })
        .execute::<GreetResponse>();
    
    // Verify the error
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid name provided");
}
```

### Basic Macro Usage

The `mock_async_calls` macro can be applied to any implementation block containing async methods that return a `Result`. Here's a simple example:

```rust
use ic_mockery_macro::mock_async_calls;

struct MyCanister;

#[mock_async_calls]
impl MyCanister {
    // This async method will be transformed to use HTTP outcalls
    async fn get_value(&self, x: u8) -> Result<u8, String> {
        Ok(x + 1)
    }

    // Non-async or non-Result methods are left untouched
    fn sync_call(&self) -> u32 {
        123
    }
}
```

### Advanced Usage

The library supports more advanced scenarios like:

- Mocking multiple HTTP calls in sequence
- Verifying expected calls were made
- Customizing response based on request parameters
- Handling different error types (String, custom errors, RejectionCode pairs)

#### Using `with_call` and `mock_fail`

For more complex scenarios, you can use `with_call` to provide a custom call function and `mock_fail` for simplified error mocking:

```rust
let borrow_result = AsyncMocker::new(&pic)
    .with_call(|| {
        let (borrow_account, borrow_request) =
            create_borrow_request(Nat::from(0.15e8 as u128), &account, pic, canister, new_pool);
        pic.submit_call(
            canister,
            Principal::anonymous(),
            "borrow_assets",
            Encode!(&borrow_account, &borrow_request).unwrap(),
        )
        .unwrap()
    })
    .mock_fail("withdraw", "Failed withdrawal")
    .execute::<BorrowResult>()
    .unwrap();
```

In this example:
- `with_call` allows you to provide a custom function for making the canister call
- `mock_fail` is a convenient shorthand for mocking a method to fail with a specific error message

### How the Macro Works

The `mock_async_calls` macro transforms your async methods that return a Result into methods that:

1. Serialize the method name and arguments into a JSON payload
2. Make an HTTP outcall using the IC's HTTP request API to a local endpoint (localhost:6969)
3. Deserialize the response back into your expected return type

During testing, PocketIC intercepts these HTTP requests and allows you to provide mock responses, making it possible to test your canister code without actual network dependencies.

### Error Handling

The macro handles different error types:
- String errors (most common)
- (RejectionCode, String) pairs (for IC rejection codes)
- Other custom error types

## Project Structure

- `crates/ic-mockery`: Core library with the mocking infrastructure
- `crates/ic-mockery-macro`: Procedural macros for automatic transformation
- `canister`: Example canister implementation

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT OR Apache-2.0

## Authors

- [Dan B](https://x.com/0xVersion)
