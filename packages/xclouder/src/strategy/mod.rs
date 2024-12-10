pub mod cos;
// pub mod tos;
// pub mod oss;

use async_trait::async_trait;
use serde_json::Value;
use crate::error::XResult;
use crate::config::BucketSource;
use crate::{Native, UploadOpts};

#[async_trait]
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn load_native(&mut self, native: Box<dyn Native>);
    fn storage_key(&self, bucket: &Value) -> String;
    fn domain_parser(&self, domain: &str) -> Value;
    async fn get_sts(&self, bucket_source: &BucketSource, opts: &UploadOpts<'_>) -> XResult<Value>;
    async fn upload(&self, bucket_source: &BucketSource, sts: Value, opts: &UploadOpts<'_>) -> XResult<UrlRes>;
}

pub struct UrlRes {
    pub base_url: String,
    pub key: String,
    pub domain: String,
    pub bucket: String,
}

impl ToString for UrlRes {
    fn to_string(&self) -> String {
        format!("{}/{}", self.base_url, self.key)
    }
}

// // 基础策略实现
// pub struct StrategyImpl {
//     pub name: String,
//     pub native: Option<Box<dyn crate::Native>>,
// }

// impl StrategyImpl {
//     pub fn new(name: &str) -> Self {
//         Self {
//             name: name.to_string(),
//             native: None,
//         }
//     }
// }

// #[async_trait]
// impl Strategy for StrategyImpl {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn load_native(&mut self, native: Box<dyn crate::Native>) {
//         self.native = Some(native);
//     }

//     fn storage_key(&self, bucket: &Value) -> String {
//         format!("sts:{}:{}", 
//             bucket["cloudName"].as_str().unwrap_or(""),
//             bucket["name"].as_str().unwrap_or("")
//         )
//     }

//     fn domain_parser(&self, domain: &str) -> Value {
//         serde_json::json!({})
//     }

//     async fn get_sts(&self, bucket_source: &BucketSource, opts: &UploadOpts) -> XResult<Value> {
//         Ok(Value::Null)
//     }

//     async fn upload(&self, bucket_source: &BucketSource, sts: Value, opts: &UploadOpts) -> XResult<UrlRes> {
//         Ok(UrlRes {
//             base_url: String::new(),
//             key: String::new(),
//             domain: String::new(),
//             bucket: String::new(),
//         })
//     }
// } 
