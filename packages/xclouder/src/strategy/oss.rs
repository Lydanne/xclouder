use async_trait::async_trait;
use serde_json::Value;
use crate::{error::XResult, Native};
use super::{Strategy, UrlRes};

pub struct Oss {
    name: String,
    native: Option<Box<dyn Native>>,
}

impl Oss {
    pub fn new() -> Self {
        Self {
            name: "oss".to_string(),
            native: None,
        }
    }
}

#[async_trait]
impl Strategy for Oss {
    fn name(&self) -> &str {
        &self.name
    }

    fn load_native(&mut self, native: Box<dyn Native>) {
        self.native = Some(native);
    }

    fn storage_key(&self, bucket: &Value) -> String {
        format!("sts:{}:{}", 
            bucket["cloudName"].as_str().unwrap_or(""),
            bucket["name"].as_str().unwrap_or("")
        )
    }

    fn domain_parser(&self, _domain: &str) -> Value {
        // OSS 不需要特殊的域名解析
        serde_json::json!({})
    }

    async fn get_sts(&self, bucket: &Value, opts: &Value) -> XResult<Value> {
        if let Some(native) = &self.native {
            let cache = native.get_storage(&self.storage_key(bucket));
            if let Some(cache) = cache {
                return Ok(cache);
            }

            let bucket_key = bucket["domain"].as_str()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");

            let res = native.request(crate::RequestArgs {
                method: "GET".to_string(),
                url: format!("/api/cloud/sts?cloud={}&cloudName={}&bucket={}", 
                    bucket["cloud"].as_str().unwrap_or(""),
                    bucket["cloudName"].as_str().unwrap_or(""),
                    bucket_key
                ),
                enable_cache: false,
                timeout: 10000,
                response_type: "json".to_string(),
            }).await?;

            if let Some(expire_at) = res["expireAt"].as_i64() {
                native.set_storage(&self.storage_key(bucket), res.clone());
            }

            Ok(res)
        } else {
            Ok(Value::Null)
        }
    }

    async fn upload(&self, bucket: &Value, sts: Value, opts: &Value) -> XResult<UrlRes> {
        let base_url = format!("https://{}", bucket["domain"].as_str().unwrap_or(""));
        
        if let Some(native) = &self.native {
            let mut form_data = serde_json::json!({
                "key": opts["key"],
                "success_action_status": "200",
            });

            if let Some(merge_data) = sts["mergeFormData"].as_object() {
                if let Some(obj) = form_data.as_object_mut() {
                    for (k, v) in merge_data {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }

            native.upload_file(crate::UploadArgs {
                url: base_url.clone(),
                name: "file".to_string(),
                file_path: opts["filePath"].as_str().unwrap_or("").to_string(),
                form_data,
                on_progress: None,
            }).await?;
        }

        Ok(UrlRes {
            base_url,
            key: opts["key"].as_str().unwrap_or("").to_string(),
            domain: bucket["domain"].as_str().unwrap_or("").to_string(),
            bucket: bucket["name"].as_str().unwrap_or("").to_string(),
        })
    }
} 