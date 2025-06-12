use std::vec;

use candid::{decode_one, CandidType, Encode, Principal};
use pocket_ic::{
    common::rest::{
        CanisterHttpReply, CanisterHttpResponse, MockCanisterHttpResponse, RawMessageId,
    },
    PocketIc,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub struct AsyncMocker<'a> {
    pic: &'a PocketIc,
    call: Option<Box<dyn FnOnce() -> RawMessageId + 'a>>,
    responders: Vec<(String, Box<dyn Fn(Value) -> Value + 'a>)>,
    expected_calls: Vec<(String, Box<dyn Fn(&Value) -> () + 'a>)>,
    max_ticks: usize,
}

impl<'a> AsyncMocker<'a> {
    pub fn new(pic: &'a PocketIc) -> Self {
        Self {
            pic,
            call: None,
            responders: vec![],
            max_ticks: 50,
            expected_calls: vec![],
        }
    }

    pub fn call<T>(mut self, canister: Principal, from: Principal, method: &'a str, args: T) -> Self
    where
        T: CandidType + 'a,
    {
        self.call = Some(Box::new(move || {
            self.pic
                .submit_call(canister, from, &method, Encode!(&args).unwrap())
                .unwrap()
        }));
        self
    }

    pub fn with_call<F>(mut self, call: F) -> Self
    where
        F: FnOnce() -> RawMessageId + 'a,
    {
        self.call = Some(Box::new(call));
        self
    }

    pub fn mock<F>(mut self, method: &str, responder: F) -> Self
    where
        F: Fn(Value) -> Value + 'a,
    {
        self.responders
            .push((method.to_string(), Box::new(responder)));
        self
    }

    pub fn execute<T>(mut self) -> Result<T, String>
    where
        T: DeserializeOwned + CandidType,
    {
        let call = self.call.take().expect("Missing call");
        let call_id = call();

        let mut tick_count = 0;
        while tick_count < self.max_ticks {
            self.pic.tick();
            tick_count += 1;

            let requests = self.pic.get_canister_http();
            for req in requests {
                let req_json: Value =
                    serde_json::from_slice(&req.body).expect("Invalid JSON in HTTP body");

                let actual_method = req_json
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                let expected_call = self
                    .expected_calls
                    .iter()
                    .position(|item| item.0 == *actual_method);

                if expected_call.is_some() {
                    let (_, verifier) = &self.expected_calls.remove(expected_call.unwrap());
                    verifier(&req_json.clone());
                }

                let responder = self
                    .responders
                    .iter()
                    .position(|item| item.0 == *actual_method);

                if responder.is_some() {
                    let (_, responder) = self.responders.remove(responder.unwrap());
                    let response_json = responder(req_json);
                    let body =
                        serde_json::to_vec(&response_json).expect("Failed to serialize response");

                    let mock = MockCanisterHttpResponse {
                        subnet_id: req.subnet_id,
                        request_id: req.request_id,
                        response: CanisterHttpResponse::CanisterHttpReply(CanisterHttpReply {
                            status: 200,
                            headers: vec![],
                            body,
                        }),
                        additional_responses: vec![],
                    };
                    self.pic.mock_canister_http_response(mock);
                    break;
                }
            }
        }

        if !self.responders.is_empty() {
            return Err(format!(
                "Expected calls {:?}",
                self.responders
                    .iter()
                    .map(|i| i.0.clone())
                    .collect::<Vec<_>>()
            ));
        }

        if !self.expected_calls.is_empty() {
            return Err(format!(
                "Expected calls {:?}",
                self.expected_calls
                    .iter()
                    .map(|i| i.0.clone())
                    .collect::<Vec<_>>()
            ));
        }

        self.pic.tick();
        let reply = self.pic.await_call(call_id);
        let data = reply.map_err(|err| err.to_string())?;
        decode_one(&data).map_err(|e| e.to_string())
    }
}
