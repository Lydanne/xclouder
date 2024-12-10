use async_trait::async_trait;
use serde_json::Value;
use crate::{error::XResult, Native, config::BucketSource, cloud_client::UploadOpts};
use super::{Strategy, UrlRes};

pub struct Tos {
    name: String,
    native: Option<Box<dyn Native>>,
}

impl Tos {
    pub fn new() -> Self {
        Self {
            name: "tos".to_string(),
            native: None,
        }
    }
}

#[async_trait]
impl Strategy for Tos {
    fn name(&self) -> &str {
        &self.name
    }

    fn load_native(&mut self, native: Box<dyn Native>) {
        self.native = Some(native);
    }

    fn storage_key(&self, bucket_source: &BucketSource) -> String {
        format!("sts:{}:{}", 
            bucket_source.cloud_name.as_deref().unwrap_or(""),
            bucket_source.name
        )
    }

    fn domain_parser(&self, _domain: &str) -> Value {
        // TOS 不需要特殊的域名解析
        serde_json::json!({})
    }

    async fn get_sts(&self, bucket_source: &BucketSource, opts: &UploadOpts<'_>) -> XResult<Value> {
        if let Some(native) = &self.native {
            let storage_key = format!("sts:{}:{}", 
                bucket_source.cloud_name.as_deref().unwrap_or(""),
                bucket_source.name);
                
            let cache = native.get_storage(&storage_key);
            if let Some(cache) = cache {
                return Ok(cache);
            }

            let bucket_key = bucket_source.domain.as_ref()
                .map(|domain| domain.split('.').next().unwrap_or(""))
                .unwrap_or("");

            let res = native.request(crate::RequestArgs {
                method: "GET".to_string(),
                url: format!("/api/cloud/sts?cloud={}&cloudName={}&bucket={}", 
                    bucket_source.cloud.as_deref().unwrap_or(""),
                    bucket_source.cloud_name.as_deref().unwrap_or(""),
                    bucket_key
                ),
                enable_cache: false,
                timeout: 10000,
                response_type: "json".to_string(),
            }).await?;

            if let Some(expire_at) = res["expireAt"].as_i64() {
                native.set_storage(&storage_key, res.clone());
            }

            Ok(res)
        } else {
            Ok(Value::Null)
        }
    }

    async fn upload(&self, bucket_source: &BucketSource, sts: Value, opts: &UploadOpts<'_>) -> XResult<UrlRes> {
        let base_url = format!("https://{}", bucket_source.domain.as_deref().unwrap_or(""));
        
        if let Some(native) = &self.native {
            let mut form_data = serde_json::json!({
                "key": opts.key,
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
                file_path: opts.file_path.clone(),
                form_data,
                on_progress: opts.on_progress.clone(),
            }).await?;
        }

        Ok(UrlRes {
            base_url,
            key: opts.key.clone(),
            domain: bucket_source.domain.clone().unwrap_or_default(),
            bucket: bucket_source.name.clone(),
        })
    }
} 