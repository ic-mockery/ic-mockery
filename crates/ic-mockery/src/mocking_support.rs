use std::vec;

use candid::{decode_one, CandidType, Encode, Principal};
use pocket_ic::{
    common::rest::{
        CanisterHttpReject, CanisterHttpReply, CanisterHttpResponse, MockCanisterHttpResponse,
        RawMessageId,
    },
    PocketIc,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

enum MockHttpResult {
    Reply(Value),
    Reject { code: u8, message: String },
}

pub struct AsyncMocker<'a> {
    pic: &'a PocketIc,
    call: Option<Box<dyn FnOnce() -> RawMessageId + 'a>>,
    responders: Vec<(String, Box<dyn Fn(Value) -> MockHttpResult + 'a>)>,
    expected_calls: Vec<(String, Box<dyn Fn(&Value) + 'a>)>,
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
                .submit_call(canister, from, method, Encode!(&args).unwrap())
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
        self.responders.push((
            method.to_string(),
            Box::new(move |args| MockHttpResult::Reply(responder(args))),
        ));
        self
    }

    pub fn mock_fail(mut self, method: &str, message: &'a str) -> Self {
        self.responders.push((
            method.to_string(),
            Box::new(move |_args| MockHttpResult::Reject {
                code: 1,
                message: message.to_string(),
            }),
        ));
        self
    }
    fn execute_with<T, F, E>(mut self, awaiter: F, tick_before_await: bool) -> Result<T, String>
    where
        T: DeserializeOwned + CandidType,
        F: FnOnce(&PocketIc, RawMessageId) -> Result<Vec<u8>, E>,
        E: std::fmt::Debug,
    {
        let call = self.call.take().expect("Missing call");
        let call_id = call();

        let mut tick_count = 0usize;
        while tick_count < self.max_ticks {
            self.pic.tick();
            tick_count += 1;

            let requests = self.pic.get_canister_http();
            for req in requests {
                let req_json: Value =
                    serde_json::from_slice(&req.body).expect("Invalid JSON in HTTP body");

                let method = req_json
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                if let Some(idx) = self.expected_calls.iter().position(|i| i.0 == method) {
                    let (_, verify) = self.expected_calls.remove(idx);
                    verify(&req_json.clone());
                }

                if let Some(idx) = self.responders.iter().position(|i| i.0 == method) {
                    let (_, responder) = self.responders.remove(idx);
                    let response = match responder(req_json) {
                        MockHttpResult::Reply(json) => {
                            let body =
                                serde_json::to_vec(&json).expect("Failed to serialize response");
                            CanisterHttpResponse::CanisterHttpReply(CanisterHttpReply {
                                status: 200,
                                headers: vec![],
                                body,
                            })
                        }
                        MockHttpResult::Reject { code, message } => {
                            CanisterHttpResponse::CanisterHttpReject(CanisterHttpReject {
                                reject_code: code as u64,
                                message,
                            })
                        }
                    };

                    self.pic
                        .mock_canister_http_response(MockCanisterHttpResponse {
                            subnet_id: req.subnet_id,
                            request_id: req.request_id,
                            response,
                            additional_responses: vec![],
                        });

                    // one-per-tick behavior
                    break;
                }
            }

            // Break early if nothing else is expected.
            if self.responders.is_empty() && self.expected_calls.is_empty() {
                break;
            }
        }

        // Always await the call — even if we blew past max_ticks — to surface real canister errors.
        if tick_before_await {
            self.pic.tick();
        }
        let reply = awaiter(self.pic, call_id);

        // Preserve rejection details (code + message via Debug)
        let data = reply.map_err(|e| format!("{e:?}"))?;

        // Prefer decoding canister-level Result<T, String> and flatten it.
        if let Ok(res) = decode_one::<Result<T, String>>(&data) {
            return res;
        }

        // Fallback: decode T directly.
        decode_one::<T>(&data).map_err(|e| {
            // If we exhausted ticks, include that context without masking the decode error.
            if tick_count >= self.max_ticks {
                format!(
                    "decode error: {e}; note: exhausted {} ticks before await",
                    self.max_ticks
                )
            } else {
                e.to_string()
            }
        })
    }

    pub fn execute<T>(self) -> Result<T, String>
    where
        T: DeserializeOwned + CandidType,
    {
        self.execute_with(|pic, call_id| pic.await_call(call_id), true)
    }

    pub fn execute_no_ticks<T>(mut self) -> Result<T, String>
    where
        T: DeserializeOwned + CandidType,
    {
        self.execute_with(|pic, call_id| pic.await_call_no_ticks(call_id), false)
    }
}
