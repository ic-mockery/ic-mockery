use ic_mockery_macro::mock_async_calls;

struct CallResultCanister;

#[mock_async_calls]
impl CallResultCanister {
    async fn call_result() -> ic_cdk::call::CallResult<u8> {
        Ok(1)
    }

    async fn call_error() -> Result<u8, ic_cdk::call::Error> {
        Ok(2)
    }
}

fn main() {}
