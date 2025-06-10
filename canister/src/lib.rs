use candid::{CandidType, Deserialize};
use ic_mockery_macro::mock_async_calls;

use serde::Serialize;

#[derive(CandidType, Deserialize, Serialize, Debug)]
pub enum Status {
    Success,
    Error,
}

#[derive(CandidType, Deserialize, Serialize, Debug)]
pub struct GreetResponse {
    pub message: String,
    pub status: Status,
}

pub struct HelloService;

#[mock_async_calls]
impl HelloService {
    pub async fn greet(_req: GreetRequest) -> Result<GreetResponse, String> {
        unimplemented!()
    }

    pub async fn prepare_greet(_req: GreetRequest) -> Result<(), String> {
        Ok(())
    }
}

#[derive(CandidType, Deserialize, Serialize, Clone)]
pub struct GreetRequest {
    pub name: String,
}

#[ic_cdk::update]
async fn greet(req: GreetRequest) -> GreetResponse {
    HelloService::prepare_greet(req.clone()).await.unwrap();
    HelloService::greet(req).await.unwrap()
}
