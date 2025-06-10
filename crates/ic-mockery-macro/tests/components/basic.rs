use ic_mockery_macro::mock_async_calls;

struct MyCanister;

#[mock_async_calls]
impl MyCanister {
    // simple async fn -> Result<_, _> should compile
    async fn get_value(&self, x: u8) -> Result<u8, String> {
        Ok(x + 1)
    }

    // non-async or non-Result methods are left untouched
    fn sync_call(&self) -> u32 {
        123
    }
}

fn main() {}
